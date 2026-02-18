#![allow(dead_code)]

mod app;
mod config;
mod claude;
mod tracker;
mod capacity;
mod state;
mod mcp;
mod pty;
mod tui;
mod web;

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
    /// Run as MCP server (stdio transport)
    Mcp,
    /// Run TUI dashboard
    Tui,
    /// Run web dashboard server
    Web {
        #[arg(long, default_value = "3100")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr (stdout is MCP transport in mcp mode)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    let application = Arc::new(app::App::new());

    match cli.command {
        Some(Commands::Mcp) | None => {
            // Default to MCP mode
            mcp::run_mcp_server(application).await?;
        }
        Some(Commands::Tui) => {
            eprintln!("TUI mode coming in Phase 4");
        }
        Some(Commands::Web { port }) => {
            eprintln!("Web mode coming in Phase 3 (port {})", port);
        }
    }

    Ok(())
}
