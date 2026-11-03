# Hermes-Omega.rs

Unified autonomous AI agent runtime written in Rust (Edition 2021). Re-architects **Nous Hermes Agent** into a single static binary and integrates core runtime mechanisms from **OpenClaw**, **OpenHands**, **OpenHuman**, and **Grokbot**.

- **Repository**: `https://github.com/ArtyormSatori/Hermes-Omega.rs`
- **Crate / Binary**: `hermes_rs` (lib) / `hermes` (bin)
- **License**: MIT

## 1. System Architecture

```text
+-----------------------------------------------------------------------------------+
|                               Ingress & Interfaces                                |
|  CLI REPL (`hermes chat`) | HTTP/WS Gateway (`claw_gateway`) | IDE ACP (`acp.rs`) |
+-----------------------------------------+-----------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
|                        HermesUnifiedAgent (`src/agent.rs`)                        |
|                                                                                   |
|  +-------------------------+  +-------------------------+  +-------------------+  |
|  | 3-Tier Prompt Builder   |  | OpenHands EventStream   |  | Multi-LLM Router  |  |
|  | Stable -> Context ->    |  | Action -> Observation   |  | ChatCompletions   |  |
|  | Volatile (`SOUL.md`,    |  | Tokio Broadcast Bus +   |  | CodexResponses    |  |
|  | `MEMORY.md`, `USER.md`) |  | ContextCondenser        |  | AnthropicMessages |  |
|  +-------------------------+  +-------------------------+  +-------------------+  |
+-----------------+-----------------------+-----------------------+-----------------+
                  |                       |                       |
                  v                       v                       v
+------------------------+  +---------------------------+  +------------------------+
| Persistence & Learning |  | Fleet & Knowledge Sync    |  | Execution & Security   |
| - SQLite WAL + FTS5    |  | - OpenHuman DashMap Fleet |  | - SandboxRuntime       |
| - Honcho Dual-Peer     |  |   (500+ concurrent slots) |  |   (Local/Docker/SSH/   |
| - Auto `SKILL.md` Loop |  | - 20m Memory Tree Wiki    |  |    Firecracker)        |
| - ShareGPT Exporter    |  | - Grokbot Role Inference  |  | - 4-Tier Risk Gate     |
|                        |  |   & Teach-a-Task Recorder |  |   (`HumanApprovalGate`)|
+------------------------+  +---------------------------+  +------------------------+
```

## 2. Subsystem Specification

| Subsystem | Source Module | Engineering Mechanism |
| :--- | :--- | :--- |
| **Hermes Core** | `src/hermes_core.rs` | 3-tier system prompt assembly (prefix cache-aligned); SQLite WAL + FTS5 full-text session search (`messages_fts`); Honcho dual-peer state table; closed learning loop writing YAML-frontmatter `SKILL.md` files; ShareGPT JSON trajectory exporter. |
| **Event Stream & Condenser** | `src/event_stream.rs` | Typed `AgentAction` and `AgentObservation` enums over a 1024-slot `tokio::sync::broadcast` bus; `ContextCondenser` retains `keep_head` and `keep_tail` events while summarizing intermediate turns when history exceeds threshold. |
| **Multi-Channel Gateway** | `src/claw_gateway.rs` | Axum HTTP/WebSocket server (`/health`, `/v1/message`, `/v1/fleet`, `/v1/skills`, `/v1/ws`); DM pairing token verification (`PairingRegistry`); background heartbeat task waking on interval and reading `SOUL.md` before state checks. |
| **Fleet Harness & Memory Tree** | `src/human_harness.rs` | Lock-free `DashMap<String, LightweightAgentSlot>` harness supporting 500+ concurrent agent states in a single process; 20-minute sync pass compiling external sources into interlinked Markdown wiki nodes (`[[backlinks]]`); 4-tier command risk classifier (`Low`, `Medium`, `High`, `Critical`). |
| **Role Inference & LLM Router** | `src/grok_engine.rs` | Deterministic role, plugin, and routine inference from agent name (`researchbot`, `email-triage`, `chief-of-staff`); `TeachATaskRecorder` compiling recorded step sequences into `SKILL.md`; multi-provider HTTP client with automatic fallback across `xAI`, `Anthropic`, `OpenAI`, `Nous`, and `Ollama`. |
| **Sandbox & Tool Registry** | `src/sandbox.rs`, `src/tools.rs` | Process execution across `Local`, `Docker` (`--network none --memory 512m`), `SSH`, and `Firecracker`; regex ANSI escape stripper; background PID tracking; JSON Schema tool registry (`terminal_exec`, `file_op`, `spawn_subagent`, `save_skill`). |
| **IDE ACP Server** | `src/acp.rs` | Stdio JSON-RPC 2.0 server (`initialize`, `agent/chat`) for VS Code, Zed, and JetBrains integration. |

## 3. Build & Verification

```bash
cargo test -- --nocapture
cargo build --release
./target/release/hermes doctor
```

## 4. Runtime Usage

```bash
# Interactive REPL with a role-inferred agent
./target/release/hermes chat --agent researchbot

# Benchmark lock-free OpenHuman fleet harness (50 concurrent agent slots)
./target/release/hermes fleet --benchmark-count 50

# Compile a recorded task demonstration into ~/.hermes/skills/<name>/SKILL.md
./target/release/hermes teach deploy-rust-service

# Full-text search across historical sessions via SQLite FTS5
./target/release/hermes search "memory safety"

# Start HTTP/WebSocket Gateway + Proactive Heartbeat + 20m Memory Tree Sync
./target/release/hermes daemon --bind 0.0.0.0:8080
```

## 5. Container Deployment

```bash
export XAI_API_KEY="your_key"
export ANTHROPIC_API_KEY="your_key"
docker compose up --build -d
curl -s http://localhost:8080/health
```
