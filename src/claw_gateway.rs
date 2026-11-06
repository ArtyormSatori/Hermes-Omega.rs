//! # OpenClaw Multi-Channel Gateway & >1GB Streaming API (`src/claw_gateway.rs` — v1.0.0)
//!
//! Bao gồm:
//! - Xác thực DM Pairing hằng số thời gian (`constant_time_eq`) & giới hạn tần suất `RateLimiter`.
//! - Gỡ bỏ giới hạn 2MB của Axum (`DefaultBodyLimit::disable()`) phục vụ tải lên/tải xuống luồng **>1GB – 16GB**
//!   (`/v1/files/chunk` & `/v1/files/download/:file_name` hỗ trợ `Range: bytes=start-end` HTTP 206).
//! - Endpoint `/v1/avatar/frame` phát trực tiếp trạng thái khẩu hình Tiếng Việt của **MUSE.AI Avatar**.
//! - Tiến trình nền **Proactive Heartbeat** (đọc `SOUL.md` đầu tiên mỗi 30 phút) & **Memory Tree 20m Sync**.

use crate::agent::HermesUnifiedAgent;
use crate::error::{HermesError, Result};
use crate::security::{constant_time_eq, RateLimiter};
use axum::{
    body::Bytes,
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path as AxumPath, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
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

#[derive(Debug, Deserialize)]
pub struct ChunkUploadQuery {
    pub file_name: String,
    #[serde(default)]
    pub chunk_index: usize,
    #[serde(default)]
    pub append: bool,
}

pub struct PairingRegistry {
    authorized_users: DashMap<String, bool>,
    master_pairing_code: String,
}

impl PairingRegistry {
    pub fn new(initial_users: &[String]) -> Self {
        let map = DashMap::new();
        for u in initial_users {
            map.insert(u.clone(), true);
        }
        Self {
            authorized_users: map,
            master_pairing_code: "HERMES-2026".to_string(),
        }
    }

    /// Xác thực người dùng hoặc mã ghép đôi bằng thuật toán so sánh hằng số thời gian (`constant_time_eq`)
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
            if constant_time_eq(code.as_bytes(), self.master_pairing_code.as_bytes()) {
                self.authorized_users.insert(sender_id.to_string(), true);
                info!(
                    "Paired new user '{}' on platform '{}' via constant-time DM pairing",
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
    pub rate_limiter: RateLimiter,
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
    let rate_limiter = RateLimiter::new(120, 60);
    let state = GatewayServerState {
        agent,
        pairing,
        rate_limiter,
    };

    Router::new()
        .route("/", get(dashboard_index_handler))
        .route("/health", get(health_handler))
        .route("/v1/message", post(handle_gateway_message))
        .route("/v1/fleet", get(handle_fleet_status))
        .route("/v1/skills", get(handle_list_skills))
        .route("/v1/avatar/frame", get(handle_avatar_frame))
        .route(
            "/v1/files/chunk",
            post(handle_large_chunk_upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/v1/files/download/:file_name", get(handle_range_download))
        .route("/v1/ws", get(handle_ws_upgrade))
        .with_state(state)
}

async fn dashboard_index_handler() -> Html<String> {
    Html(format!("<html><body style='background:#0c0e12;color:#e2e8f0;font-family:monospace;padding:2rem'><h2>Ω Hermes Omega.rs v{} Gateway Active</h2><p>Endpoints: /health, /v1/message, /v1/fleet, /v1/skills, /v1/avatar/frame, /v1/files/chunk (>1GB), /v1/files/download/:name (HTTP 206), /v1/ws</p></body></html>", env!("CARGO_PKG_VERSION")))
}

async fn health_handler(State(state): State<GatewayServerState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "hermes-omega-rs-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "security_layers": [
            "PromptInjectionGuard (Spotlighting + Canary)",
            "SqlSecurityGuard (Prepared + FTS5 Sanitizer)",
            "PathTraversalJail",
            "ConstantTimeHmac",
            "ShellMetacharacterGuard",
            "SlidingWindowRateLimiter"
        ],
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

async fn handle_avatar_frame(State(state): State<GatewayServerState>) -> Json<Value> {
    let frame = state.agent.avatar.lock().next_frame(None);
    Json(json!(frame))
}

async fn handle_gateway_message(
    State(state): State<GatewayServerState>,
    Json(req): Json<IncomingGatewayMessage>,
) -> std::result::Result<Json<OutgoingGatewayResponse>, (StatusCode, String)> {
    state
        .rate_limiter
        .check_and_record(&req.sender_id)
        .map_err(|e| (StatusCode::TOO_MANY_REQUESTS, e.to_string()))?;

    state
        .pairing
        .verify_or_pair(&req.platform, &req.sender_id, req.pairing_code.as_deref())
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let agent_name = req.agent_name.as_deref().unwrap_or("hermes-omega");
    let turn_result = state
        .agent
        .process_turn(agent_name, &req.platform, &req.text)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(OutgoingGatewayResponse {
        platform: req.platform,
        agent_name: agent_name.to_string(),
        inferred_role: turn_result.inferred_role,
        session_id: turn_result.session_id,
        reply: turn_result.reply,
        active_fleet_size: state.agent.harness.active_count(),
    }))
}

/// Endpoint tải lên tệp lớn (>1GB – 16GB) theo từng khối (Chunked Stream) qua BufWriter 8MB
async fn handle_large_chunk_upload(
    State(state): State<GatewayServerState>,
    Query(q): Query<ChunkUploadQuery>,
    body: Bytes,
) -> std::result::Result<Json<Value>, (StatusCode, String)> {
    let receipt = state
        .agent
        .transfer_engine
        .write_stream_chunk(&q.file_name, q.chunk_index, q.append, &body)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(receipt)))
}

/// Endpoint tải xuống nối tiếp HTTP 206 Partial Content (`Range: bytes=start-end`) cho tệp >1GB
async fn handle_range_download(
    State(state): State<GatewayServerState>,
    AxumPath(file_name): AxumPath<String>,
    headers: HeaderMap,
) -> std::result::Result<Response, (StatusCode, String)> {
    let start_offset: u64 = headers
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("bytes="))
        .and_then(|s| s.split('-').next())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let slice = state
        .agent
        .transfer_engine
        .read_range_slice(&file_name, start_offset, 4 * 1024 * 1024)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    let content_range = format!("bytes {}-{}/{}", slice.start, slice.end, slice.total_size);
    let resp = Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header("Accept-Ranges", "bytes")
        .header("Content-Range", content_range)
        .header("Content-Type", "application/octet-stream")
        .body(axum::body::Body::from(slice.data))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
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
