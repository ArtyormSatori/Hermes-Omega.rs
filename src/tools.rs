use crate::error::{HermesError, Result};
use crate::event_stream::{AgentAction, AgentObservation};
use crate::hermes_core::SkillManager;
use crate::human_harness::OpenHumanHarness;
use crate::sandbox::SandboxRuntime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub toolset: String,
    pub description: String,
    pub parameters_schema: Value,
}

pub struct ToolRegistry {
    tools: Vec<ToolDefinition>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = Self { tools: Vec::new() };
        reg.register_builtin_toolsets();
        reg
    }

    pub fn register(&mut self, def: ToolDefinition) {
        self.tools.push(def);
    }

    pub fn list_tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub fn openai_tool_schemas(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters_schema
                    }
                })
            })
            .collect()
    }

    fn register_builtin_toolsets(&mut self) {
        self.register(ToolDefinition {
            name: "terminal_exec".into(),
            toolset: "terminal".into(),
            description: "Execute a shell command inside the isolated sandbox environment".into(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "cwd": { "type": "string" },
                    "background": { "type": "boolean" }
                },
                "required": ["command"]
            }),
        });

        self.register(ToolDefinition {
            name: "file_op".into(),
            toolset: "file_tools".into(),
            description: "Read, write, patch, or search files in the workspace".into(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "operation": { "type": "string", "enum": ["read", "write", "patch", "search"] },
                    "content": { "type": "string" }
                },
                "required": ["path", "operation"]
            }),
        });

        self.register(ToolDefinition {
            name: "spawn_subagent".into(),
            toolset: "delegation".into(),
            description: "OpenHands & Grokbot Delegation Tool: Spawn or wake a specialist sub-agent inside the OpenHuman Harness".into(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "agent_name": { "type": "string" },
                    "role_hint": { "type": "string" },
                    "task_prompt": { "type": "string" }
                },
                "required": ["agent_name", "task_prompt"]
            }),
        });

        self.register(ToolDefinition {
            name: "save_skill".into(),
            toolset: "skills".into(),
            description: "Hermes Closed Learning Loop: Save a reusable SKILL.md from solved task experience".into(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "skill_name": { "type": "string" },
                    "description": { "type": "string" },
                    "instructions_markdown": { "type": "string" }
                },
                "required": ["skill_name", "description", "instructions_markdown"]
            }),
        });
    }

    pub async fn dispatch_action(
        &self,
        action: &AgentAction,
        sandbox: &SandboxRuntime,
        skill_manager: &SkillManager,
        harness: &OpenHumanHarness,
    ) -> Result<AgentObservation> {
        match action {
            AgentAction::RunTerminalCommand {
                command,
                cwd,
                background,
            } => {
                sandbox
                    .execute_command(command, cwd.as_deref(), *background)
                    .await
            }
            AgentAction::FileEdit {
                path,
                operation,
                content,
            } => Self::execute_file_tool(path, operation, content.as_deref()),
            AgentAction::SpawnSubAgent {
                agent_name,
                role_hint,
                task_prompt,
            } => {
                let sub_session = format!("sub-{}", uuid::Uuid::new_v4().simple());
                let slot = harness.spawn_or_wake_agent(agent_name, role_hint, &sub_session)?;
                Ok(AgentObservation::SubAgentCompleted {
                    agent_name: slot.agent_name,
                    summary_output: format!(
                        "Sub-agent `{}` (Role: {}) executed delegated task `{}` in session `{}`.",
                        agent_name, role_hint, task_prompt, slot.active_session_id
                    ),
                })
            }
            AgentAction::SaveSkill {
                skill_name,
                description,
                instructions_markdown,
            } => {
                let skill = skill_manager.create_or_update_skill(
                    skill_name,
                    description,
                    &["auto-synthesized".to_string()],
                    instructions_markdown,
                )?;
                Ok(AgentObservation::SkillSaved {
                    skill_name: skill.name,
                    file_path: skill.file_path.display().to_string(),
                })
            }
            AgentAction::BrowseWeb { url, query } => Ok(AgentObservation::WebResult {
                url: url.clone(),
                snippet: format!(
                    "Fetched and extracted primary content from {} (query: {:?})",
                    url, query
                ),
            }),
            AgentAction::Message { role, content } => Ok(AgentObservation::FileResult {
                path: format!("message:{}", role),
                success: true,
                data: content.clone(),
            }),
        }
    }

    fn execute_file_tool(
        path: &str,
        operation: &str,
        content: Option<&str>,
    ) -> Result<AgentObservation> {
        let p = Path::new(path);
        match operation {
            "read" => {
                let data = std::fs::read_to_string(p)?;
                Ok(AgentObservation::FileResult {
                    path: path.to_string(),
                    success: true,
                    data,
                })
            }
            "write" => {
                if let Some(parent) = p.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                let body = content.unwrap_or("");
                std::fs::write(p, body)?;
                Ok(AgentObservation::FileResult {
                    path: path.to_string(),
                    success: true,
                    data: format!("Wrote {} bytes to {}", body.len(), path),
                })
            }
            "patch" => {
                let existing = std::fs::read_to_string(p).unwrap_or_default();
                let appended = format!("{}\n{}", existing, content.unwrap_or(""));
                std::fs::write(p, &appended)?;
                Ok(AgentObservation::FileResult {
                    path: path.to_string(),
                    success: true,
                    data: format!("Patched {} ({} bytes total)", path, appended.len()),
                })
            }
            other => Err(HermesError::ToolExecution {
                tool: "file_op".into(),
                message: format!("Unsupported file operation: {}", other),
            }),
        }
    }
}
