use crate::agent::HermesUnifiedAgent;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub async fn run_acp_stdio_server(agent: Arc<HermesUnifiedAgent>) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Value::Null,
                    result: None,
                    error: Some(json!({ "code": -32700, "message": format!("Parse error: {}", e) })),
                };
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = req.id.unwrap_or(Value::Null);
        let response = match req.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: Some(json!({
                    "protocolVersion": "2026.1",
                    "serverInfo": {
                        "name": "hermes-omega-rs-acp",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "tools": true,
                        "skills": true,
                        "memoryTree": true,
                        "subAgentFleet": true
                    }
                })),
                error: None,
            },
            "agent/chat" => {
                let prompt = req.params["prompt"].as_str().unwrap_or("");
                let agent_name = req.params["agentName"].as_str().unwrap_or("hermes-ide");
                match agent.process_turn(agent_name, "acp-ide", prompt).await {
                    Ok(turn) => JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id,
                        result: Some(json!({
                            "sessionId": turn.session_id,
                            "agentName": agent_name,
                            "role": turn.inferred_role,
                            "reply": turn.reply
                        })),
                        error: None,
                    },
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id,
                        result: None,
                        error: Some(json!({ "code": -32000, "message": e.to_string() })),
                    },
                }
            }
            other => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(json!({ "code": -32601, "message": format!("Method not found: {}", other) })),
            },
        };

        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}
