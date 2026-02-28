mod app;
mod config;
mod claude;
mod tracker;
mod capacity;
mod state;
mod mcp;
mod mcp_registry;
mod pty;
mod tui;
mod web;
mod workspace;
mod queue;
mod multi_agent;
mod collab;
mod knowledge;
mod machine;

use std::sync::Arc;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agentos", about = "AgentOS: AI agent orchestration runtime")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as MCP server (stdio transport) — default
    Mcp {
        /// Also start web dashboard in background
        #[arg(long)]
        web_port: Option<u16>,
        /// Disable background web server
        #[arg(long)]
        no_web: bool,
    },
    /// Run TUI dashboard (standalone operator console)
    Tui,
    /// Run web dashboard server only
    Web {
        #[arg(long)]
        port: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize config singleton (reads ~/.config/agentos/config.json)
    let cfg = config::init();

    let cli = Cli::parse();
    let application = Arc::new(app::App::new());

    // Clean up stale worktrees from previous crashed sessions
    if let Ok(cleaned) = workspace::cleanup_stale_worktrees() {
        if !cleaned.is_empty() {
            eprintln!("Cleaned {} stale worktrees", cleaned.len());
        }
    }

    // Graceful shutdown: kill all PTY children when process exits
    let shutdown_app = Arc::clone(&application);
    let _shutdown_guard = ShutdownGuard(shutdown_app);

    match cli.command {
        Some(Commands::Mcp { web_port, no_web }) => {
            let port = web_port.unwrap_or(cfg.web_port);
            run_mcp_mode(application, port, no_web).await?;
        }
        None => {
            run_mcp_mode(application, cfg.web_port, false).await?;
        }
        Some(Commands::Tui) => {
            tui::run_tui(application)?;
        }
        Some(Commands::Web { port }) => {
            let port = port.unwrap_or(cfg.web_port);
            init_tracing();
            tracing::info!("Web dashboard at http://localhost:{}", port);
            web::run_web_server(application, port).await?;
        }
    }

    Ok(())
}

async fn run_mcp_mode(app: Arc<app::App>, web_port: u16, no_web: bool) -> anyhow::Result<()> {
    init_tracing();

    if !no_web {
        let web_app = Arc::clone(&app);
        tokio::spawn(async move {
            if let Err(e) = web::run_web_server(web_app, web_port).await {
                tracing::warn!("Web server error: {}", e);
            }
        });
        tracing::info!("Web dashboard at http://localhost:{}", web_port);
    }

    // Background auto-cycle timer — reads interval from config, runs auto_cycle periodically
    let cycle_app = Arc::clone(&app);
    tokio::spawn(async move {
        // Initial delay to let MCP server start
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        loop {
            let cfg = queue::load_auto_config();
            if cfg.cycle_interval_secs == 0 {
                // Disabled — check again in 30s in case config changes
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }

            let interval = std::time::Duration::from_secs(cfg.cycle_interval_secs);
            tokio::time::sleep(interval).await;

            let result = mcp::tools::auto_cycle(&cycle_app).await;
            // Only log if something happened (not just empty cycle)
            if result.contains("auto_complete") || result.contains("auto_spawn") || result.contains("error_kill") {
                tracing::info!("Auto-cycle: {}", result);
            }
        }
    });

    mcp::run_mcp_server(app).await
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}

/// RAII guard that kills all PTY children on drop (process exit)
struct ShutdownGuard(Arc<app::App>);

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        if let Ok(mut pty) = self.0.pty.lock() {
            pty.kill_all();
        }
        machine::deregister_all();
    }
}
