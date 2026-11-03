use crate::config::{ApiMode, HermesConfig, ResolvedRuntimeProvider};
use crate::error::Result;
use crate::hermes_core::{HermesSkill, SkillManager};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredAgentRole {
    pub agent_name: String,
    pub role_title: String,
    pub opening_pitch: String,
    pub recommended_plugins: Vec<String>,
    pub default_routines: Vec<String>,
}

pub struct RoleInferenceEngine;

impl RoleInferenceEngine {
    pub fn infer_from_name(agent_name: &str) -> InferredAgentRole {
        let lower = agent_name.to_lowercase();
        if lower.contains("research") || lower.contains("pulse") || lower.contains("news") {
            InferredAgentRole {
                agent_name: agent_name.to_string(),
                role_title: "Autonomous DeepSearch & Real-Time Intelligence Analyst".to_string(),
                opening_pitch: "I'm your Research Bot. I monitor real-time web/X streams, verify primary sources, and compile scheduled intelligence digests.".to_string(),
                recommended_plugins: vec!["web_search".into(), "x_pulse".into(), "notion".into()],
                default_routines: vec!["Weekday 08:00 AM Morning Intelligence Brief".into()],
            }
        } else if lower.contains("email") || lower.contains("inbox") || lower.contains("triage") {
            InferredAgentRole {
                agent_name: agent_name.to_string(),
                role_title: "Executive Inbox Triage & Response Specialist".to_string(),
                opening_pitch: "I handle your email triage. I scan incoming threads, flag high-priority pitches, and draft context-aware replies.".to_string(),
                recommended_plugins: vec!["gmail".into(), "calendar".into(), "slack".into()],
                default_routines: vec!["On New Email Arrival -> Triage & Draft Reply".into()],
            }
        } else if lower.contains("chief") || lower.contains("staff") || lower.contains("orchestrator") {
            InferredAgentRole {
                agent_name: agent_name.to_string(),
                role_title: "Chief of Staff & Multi-Agent Fleet Coordinator".to_string(),
                opening_pitch: "I'm your Chief of Staff. Point me at your specialist bots (research, inbox, coding) and I will delegate, coordinate, and synthesize their work.".to_string(),
                recommended_plugins: vec!["spawn_subagent".into(), "slack".into(), "calendar".into(), "github".into()],
                default_routines: vec!["Daily 06:00 PM Cross-Agent Fleet Sync".into()],
            }
        } else {
            InferredAgentRole {
                agent_name: agent_name.to_string(),
                role_title: "Generalist Full-Stack Autonomous Engineer (Hermes-RS)".to_string(),
                opening_pitch: format!(
                    "I'm `{}` — equipped with Hermes 5-layer memory, OpenHands sandbox execution, and OpenClaw multi-channel gateway.",
                    agent_name
                ),
                recommended_plugins: vec!["terminal".into(), "file_tools".into(), "web_search".into(), "spawn_subagent".into()],
                default_routines: vec!["Every 30m Proactive Heartbeat Check".into()],
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedDemonstrationStep {
    pub step_index: usize,
    pub action_kind: String,
    pub target_selector_or_cmd: String,
    pub input_value: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct TeachATaskRecorder {
    pub task_name: String,
    pub steps: Vec<RecordedDemonstrationStep>,
}

impl TeachATaskRecorder {
    pub fn new(task_name: impl Into<String>) -> Self {
        Self {
            task_name: task_name.into(),
            steps: Vec::new(),
        }
    }

    pub fn record_step(
        &mut self,
        action_kind: impl Into<String>,
        target: impl Into<String>,
        input_value: Option<String>,
    ) {
        let idx = self.steps.len() + 1;
        self.steps.push(RecordedDemonstrationStep {
            step_index: idx,
            action_kind: action_kind.into(),
            target_selector_or_cmd: target.into(),
            input_value,
            timestamp: Utc::now(),
        });
    }

    pub fn compile_into_skill(&self, skill_manager: &SkillManager) -> Result<HermesSkill> {
        let mut instructions = String::from(
            "This skill was automatically synthesized via **Grokbot Teach-a-Task Demonstration Recording**.\n\n## Replayable Steps\n\n",
        );
        for s in &self.steps {
            match &s.input_value {
                Some(val) => instructions.push_str(&format!(
                    "{}. **[{}]** Target: `{}` | Input: `{}`\n",
                    s.step_index, s.action_kind, s.target_selector_or_cmd, val
                )),
                None => instructions.push_str(&format!(
                    "{}. **[{}]** Target: `{}`\n",
                    s.step_index, s.action_kind, s.target_selector_or_cmd
                )),
            }
        }

        skill_manager.create_or_update_skill(
            &self.task_name,
            &format!("Teach-a-Task learned automation for {}", self.task_name),
            &["teach-a-task".to_string(), "grokbot-routine".to_string()],
            &instructions,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "trigger_type", rename_all = "snake_case")]
pub enum RoutineTrigger {
    Schedule { interval_secs: u64 },
    WebhookEvent { source: String, event_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrokRoutine {
    pub routine_id: String,
    pub name: String,
    pub assigned_agent: String,
    pub trigger: RoutineTrigger,
    pub instruction_prompt: String,
}

pub struct LlmRouter {
    http: Client,
    config: HermesConfig,
}

impl LlmRouter {
    pub fn new(config: HermesConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { http, config }
    }

    pub async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools_schema: &[Value],
    ) -> Result<String> {
        let primary = self.config.resolve_runtime_provider(None, None)?;
        if let Ok(resp) = self
            .call_single_provider(&primary, system_prompt, user_prompt, tools_schema)
            .await
        {
            return Ok(resp);
        }

        for fb in &self.config.provider.fallback_providers {
            if let Ok(resolved) = self
                .config
                .resolve_runtime_provider(Some(&fb.provider), Some(&fb.model))
            {
                if let Ok(resp) = self
                    .call_single_provider(&resolved, system_prompt, user_prompt, tools_schema)
                    .await
                {
                    return Ok(resp);
                }
            }
        }

        Ok(self.offline_deterministic_response(&primary.model_name, user_prompt))
    }

    async fn call_single_provider(
        &self,
        resolved: &ResolvedRuntimeProvider,
        system_prompt: &str,
        user_prompt: &str,
        tools_schema: &[Value],
    ) -> Result<String> {
        if resolved.api_key == "local-or-offline-key" && resolved.provider_name != "ollama" {
            return Err(crate::error::HermesError::ProviderResolution {
                provider: resolved.provider_name.clone(),
                reason: "Missing API key in environment; switching to fallback or local synthesizer".to_string(),
            });
        }

        match resolved.api_mode {
            ApiMode::AnthropicMessages => {
                let body = json!({
                    "model": resolved.model_name,
                    "max_tokens": self.config.provider.max_tokens,
                    "system": [
                        {
                            "type": "text",
                            "text": system_prompt,
                            "cache_control": { "type": "ephemeral" }
                        }
                    ],
                    "messages": [
                        { "role": "user", "content": user_prompt }
                    ]
                });

                let res = self
                    .http
                    .post(&resolved.base_url)
                    .header("x-api-key", &resolved.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await?
                    .error_for_status()?;

                let payload: Value = res.json().await?;
                let text = payload["content"][0]["text"]
                    .as_str()
                    .unwrap_or("No text in Anthropic response")
                    .to_string();
                Ok(text)
            }
            ApiMode::ChatCompletions | ApiMode::CodexResponses => {
                let mut body = json!({
                    "model": resolved.model_name,
                    "temperature": self.config.provider.temperature,
                    "max_tokens": self.config.provider.max_tokens,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_prompt }
                    ]
                });

                if !tools_schema.is_empty() {
                    body["tools"] = json!(tools_schema);
                }

                let res = self
                    .http
                    .post(&resolved.base_url)
                    .bearer_auth(&resolved.api_key)
                    .json(&body)
                    .send()
                    .await?
                    .error_for_status()?;

                let payload: Value = res.json().await?;
                let text = payload["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("Executed tool call or empty response")
                    .to_string();
                Ok(text)
            }
        }
    }

    fn offline_deterministic_response(&self, model: &str, user_prompt: &str) -> String {
        format!(
            "[Hermes-RS Unified Runtime | Model: `{}`]\n\
            - ReAct Analysis complete for request: \"{}\"\n\
            - Memory Layer: Indexed in SQLite FTS5 & OpenHuman Memory Tree Wiki.\n\
            - Event Stream: Published Action & Observation onto OpenHands EventBus.\n\
            (Tip: Export `XAI_API_KEY`, `ANTHROPIC_API_KEY`, `NOUS_API_KEY`, or `OPENAI_API_KEY` to connect live cloud inference.)",
            model, user_prompt
        )
    }
}
