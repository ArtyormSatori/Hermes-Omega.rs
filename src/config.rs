use crate::error::{HermesError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiMode {
    ChatCompletions,
    CodexResponses,
    AnthropicMessages,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackProvider {
    pub provider: String,
    pub model: String,
    pub api_mode: ApiMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub active_provider: String,
    pub active_model: String,
    pub api_mode: ApiMode,
    pub temperature: f32,
    pub max_tokens: u32,
    pub enable_prompt_caching: bool,
    #[serde(default)]
    pub fallback_providers: Vec<FallbackProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub memory_md_max_chars: usize,
    pub user_md_max_chars: usize,
    pub enable_fts5_search: bool,
    pub auto_create_skills: bool,
    pub honcho_dual_peer_modeling: bool,
    pub context_condenser_threshold_messages: usize,
    pub context_condenser_keep_head: usize,
    pub context_condenser_keep_tail: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackendType {
    Local,
    Docker,
    Ssh,
    Firecracker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub default_backend: SandboxBackendType,
    pub docker_image: String,
    pub command_timeout_secs: u64,
    pub require_approval_for_high_risk: bool,
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub bind_addr: String,
    pub heartbeat_interval_secs: u64,
    pub enable_dm_pairing: bool,
    pub authorized_users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanConfig {
    pub max_concurrent_agents: usize,
    pub agent_memory_budget_kb: usize,
    pub memory_tree_sync_interval_secs: u64,
    pub connected_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrokbotConfig {
    pub enable_role_inference: bool,
    pub enable_teach_a_task_recorder: bool,
    pub enable_chief_of_staff_delegation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesConfig {
    pub profile: String,
    pub hermes_home: String,
    pub provider: ProviderConfig,
    pub memory: MemoryConfig,
    pub sandbox: SandboxConfig,
    pub gateway: GatewayConfig,
    pub openhuman: OpenHumanConfig,
    pub grokbot: GrokbotConfig,
}

#[derive(Debug, Clone)]
pub struct ResolvedRuntimeProvider {
    pub provider_name: String,
    pub model_name: String,
    pub api_mode: ApiMode,
    pub base_url: String,
    pub api_key: String,
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            profile: "default".to_string(),
            hermes_home: "~/.hermes".to_string(),
            provider: ProviderConfig {
                active_provider: "xai".to_string(),
                active_model: "grok-4.6".to_string(),
                api_mode: ApiMode::ChatCompletions,
                temperature: 0.2,
                max_tokens: 8192,
                enable_prompt_caching: true,
                fallback_providers: vec![
                    FallbackProvider {
                        provider: "anthropic".to_string(),
                        model: "claude-3-7-sonnet-latest".to_string(),
                        api_mode: ApiMode::AnthropicMessages,
                    },
                    FallbackProvider {
                        provider: "openai".to_string(),
                        model: "gpt-4o".to_string(),
                        api_mode: ApiMode::ChatCompletions,
                    },
                    FallbackProvider {
                        provider: "ollama".to_string(),
                        model: "hermes3:8b".to_string(),
                        api_mode: ApiMode::ChatCompletions,
                    },
                ],
            },
            memory: MemoryConfig {
                memory_md_max_chars: 2200,
                user_md_max_chars: 2200,
                enable_fts5_search: true,
                auto_create_skills: true,
                honcho_dual_peer_modeling: true,
                context_condenser_threshold_messages: 24,
                context_condenser_keep_head: 4,
                context_condenser_keep_tail: 8,
            },
            sandbox: SandboxConfig {
                default_backend: SandboxBackendType::Local,
                docker_image: "ubuntu:24.04".to_string(),
                command_timeout_secs: 60,
                require_approval_for_high_risk: true,
                allowed_paths: vec!["./workspace".to_string(), "/tmp".to_string()],
            },
            gateway: GatewayConfig {
                bind_addr: "0.0.0.0:8080".to_string(),
                heartbeat_interval_secs: 1800,
                enable_dm_pairing: true,
                authorized_users: vec!["admin".to_string(), "owner".to_string()],
            },
            openhuman: OpenHumanConfig {
                max_concurrent_agents: 512,
                agent_memory_budget_kb: 256,
                memory_tree_sync_interval_secs: 1200,
                connected_sources: vec![
                    "gmail".to_string(),
                    "github".to_string(),
                    "calendar".to_string(),
                    "slack".to_string(),
                    "notion".to_string(),
                ],
            },
            grokbot: GrokbotConfig {
                enable_role_inference: true,
                enable_teach_a_task_recorder: true,
                enable_chief_of_staff_delegation: true,
            },
        }
    }
}

impl HermesConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        if path_ref.exists() {
            let content = std::fs::read_to_string(path_ref)?;
            let cfg: HermesConfig = serde_yaml::from_str(&content)?;
            Ok(cfg)
        } else {
            Ok(Self::default())
        }
    }

    pub fn resolved_home_dir(&self) -> PathBuf {
        let raw = std::env::var("HERMES_HOME").unwrap_or_else(|_| self.hermes_home.clone());
        let base = if let Some(stripped) = raw.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(stripped)
            } else {
                PathBuf::from(".hermes")
            }
        } else {
            PathBuf::from(raw)
        };

        if self.profile == "default" {
            base
        } else {
            base.join("profiles").join(&self.profile)
        }
    }

    pub fn ensure_directories(&self) -> Result<PathBuf> {
        let home = self.resolved_home_dir();
        std::fs::create_dir_all(home.join("skills"))?;
        std::fs::create_dir_all(home.join("memory_tree/wiki"))?;
        std::fs::create_dir_all(home.join("trajectories"))?;
        std::fs::create_dir_all(home.join("plugins"))?;
        Ok(home)
    }

    pub fn resolve_runtime_provider(
        &self,
        override_provider: Option<&str>,
        override_model: Option<&str>,
    ) -> Result<ResolvedRuntimeProvider> {
        let provider = override_provider
            .unwrap_or(&self.provider.active_provider)
            .to_lowercase();
        let model = override_model
            .unwrap_or(&self.provider.active_model)
            .to_string();

        let (base_url, env_key, default_mode) = match provider.as_str() {
            "xai" | "grok" => (
                "https://api.x.ai/v1/chat/completions".to_string(),
                "XAI_API_KEY",
                ApiMode::ChatCompletions,
            ),
            "nous" | "hermes" => (
                "https://inference-api.nousresearch.com/v1/chat/completions".to_string(),
                "NOUS_API_KEY",
                ApiMode::ChatCompletions,
            ),
            "openai" => (
                "https://api.openai.com/v1/chat/completions".to_string(),
                "OPENAI_API_KEY",
                ApiMode::ChatCompletions,
            ),
            "anthropic" => (
                "https://api.anthropic.com/v1/messages".to_string(),
                "ANTHROPIC_API_KEY",
                ApiMode::AnthropicMessages,
            ),
            "ollama" | "local" => {
                let url = std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                (
                    format!("{}/v1/chat/completions", url.trim_end_matches('/')),
                    "OLLAMA_API_KEY",
                    ApiMode::ChatCompletions,
                )
            }
            other => {
                return Err(HermesError::ProviderResolution {
                    provider: other.to_string(),
                    reason: "Unsupported provider identifier".to_string(),
                })
            }
        };

        let api_key =
            std::env::var(env_key).unwrap_or_else(|_| "local-or-offline-key".to_string());
        let api_mode = if override_provider.is_some() {
            default_mode
        } else {
            self.provider.api_mode.clone()
        };

        Ok(ResolvedRuntimeProvider {
            provider_name: provider,
            model_name: model,
            api_mode,
            base_url,
            api_key,
        })
    }
}
