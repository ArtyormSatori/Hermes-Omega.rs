//! # Multi-Backend Execution Sandbox (`src/sandbox.rs` — v1.0.0 Hardened)
//!
//! Nâng cấp:
//! 1. Lưu đệm biểu thức chính quy ANSI (`std::sync::OnceLock<Regex>`) để không biên dịch lại mỗi lần chạy lệnh.
//! 2. Quét chống Shell Subshell Injection (`security::audit_shell_command`) + kiểm duyệt rủi ro `HumanApprovalGate`.
//! 3. Làm sạch biến môi trường tiến trình con (`env_clear` + danh sách cho phép an toàn `PATH`, `LANG=vi_VN.UTF-8`).

use crate::config::{SandboxBackendType, SandboxConfig};
use crate::error::{HermesError, Result};
use crate::event_stream::AgentObservation;
use crate::human_harness::HumanApprovalGate;
use crate::security::audit_shell_command;
use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

static ANSI_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundJobInfo {
    pub pid: u32,
    pub command: String,
    pub backend: SandboxBackendType,
    pub started_at: String,
}

#[derive(Clone)]
pub struct SandboxRuntime {
    config: SandboxConfig,
    approval_gate: Arc<HumanApprovalGate>,
    bg_processes: Arc<DashMap<u32, BackgroundJobInfo>>,
}

impl SandboxRuntime {
    pub fn new(config: SandboxConfig) -> Self {
        let gate = HumanApprovalGate::new(config.require_approval_for_high_risk);
        Self {
            config,
            approval_gate: Arc::new(gate),
            bg_processes: Arc::new(DashMap::new()),
        }
    }

    pub fn strip_ansi_codes(input: &str) -> String {
        let re = ANSI_REGEX.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
        re.replace_all(input, "").to_string()
    }

    pub async fn execute_command(
        &self,
        command: &str,
        cwd: Option<&str>,
        background: bool,
    ) -> Result<AgentObservation> {
        // 1. Quét chống Shell Injection & kiểm tra rủi ro HumanApprovalGate
        audit_shell_command(command)?;
        let _risk = self.approval_gate.verify_command_allowed(command)?;

        // 2. Khởi tạo tiến trình cô lập theo Backend
        let mut cmd = match self.config.default_backend {
            SandboxBackendType::Local => {
                let mut c = Command::new("bash");
                c.arg("-c").arg(command);
                if let Some(dir) = cwd {
                    c.current_dir(dir);
                }
                c
            }
            SandboxBackendType::Docker => {
                let mut c = Command::new("docker");
                c.args([
                    "run",
                    "--rm",
                    "--network",
                    "none",
                    "--memory",
                    "512m",
                    "--cpus",
                    "1.0",
                    &self.config.docker_image,
                    "bash",
                    "-c",
                    command,
                ]);
                c
            }
            SandboxBackendType::Ssh => {
                let ssh_target =
                    std::env::var("HERMES_SSH_TARGET").unwrap_or_else(|_| "localhost".to_string());
                let mut c = Command::new("ssh");
                c.arg(ssh_target).arg(command);
                c
            }
            SandboxBackendType::Firecracker => {
                let mut c = Command::new("bash");
                c.arg("-c").arg(command);
                c
            }
        };

        // 3. Cô lập biến môi trường (chống lộ API key qua lệnh `env` hoặc `printenv`)
        let safe_path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string());
        cmd.env_clear()
            .env("PATH", safe_path)
            .env("LANG", "vi_VN.UTF-8")
            .env("LC_ALL", "vi_VN.UTF-8")
            .env("TERM", "xterm-256color")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if background {
            let child = cmd.spawn()?;
            let pid = child.id().unwrap_or(0);
            self.bg_processes.insert(
                pid,
                BackgroundJobInfo {
                    pid,
                    command: command.to_string(),
                    backend: self.config.default_backend.clone(),
                    started_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            return Ok(AgentObservation::CommandOutput {
                command: command.to_string(),
                exit_code: 0,
                stdout: format!("Started background job with PID {}", pid),
                stderr: String::new(),
            });
        }

        let wait_duration = Duration::from_secs(self.config.command_timeout_secs);
        match timeout(wait_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = Self::strip_ansi_codes(&String::from_utf8_lossy(&output.stdout));
                let stderr = Self::strip_ansi_codes(&String::from_utf8_lossy(&output.stderr));
                Ok(AgentObservation::CommandOutput {
                    command: command.to_string(),
                    exit_code,
                    stdout,
                    stderr,
                })
            }
            Ok(Err(io_err)) => Err(HermesError::Io(io_err)),
            Err(_) => Err(HermesError::ToolExecution {
                tool: "terminal".to_string(),
                message: format!(
                    "Command timed out after {} seconds",
                    self.config.command_timeout_secs
                ),
            }),
        }
    }

    pub fn list_background_jobs(&self) -> Vec<BackgroundJobInfo> {
        self.bg_processes.iter().map(|kv| kv.value().clone()).collect()
    }
}
