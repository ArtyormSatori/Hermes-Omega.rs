use crate::error::{HermesError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightweightAgentSlot {
    pub agent_id: String,
    pub agent_name: String,
    pub inferred_role: String,
    pub active_session_id: String,
    pub compressed_context_hash: String,
    pub status: AgentRunState,
    pub spawned_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunState {
    Idle,
    Running,
    WaitingForHumanApproval,
    SleepingOnSchedule,
}

#[derive(Clone)]
pub struct OpenHumanHarness {
    max_agents: usize,
    fleet: Arc<DashMap<String, LightweightAgentSlot>>,
}

impl OpenHumanHarness {
    pub fn new(max_agents: usize) -> Self {
        Self {
            max_agents,
            fleet: Arc::new(DashMap::new()),
        }
    }

    pub fn spawn_or_wake_agent(
        &self,
        agent_name: &str,
        inferred_role: &str,
        session_id: &str,
    ) -> Result<LightweightAgentSlot> {
        if !self.fleet.contains_key(agent_name) && self.fleet.len() >= self.max_agents {
            return Err(HermesError::HarnessCapacityReached {
                max: self.max_agents,
            });
        }

        let now = Utc::now();
        let slot = LightweightAgentSlot {
            agent_id: format!("oh-{}", uuid::Uuid::new_v4().simple()),
            agent_name: agent_name.to_string(),
            inferred_role: inferred_role.to_string(),
            active_session_id: session_id.to_string(),
            compressed_context_hash: String::new(),
            status: AgentRunState::Running,
            spawned_at: now,
            last_active_at: now,
        };

        self.fleet.insert(agent_name.to_string(), slot.clone());
        Ok(slot)
    }

    pub fn update_status(&self, agent_name: &str, state: AgentRunState) {
        if let Some(mut entry) = self.fleet.get_mut(agent_name) {
            entry.status = state;
            entry.last_active_at = Utc::now();
        }
    }

    pub fn list_fleet(&self) -> Vec<LightweightAgentSlot> {
        self.fleet.iter().map(|kv| kv.value().clone()).collect()
    }

    pub fn active_count(&self) -> usize {
        self.fleet.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTreeNode {
    pub topic_slug: String,
    pub source_integration: String,
    pub summary_markdown: String,
    pub backlinks: Vec<String>,
    pub last_synced: DateTime<Utc>,
}

pub struct MemoryTreeWiki {
    wiki_dir: PathBuf,
}

impl MemoryTreeWiki {
    pub fn new(hermes_home: &Path) -> Result<Self> {
        let wiki_dir = hermes_home.join("memory_tree").join("wiki");
        std::fs::create_dir_all(&wiki_dir)?;
        Ok(Self { wiki_dir })
    }

    pub fn upsert_wiki_node(&self, node: &MemoryTreeNode) -> Result<PathBuf> {
        let file_path = self.wiki_dir.join(format!("{}.md", node.topic_slug));
        let backlinks_str = node
            .backlinks
            .iter()
            .map(|b| format!("[[{}]]", b))
            .collect::<Vec<_>>()
            .join(" ");

        let markdown = format!(
            "---\ntopic: {}\nsource: {}\nlast_synced: {}\n---\n\n# {}\n\n{}\n\n**Backlinks:** {}\n",
            node.topic_slug,
            node.source_integration,
            node.last_synced.to_rfc3339(),
            node.topic_slug,
            node.summary_markdown,
            backlinks_str
        );
        std::fs::write(&file_path, markdown)?;
        Ok(file_path)
    }

    pub fn run_sync_pass(&self, sources: &[String]) -> Result<String> {
        let now = Utc::now();
        for src in sources {
            let slug = format!("sync_{}", src);
            let target = self.wiki_dir.join(format!("{}.md", slug));
            if !target.exists() {
                let node = MemoryTreeNode {
                    topic_slug: slug,
                    source_integration: src.clone(),
                    summary_markdown: format!(
                        "Compressed knowledge index for `{}` integration (auto-synced by OpenHuman Memory Tree).",
                        src
                    ),
                    backlinks: vec!["index".to_string()],
                    last_synced: now,
                };
                self.upsert_wiki_node(&node)?;
            }
        }
        self.compile_compressed_index()
    }

    pub fn compile_compressed_index(&self) -> Result<String> {
        let mut summaries = Vec::new();
        if self.wiki_dir.exists() {
            for entry in std::fs::read_dir(&self.wiki_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let topic = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let content = std::fs::read_to_string(&path)?;
                    let first_body_line = content
                        .lines()
                        .skip_while(|l| !l.starts_with("# "))
                        .nth(1)
                        .unwrap_or("Indexed wiki node");
                    summaries.push(format!("- [[{}]]: {}", topic, first_body_line));
                }
            }
        }
        if summaries.is_empty() {
            Ok("Memory Tree Wiki is initialized and ready for 20-minute sync passes.".to_string())
        } else {
            Ok(summaries.join("\n"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskTier {
    Low,
    Medium,
    High,
    Critical,
}

pub struct HumanApprovalGate {
    pub require_approval_for_high_risk: bool,
}

impl HumanApprovalGate {
    pub fn new(require_approval_for_high_risk: bool) -> Self {
        Self {
            require_approval_for_high_risk,
        }
    }

    pub fn classify_command_risk(&self, command: &str) -> RiskTier {
        let lower = command.to_lowercase();
        let critical_patterns = [
            "rm -rf /",
            "mkfs",
            "dd if=",
            ":(){:|:&};:",
            "chmod -r 777 /",
            "shutdown",
            "reboot",
        ];
        if critical_patterns.iter().any(|p| lower.contains(p)) {
            return RiskTier::Critical;
        }

        let high_patterns = [
            "sudo ",
            "rm -rf",
            "curl ",
            "wget ",
            "docker run --privileged",
            "ssh ",
            "scp ",
            "git push --force",
            "drop table",
        ];
        if high_patterns.iter().any(|p| lower.contains(p)) {
            return RiskTier::High;
        }

        let medium_patterns = ["git commit", "cargo publish", "npm publish", "kill "];
        if medium_patterns.iter().any(|p| lower.contains(p)) {
            return RiskTier::Medium;
        }

        RiskTier::Low
    }

    pub fn verify_command_allowed(&self, command: &str) -> Result<RiskTier> {
        let risk = self.classify_command_risk(command);
        if risk == RiskTier::Critical {
            return Err(HermesError::SandboxViolation(format!(
                "Critical destructive command blocked by HumanApprovalGate: `{}`",
                command
            )));
        }
        if self.require_approval_for_high_risk && risk == RiskTier::High {
            let auto_approve = std::env::var("HERMES_AUTO_APPROVE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if !auto_approve {
                return Err(HermesError::ApprovalRejected {
                    action: command.to_string(),
                    reason: "High-risk command requires human approval (set HERMES_AUTO_APPROVE=true or approve interactively)".to_string(),
                });
            }
        }
        Ok(risk)
    }
}
