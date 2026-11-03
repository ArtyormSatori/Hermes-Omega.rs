use crate::config::HermesConfig;
use crate::error::Result;
use crate::event_stream::{AgentAction, ContextCondenser, EventPayload, EventStream};
use crate::grok_engine::{LlmRouter, RoleInferenceEngine};
use crate::hermes_core::{HermesStateDb, PromptBuilder, SkillManager};
use crate::human_harness::{AgentRunState, MemoryTreeWiki, OpenHumanHarness};
use crate::sandbox::SandboxRuntime;
use crate::tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnExecutionResult {
    pub session_id: String,
    pub agent_name: String,
    pub inferred_role: String,
    pub reply: String,
    pub condensed_notice: Option<String>,
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
        })
    }

    pub async fn process_turn(
        &self,
        agent_name: &str,
        platform: &str,
        user_input: &str,
    ) -> Result<TurnExecutionResult> {
        let role_profile = RoleInferenceEngine::infer_from_name(agent_name);

        let slot = self.harness.spawn_or_wake_agent(
            agent_name,
            &role_profile.role_title,
            &self.event_stream.session_id,
        )?;

        self.state_db
            .upsert_session(&slot.active_session_id, None, platform, agent_name)?;
        self.state_db
            .append_message(&slot.active_session_id, "user", user_input)?;

        self.event_stream
            .publish(EventPayload::Action(AgentAction::Message {
                role: "user".to_string(),
                content: user_input.to_string(),
            }));

        if let Some(cmd_str) = user_input.strip_prefix('!') {
            let action = AgentAction::RunTerminalCommand {
                command: cmd_str.trim().to_string(),
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

        let skills = self.skill_manager.list_skills()?;
        let wiki_index = self.memory_wiki.compile_compressed_index()?;
        let system_prompt = self.prompt_builder.build_system_prompt(
            agent_name,
            &role_profile.role_title,
            &skills,
            &wiki_index,
        );

        let schemas = self.tool_registry.openai_tool_schemas();
        let reply = self
            .llm_router
            .complete(&system_prompt, user_input, &schemas)
            .await?;

        self.state_db
            .append_message(&slot.active_session_id, "assistant", &reply)?;
        self.event_stream
            .publish(EventPayload::Action(AgentAction::Message {
                role: "assistant".to_string(),
                content: reply.clone(),
            }));

        let condensed_notice = self.condenser.condense_if_needed(&self.event_stream);

        if self.config.memory.auto_create_skills
            && (user_input.to_lowercase().contains("remember workflow")
                || user_input.to_lowercase().contains("save skill"))
        {
            let _ = self.skill_manager.create_or_update_skill(
                &format!("{}-learned-workflow", agent_name),
                "Automatically synthesized skill from conversation problem-solving",
                &["closed-learning-loop".to_string()],
                &format!("User trigger: {}\nSynthesized solution:\n{}", user_input, reply),
            );
        }

        self.harness.update_status(agent_name, AgentRunState::Idle);

        Ok(TurnExecutionResult {
            session_id: slot.active_session_id,
            agent_name: agent_name.to_string(),
            inferred_role: role_profile.role_title,
            reply,
            condensed_notice,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_hermes_agent_mechanisms() {
        let mut cfg = HermesConfig::default();
        cfg.hermes_home = "/tmp/hermes_omega_rs_unit_test".to_string();
        let agent = HermesUnifiedAgent::new(cfg).expect("Failed to initialize HermesUnifiedAgent");

        let res = agent
            .process_turn("researchbot", "cli", "Analyze Rust async memory safety remember workflow")
            .await
            .expect("Turn failed");
        assert!(res.inferred_role.contains("Intelligence Analyst"));
        assert_eq!(agent.harness.active_count(), 1);

        let hits = agent
            .state_db
            .search_past_sessions("Rust", 5)
            .expect("FTS5 search failed");
        assert!(!hits.is_empty());

        let skills = agent.skill_manager.list_skills().expect("List skills failed");
        assert!(!skills.is_empty());
    }
}
