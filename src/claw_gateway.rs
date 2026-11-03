use crate::agent::HermesUnifiedAgent;
use crate::error::{HermesError, Result};
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingGatewayMessage {
    pub platform: String,
    pub sender_id: String,
    pub agent_name: Option<String>,
    pub text: String,
    pub pairing_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingGatewayResponse {
    pub platform: String,
    pub agent_name: String,
    pub inferred_role: String,
    pub session_id: String,
    pub reply: String,
    pub active_fleet_size: usize,
}

pub struct PairingRegistry {
    authorized_users: DashMap<String, bool>,
    active_pairing_codes: DashMap<String, String>,
}

impl PairingRegistry {
    pub fn new(initial_users: &[String]) -> Self {
        let map = DashMap::new();
        for u in initial_users {
            map.insert(u.clone(), true);
        }
        let codes = DashMap::new();
        codes.insert("HERMES-2026".to_string(), "default-pairing-token".to_string());
        Self {
            authorized_users: map,
            active_pairing_codes: codes,
        }
    }

    pub fn verify_or_pair(
        &self,
        platform: &str,
        sender_id: &str,
        pairing_code: Option<&str>,
    ) -> Result<()> {
        if self.authorized_users.contains_key(sender_id) {
            return Ok(());
        }
        if let Some(code) = pairing_code {
            if self.active_pairing_codes.contains_key(code) {
                self.authorized_users.insert(sender_id.to_string(), true);
                info!(
                    "Paired new user '{}' on platform '{}' via DM pairing code",
                    sender_id, platform
                );
                return Ok(());
            }
        }
        Err(HermesError::UnauthorizedGateway {
            platform: platform.to_string(),
            user: sender_id.to_string(),
        })
    }
}

#[derive(Clone)]
pub struct GatewayServerState {
    pub agent: Arc<HermesUnifiedAgent>,
    pub pairing: Arc<PairingRegistry>,
}

pub fn spawn_proactive_heartbeat_and_sync_daemons(agent: Arc<HermesUnifiedAgent>) {
    let heartbeat_secs = agent.config.gateway.heartbeat_interval_secs.max(60);
    let sync_secs = agent.config.openhuman.memory_tree_sync_interval_secs.max(60);

    let hb_agent = agent.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(heartbeat_secs));
        loop {
            interval.tick().await;
            let soul = hb_agent
                .prompt_builder
                .read_bounded_file("SOUL.md", 4000, "Default SOUL.md");
            info!(
                "[OpenClaw Heartbeat] Agent woke up. Loaded SOUL.md ({} chars). Active fleet: {} agents.",
                soul.len(),
                hb_agent.harness.active_count()
            );
        }
    });

    let sync_agent = agent;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(sync_secs));
        loop {
            interval.tick().await;
            match sync_agent
                .memory_wiki
                .run_sync_pass(&sync_agent.config.openhuman.connected_sources)
            {
                Ok(summary) => {
                    info!(
                        "[OpenHuman Memory Tree] Completed 20-min sync pass ({} bytes indexed).",
                        summary.len()
                    );
                }
                Err(e) => {
                    warn!("[OpenHuman Memory Tree] Sync error: {}", e);
                }
            }
        }
    });
}

pub fn build_gateway_router(agent: Arc<HermesUnifiedAgent>) -> Router {
    let pairing = Arc::new(PairingRegistry::new(
        &agent.config.gateway.authorized_users,
    ));
    let state = GatewayServerState { agent, pairing };

    Router::new()
        .route("/health", get(health_handler))
        .route("/v1/message", post(handle_gateway_message))
        .route("/v1/fleet", get(handle_fleet_status))
        .route("/v1/skills", get(handle_list_skills))
        .route("/v1/ws", get(handle_ws_upgrade))
        .with_state(state)
}

async fn health_handler(State(state): State<GatewayServerState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "hermes-omega-rs-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "active_agents_in_harness": state.agent.harness.active_count(),
        "max_agents_capacity": state.agent.config.openhuman.max_concurrent_agents
    }))
}

async fn handle_fleet_status(State(state): State<GatewayServerState>) -> Json<Value> {
    Json(json!({
        "fleet": state.agent.harness.list_fleet()
    }))
}

async fn handle_list_skills(State(state): State<GatewayServerState>) -> Json<Value> {
    let skills = state.agent.skill_manager.list_skills().unwrap_or_default();
    Json(json!({ "skills": skills }))
}

async fn handle_gateway_message(
    State(state): State<GatewayServerState>,
    Json(req): Json<IncomingGatewayMessage>,
) -> std::result::Result<Json<OutgoingGatewayResponse>, (axum::http::StatusCode, String)> {
    state
        .pairing
        .verify_or_pair(&req.platform, &req.sender_id, req.pairing_code.as_deref())
        .map_err(|e| (axum::http::StatusCode::UNAUTHORIZED, e.to_string()))?;

    let agent_name = req.agent_name.as_deref().unwrap_or("hermes-main");
    let turn_result = state
        .agent
        .process_turn(agent_name, &req.platform, &req.text)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(OutgoingGatewayResponse {
        platform: req.platform,
        agent_name: agent_name.to_string(),
        inferred_role: turn_result.inferred_role,
        session_id: turn_result.session_id,
        reply: turn_result.reply,
        active_fleet_size: state.agent.harness.active_count(),
    }))
}

async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<GatewayServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_stream(socket, state))
}

async fn handle_ws_stream(mut socket: WebSocket, state: GatewayServerState) {
    let mut rx = state.agent.event_stream.subscribe();
    while let Ok(evt) = rx.recv().await {
        if let Ok(serialized) = serde_json::to_string(&evt) {
            if socket.send(WsMessage::Text(serialized)).await.is_err() {
                break;
            }
        }
    }
}
