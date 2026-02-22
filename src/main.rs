#![allow(dead_code)]

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
        #[arg(long, default_value = "3100")]
        web_port: u16,
        /// Disable background web server
        #[arg(long)]
        no_web: bool,
    },
    /// Run TUI dashboard (standalone operator console)
    Tui,
    /// Run web dashboard server only
    Web {
        #[arg(long, default_value = "3100")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
            run_mcp_mode(application, web_port, no_web).await?;
        }
        None => {
            run_mcp_mode(application, 3100, false).await?;
        }
        Some(Commands::Tui) => {
            tui::run_tui(application)?;
        }
        Some(Commands::Web { port }) => {
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
    }
}
