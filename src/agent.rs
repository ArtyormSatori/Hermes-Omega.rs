//! # Unified AIAgent Core Loop (`src/agent.rs` — v1.0.0 Final Release)
//!
//! Hợp nhất toàn bộ 16 phân hệ của **Ω Hermes Omega.rs**:
//! - Quét chống Prompt Injection (`PromptInjectionGuard::scan_and_spotlight`) và kiểm tra rò rỉ Canary Token (`verify_no_canary_leak`) trên mọi lượt hội thoại.
//! - Lưu trữ phiên vào SQLite WAL + FTS5 tiếng Việt an toàn qua `SqlSecurityGuard`.
//! - Đồng bộ trạng thái nhận thức & khẩu hình tiếng Việt thời gian thực lên `MuseAvatarController`.
//! - Hỗ trợ truyền tải tệp >1GB qua `LargeFileEngine`.

use crate::avatar::{MuseAvatarController, MuseAvatarFrame, MuseAvatarState};
use crate::config::HermesConfig;
use crate::error::Result;
use crate::event_stream::{AgentAction, ContextCondenser, EventPayload, EventStream};
use crate::grok_engine::{LlmRouter, RoleInferenceEngine};
use crate::hermes_core::{HermesStateDb, PromptBuilder, SkillManager};
use crate::human_harness::{AgentRunState, MemoryTreeWiki, OpenHumanHarness};
use crate::large_transfer::LargeFileEngine;
use crate::sandbox::SandboxRuntime;
use crate::security::audit_shell_command;
use crate::tools::ToolRegistry;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnExecutionResult {
    pub session_id: String,
    pub agent_name: String,
    pub inferred_role: String,
    pub reply: String,
    pub condensed_notice: Option<String>,
    pub avatar_frame: MuseAvatarFrame,
}

pub struct HermesUnifiedAgent {
    pub config: HermesConfig,
    pub state_db: HermesStateDb,
    pub skill_manager: SkillManager,
    pub prompt_builder: PromptBuilder,
    pub event_stream: EventStream,
    pub condenser: ContextCondenser,
    pub harness: OpenHumanHarness,
    pub memory_wiki: MemoryTreeWiki,
    pub sandbox: SandboxRuntime,
    pub tool_registry: ToolRegistry,
    pub llm_router: LlmRouter,
    pub transfer_engine: LargeFileEngine,
    pub avatar: Mutex<MuseAvatarController>,
}

impl HermesUnifiedAgent {
    pub fn new(config: HermesConfig) -> Result<Self> {
        let home = config.ensure_directories()?;
        let state_db = HermesStateDb::new(&home)?;
        let skill_manager = SkillManager::new(&home)?;
        let prompt_builder = PromptBuilder::new(&config);
        let session_id = format!("sess-{}", Uuid::new_v4().simple());
        let event_stream = EventStream::new(&session_id);
        let condenser = ContextCondenser::new(
            config.memory.context_condenser_threshold_messages,
            config.memory.context_condenser_keep_head,
            config.memory.context_condenser_keep_tail,
        );
        let harness = OpenHumanHarness::new(config.openhuman.max_concurrent_agents);
        let memory_wiki = MemoryTreeWiki::new(&home)?;
        let sandbox = SandboxRuntime::new(config.sandbox.clone());
        let tool_registry = ToolRegistry::new();
        let llm_router = LlmRouter::new(config.clone());
        let transfer_engine = LargeFileEngine::new(&home)?;
        let avatar = Mutex::new(MuseAvatarController::new());

        Ok(Self {
            config,
            state_db,
            skill_manager,
            prompt_builder,
            event_stream,
            condenser,
            harness,
            memory_wiki,
            sandbox,
            tool_registry,
            llm_router,
            transfer_engine,
            avatar,
        })
    }

    pub async fn process_turn(
        &self,
        agent_name: &str,
        platform: &str,
        user_input: &str,
    ) -> Result<TurnExecutionResult> {
        // 1. Quét tường lửa chống Prompt Injection & bọc dữ liệu trong thẻ Spotlight
        let scan_report = self
            .prompt_builder
            .injection_guard
            .scan_and_spotlight(user_input)?;

        // 2. Chuyển trạng thái MUSE.AI Avatar sang Thinking
        {
            let mut av = self.avatar.lock();
            av.set_state(MuseAvatarState::Thinking);
        }

        // 3. Suy luận vai trò Grokbot & đánh thức Agent trong OpenHuman Harness
        let role_profile = RoleInferenceEngine::infer_from_name(agent_name);
        let slot = self.harness.spawn_or_wake_agent(
            agent_name,
            &role_profile.role_title,
            &self.event_stream.session_id,
        )?;

        // 4. Lưu vào SQLite WAL + FTS5 (đã bảo vệ qua SqlSecurityGuard + Prepared Statements)
        self.state_db
            .upsert_session(&slot.active_session_id, None, platform, agent_name)?;
        self.state_db.append_message(
            &slot.active_session_id,
            "user",
            &scan_report.sanitized_text,
        )?;

        self.event_stream
            .publish(EventPayload::Action(AgentAction::Message {
                role: "user".to_string(),
                content: scan_report.sanitized_text.clone(),
            }));

        // 5. Thực thi lệnh Sandbox nếu có tiền tố `!` (kiểm tra qua Shell Injection Auditor trước)
        if let Some(cmd_str) = scan_report.sanitized_text.strip_prefix('!') {
            let trimmed_cmd = cmd_str.trim();
            audit_shell_command(trimmed_cmd)?;
            {
                let mut av = self.avatar.lock();
                av.set_state(MuseAvatarState::Executing);
            }
            let action = AgentAction::RunTerminalCommand {
                command: trimmed_cmd.to_string(),
                cwd: None,
                background: false,
            };
            self.event_stream
                .publish(EventPayload::Action(action.clone()));
            let obs = self
                .tool_registry
                .dispatch_action(&action, &self.sandbox, &self.skill_manager, &self.harness)
                .await?;
            self.event_stream
                .publish(EventPayload::Observation(obs));
        }

        // 6. Lắp ghép System Prompt 3 tầng (kèm Canary Token & Spotlight Wiki)
        let skills = self.skill_manager.list_skills()?;
        let wiki_index = self.memory_wiki.compile_compressed_index()?;
        let system_prompt = self.prompt_builder.build_system_prompt(
            agent_name,
            &role_profile.role_title,
            &skills,
            &wiki_index,
        );

        // 7. Gọi LLM với dữ liệu đầu vào đã được bọc Spotlight
        let schemas = self.tool_registry.openai_tool_schemas();
        let reply = self
            .llm_router
            .complete(&system_prompt, &scan_report.spotlight_wrapped, &schemas)
            .await?;

        // 8. Kiểm tra chống rò rỉ Canary Token ở đầu ra
        self.prompt_builder
            .injection_guard
            .verify_no_canary_leak(&reply)?;

        // 9. Lưu phản hồi & cập nhật khẩu hình MUSE.AI Avatar sang Speaking
        self.state_db
            .append_message(&slot.active_session_id, "assistant", &reply)?;
        self.event_stream
            .publish(EventPayload::Action(AgentAction::Message {
                role: "assistant".to_string(),
                content: reply.clone(),
            }));

        let avatar_frame = {
            let mut av = self.avatar.lock();
            av.set_state(MuseAvatarState::Speaking);
            av.next_frame(reply.chars().next())
        };

        // 10. Tự động nén ngữ cảnh (OpenHands ContextCondenser) & tự học SKILL.md (Hermes Closed Loop)
        let condensed_notice = self.condenser.condense_if_needed(&self.event_stream);

        let lower_input = scan_report.sanitized_text.to_lowercase();
        if self.config.memory.auto_create_skills
            && (lower_input.contains("remember workflow")
                || lower_input.contains("save skill")
                || lower_input.contains("lưu kỹ năng"))
        {
            let _ = self.skill_manager.create_or_update_skill(
                &format!("{}-learned-workflow", agent_name),
                "Automatically synthesized skill from conversation problem-solving",
                &["closed-learning-loop".to_string()],
                &format!(
                    "User trigger: {}\nSynthesized solution:\n{}",
                    scan_report.sanitized_text, reply
                ),
            );
        }

        self.harness.update_status(agent_name, AgentRunState::Idle);

        Ok(TurnExecutionResult {
            session_id: slot.active_session_id,
            agent_name: agent_name.to_string(),
            inferred_role: role_profile.role_title,
            reply,
            condensed_notice,
            avatar_frame,
        })
    }
}
