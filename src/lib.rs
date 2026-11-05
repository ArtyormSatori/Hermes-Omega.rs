//! # Ω Hermes Omega.rs (`hermes_omega_rs` — v1.0.0 Final Release)
//!
//! Runtime Agent Tự Trị Hợp Nhất viết bằng Rust (Edition 2021).
//! Tái cấu trúc toàn diện **Nous Hermes Agent** kết hợp:
//! - **OpenClaw** (`claw_gateway`): Multi-Channel Gateway, DM Pairing, Proactive 30m Heartbeat, >1GB Streaming API.
//! - **OpenHands** (`event_stream`): Immutable `Action` -> `Observation` EventBus, `ContextCondenser`, `spawn_subagent`.
//! - **OpenHuman** (`human_harness`): Lock-free `DashMap` 500+ Agent Harness, 20m Karpathy Memory Tree Wiki, 4-Tier Risk Gate.
//! - **Grokbot** (`grok_engine`): Named Role Inference, `TeachATaskRecorder`, Schedule/Webhook `GrokRoutine`, Multi-LLM Router.
//! - **MUSE.AI Embodiment & Monochrome ASCII** (`avatar`, `ascii_art`, `tui`, `i18n`): Realtime Vietnamese Viseme Lip-Sync Avatar, Multi-Theme TUI/Dashboard, Standard Vietnamese Localization (`Locale::Vi`).
//! - **6-Layer Defense-in-Depth Security** (`security`, `large_transfer`): Anti-Prompt Injection (Spotlighting + Canary Token + Unicode Stripper), Anti-SQL/FTS5 Injection (`rusqlite::params!` + `PRAGMA trusted_schema = OFF` + FTS5 Sanitizer), Path Traversal Jail, Constant-Time HMAC-SHA256, Shell Injection Auditor, Sliding-Window Rate Limiter, and >1GB–16GB Zero-Copy Streaming Upload/Range Download.

pub mod acp;
pub mod agent;
pub mod ascii_art;
pub mod avatar;
pub mod claw_gateway;
pub mod config;
pub mod error;
pub mod event_stream;
pub mod grok_engine;
pub mod hermes_core;
pub mod human_harness;
pub mod i18n;
pub mod large_transfer;
pub mod sandbox;
pub mod security;
pub mod tools;
pub mod tui;

pub use agent::HermesUnifiedAgent;
pub use config::HermesConfig;
pub use error::{HermesError, Result};
