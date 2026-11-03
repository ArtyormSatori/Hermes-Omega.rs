use clap::{Parser, Subcommand};
use hermes_omega_rs::{
    acp::run_acp_stdio_server,
    agent::HermesUnifiedAgent,
    claw_gateway::{build_gateway_router, spawn_proactive_heartbeat_and_sync_daemons},
    config::HermesConfig,
    error::Result,
    grok_engine::{RoleInferenceEngine, TeachATaskRecorder},
    hermes_core::TrajectoryExporter,
};
use rustyline::DefaultEditor;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "hermes",
    version = env!("CARGO_PKG_VERSION"),
    about = "Hermes-RS: Unified Autonomous AI Agent in Rust (Hermes + OpenClaw + OpenHands + OpenHuman + Grokbot)"
)]
struct Cli {
    #[arg(short = 'p', long = "profile", default_value = "default")]
    profile: String,

    #[arg(short = 'c', long = "config", default_value = "config.yaml")]
    config_path: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Chat {
        #[arg(short = 'a', long = "agent", default_value = "hermes-main")]
        agent_name: String,
    },
    Daemon {
        #[arg(short = 'b', long = "bind", default_value = "0.0.0.0:8080")]
        bind: String,
    },
    Fleet {
        #[arg(long = "benchmark-count", default_value_t = 10)]
        benchmark_count: usize,
    },
    Teach {
        #[arg(help = "Task name to teach")]
        task_name: String,
    },
    Skills,
    Search {
        #[arg(help = "FTS5 search query")]
        query: String,
    },
    Acp,
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let mut cfg = HermesConfig::load_or_default(&cli.config_path)?;
    cfg.profile = cli.profile;

    let agent = Arc::new(HermesUnifiedAgent::new(cfg)?);

    match cli.command.unwrap_or(Commands::Chat {
        agent_name: "hermes-main".to_string(),
    }) {
        Commands::Chat { agent_name } => run_interactive_cli(agent, &agent_name).await?,
        Commands::Daemon { bind } => {
            spawn_proactive_heartbeat_and_sync_daemons(agent.clone());
            let app = build_gateway_router(agent);
            info!("Hermes-RS OpenClaw Gateway & Heartbeat Daemon listening on http://{}", bind);
            let listener = tokio::net::TcpListener::bind(&bind).await?;
            axum::serve(listener, app).await?;
        }
        Commands::Fleet { benchmark_count } => {
            println!("=== [OpenHuman Ultra-Lightweight Multi-Agent Harness] ===");
            for i in 1..=benchmark_count {
                let name = match i % 3 {
                    0 => format!("researchbot-{}", i),
                    1 => format!("email-triage-{}", i),
                    _ => format!("chief-of-staff-{}", i),
                };
                let role = RoleInferenceEngine::infer_from_name(&name);
                let slot = agent.harness.spawn_or_wake_agent(
                    &name,
                    &role.role_title,
                    &format!("sess-fleet-{}", i),
                )?;
                println!(
                    "  [+] Spawned {} | ID: {} | Role: {}",
                    slot.agent_name, slot.agent_id, slot.inferred_role
                );
            }
            println!(
                "Total active agents in lock-free harness: {} / {}",
                agent.harness.active_count(),
                agent.config.openhuman.max_concurrent_agents
            );
        }
        Commands::Teach { task_name } => {
            println!("=== [Grokbot 'Teach a Task' Demonstration Recorder] ===");
            let mut recorder = TeachATaskRecorder::new(&task_name);
            recorder.record_step("open_url", "https://github.com/trending", None);
            recorder.record_step(
                "cli_command",
                "cargo test --workspace",
                Some("verify all rust crates".to_string()),
            );
            let skill = recorder.compile_into_skill(&agent.skill_manager)?;
            println!(
                "Compiled demonstration into reusable Hermes Skill: {} -> {}",
                skill.name,
                skill.file_path.display()
            );
        }
        Commands::Skills => {
            let skills = agent.skill_manager.list_skills()?;
            println!("=== [Hermes Self-Improving Skills ({} total)] ===", skills.len());
            for s in skills {
                println!("- {} ({}): {}", s.name, s.file_path.display(), s.description);
            }
        }
        Commands::Search { query } => {
            let hits = agent.state_db.search_past_sessions(&query, 20)?;
            println!("=== [Hermes SQLite FTS5 Past Session Search: '{}'] ===", query);
            for h in hits {
                println!("[{} | {} | {}] {}", h.timestamp, h.session_id, h.role, h.content_snippet);
            }
        }
        Commands::Acp => {
            run_acp_stdio_server(agent).await?;
        }
        Commands::Doctor => {
            let home = agent.config.resolved_home_dir();
            let wiki_summary = agent
                .memory_wiki
                .run_sync_pass(&agent.config.openhuman.connected_sources)?;
            println!("=== [Hermes-RS Unified Doctor Report] ===");
            println!("  [OK] Profile:             {}", agent.config.profile);
            println!("  [OK] HERMES_HOME:         {}", home.display());
            println!("  [OK] Primary Provider:    {} ({})", agent.config.provider.active_provider, agent.config.provider.active_model);
            println!("  [OK] SQLite + FTS5 DB:    {}", home.join("hermes_state.sqlite3").display());
            println!("  [OK] Registered Tools:    {}", agent.tool_registry.list_tools().len());
            println!("  [OK] OpenHuman Capacity:  {} concurrent agents", agent.config.openhuman.max_concurrent_agents);
            println!("  [OK] Memory Tree Wiki:\n{}", wiki_summary);
        }
    }

    Ok(())
}

async fn run_interactive_cli(agent: Arc<HermesUnifiedAgent>, initial_agent_name: &str) -> Result<()> {
    let mut current_agent = initial_agent_name.to_string();
    let role = RoleInferenceEngine::infer_from_name(&current_agent);

    println!("=========================================================================");
    println!(" Hermes-RS v{} — The Rust Agent That Grows With You", env!("CARGO_PKG_VERSION"));
    println!(" Active Agent: {} ({})", current_agent, role.role_title);
    println!(" Opening Pitch: {}", role.opening_pitch);
    println!(" Slash commands: /agent <name>, /skills, /fleet, /condense, /export, /quit");
    println!(" Prefix any shell command with '!' (e.g. '!ls -la') to run in Sandbox.");
    println!("=========================================================================");

    let mut rl = DefaultEditor::new().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let mut turns_log: Vec<(String, String)> = Vec::new();

    loop {
        let prompt_str = format!("[{}] > ", current_agent);
        let readline = rl.readline(&prompt_str);
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(input);

                if input == "/quit" || input == "/exit" {
                    break;
                } else if let Some(new_name) = input.strip_prefix("/agent ") {
                    current_agent = new_name.trim().to_string();
                    let new_role = RoleInferenceEngine::infer_from_name(&current_agent);
                    println!(
                        "Switched to Grokbot Named Agent `{}` -> Role: {}\nPitch: {}",
                        current_agent, new_role.role_title, new_role.opening_pitch
                    );
                    continue;
                } else if input == "/skills" {
                    for s in agent.skill_manager.list_skills()? {
                        println!("  * {}: {}", s.name, s.description);
                    }
                    continue;
                } else if input == "/fleet" {
                    for slot in agent.harness.list_fleet() {
                        println!(
                            "  * {} [{}] — Role: {} (Session: {})",
                            slot.agent_name,
                            serde_json::to_string(&slot.status).unwrap_or_default(),
                            slot.inferred_role,
                            slot.active_session_id
                        );
                    }
                    continue;
                } else if input == "/condense" {
                    if let Some(summary) = agent.condenser.condense_if_needed(&agent.event_stream) {
                        println!("{}", summary);
                    } else {
                        println!("EventStream has {} events (below condensation threshold).", agent.event_stream.len());
                    }
                    continue;
                } else if input == "/export" {
                    let path = TrajectoryExporter::export_session(
                        &agent.config.resolved_home_dir(),
                        &agent.event_stream.session_id,
                        "Hermes-RS 3-Tier System Prompt",
                        &turns_log,
                    )?;
                    println!("Exported ShareGPT Trajectory to {}", path.display());
                    continue;
                }

                let turn = agent.process_turn(&current_agent, "cli", input).await?;
                turns_log.push(("user".to_string(), input.to_string()));
                turns_log.push(("assistant".to_string(), turn.reply.clone()));

                println!("\n{}\n", turn.reply);
                if let Some(notice) = turn.condensed_notice {
                    println!("{}\n", notice);
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}
