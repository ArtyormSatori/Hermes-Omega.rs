use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type", rename_all = "snake_case")]
pub enum AgentAction {
    Message {
        role: String,
        content: String,
    },
    RunTerminalCommand {
        command: String,
        cwd: Option<String>,
        background: bool,
    },
    FileEdit {
        path: String,
        operation: String,
        content: Option<String>,
    },
    BrowseWeb {
        url: String,
        query: Option<String>,
    },
    SpawnSubAgent {
        agent_name: String,
        role_hint: String,
        task_prompt: String,
    },
    SaveSkill {
        skill_name: String,
        description: String,
        instructions_markdown: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "observation_type", rename_all = "snake_case")]
pub enum AgentObservation {
    CommandOutput {
        command: String,
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    FileResult {
        path: String,
        success: bool,
        data: String,
    },
    WebResult {
        url: String,
        snippet: String,
    },
    SubAgentCompleted {
        agent_name: String,
        summary_output: String,
    },
    SkillSaved {
        skill_name: String,
        file_path: String,
    },
    CondensationSummary {
        condensed_range: (usize, usize),
        summary_text: String,
    },
    Error {
        source: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub id: String,
    pub session_id: String,
    pub sequence_num: usize,
    pub timestamp: DateTime<Utc>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventPayload {
    Action(AgentAction),
    Observation(AgentObservation),
}

#[derive(Clone)]
pub struct EventStream {
    pub session_id: String,
    history: Arc<RwLock<Vec<StreamEvent>>>,
    tx: broadcast::Sender<StreamEvent>,
}

impl EventStream {
    pub fn new(session_id: impl Into<String>) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            session_id: session_id.into(),
            history: Arc::new(RwLock::new(Vec::new())),
            tx,
        }
    }

    pub fn publish(&self, payload: EventPayload) -> StreamEvent {
        let mut guard = self.history.write();
        let seq = guard.len() + 1;
        let evt = StreamEvent {
            id: Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            sequence_num: seq,
            timestamp: Utc::now(),
            payload,
        };
        guard.push(evt.clone());
        let _ = self.tx.send(evt.clone());
        evt
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    pub fn snapshot(&self) -> Vec<StreamEvent> {
        self.history.read().clone()
    }

    pub fn len(&self) -> usize {
        self.history.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.read().is_empty()
    }

    pub fn replace_history(&self, condensed: Vec<StreamEvent>) {
        let mut guard = self.history.write();
        *guard = condensed;
    }
}

pub struct ContextCondenser {
    pub threshold_events: usize,
    pub keep_head: usize,
    pub keep_tail: usize,
}

impl ContextCondenser {
    pub fn new(threshold_events: usize, keep_head: usize, keep_tail: usize) -> Self {
        Self {
            threshold_events,
            keep_head,
            keep_tail,
        }
    }

    pub fn condense_if_needed(&self, stream: &EventStream) -> Option<String> {
        let events = stream.snapshot();
        if events.len() <= self.threshold_events
            || events.len() <= (self.keep_head + self.keep_tail + 2)
        {
            return None;
        }

        let mid_start = self.keep_head;
        let mid_end = events.len() - self.keep_tail;
        let middle_slice = &events[mid_start..mid_end];

        let mut bullet_points = Vec::new();
        for ev in middle_slice {
            match &ev.payload {
                EventPayload::Action(AgentAction::RunTerminalCommand { command, .. }) => {
                    bullet_points.push(format!("- Executed command: `{}`", command));
                }
                EventPayload::Action(AgentAction::FileEdit {
                    path, operation, ..
                }) => {
                    bullet_points.push(format!("- File {} on `{}`", operation, path));
                }
                EventPayload::Action(AgentAction::Message { role, content }) => {
                    let preview: String = content.chars().take(100).collect();
                    bullet_points.push(format!("- [{}] {}", role, preview));
                }
                EventPayload::Observation(AgentObservation::CommandOutput {
                    exit_code, ..
                }) => {
                    bullet_points.push(format!("  -> Command finished with exit_code={}", exit_code));
                }
                _ => {}
            }
        }

        let summary_text = format!(
            "[OpenHands Context Condenser: Condensed {} intermediate events (#{}..#{})]\n{}",
            middle_slice.len(),
            mid_start + 1,
            mid_end,
            bullet_points.join("\n")
        );

        let mut new_events = Vec::with_capacity(self.keep_head + 1 + self.keep_tail);
        new_events.extend_from_slice(&events[..mid_start]);
        new_events.push(StreamEvent {
            id: Uuid::new_v4().to_string(),
            session_id: stream.session_id.clone(),
            sequence_num: mid_start + 1,
            timestamp: Utc::now(),
            payload: EventPayload::Observation(AgentObservation::CondensationSummary {
                condensed_range: (mid_start + 1, mid_end),
                summary_text: summary_text.clone(),
            }),
        });
        new_events.extend_from_slice(&events[mid_end..]);

        for (idx, item) in new_events.iter_mut().enumerate() {
            item.sequence_num = idx + 1;
        }

        stream.replace_history(new_events);
        Some(summary_text)
    }
}
