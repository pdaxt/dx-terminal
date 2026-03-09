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
mod analytics;
mod quality;
mod dashboard;
mod engine;
mod scanner;
mod audit;
mod factory;
mod screen;
mod tmux;
mod session_stream;
mod build;

use std::sync::Arc;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dx", about = "DX Terminal: AI-native terminal multiplexer for AI agent teams")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as MCP server (stdio transport) — default (all 206 tools)
    Mcp {
        /// Server subset: core, queue, tracker, coord, intel (default: all)
        #[arg(value_name = "SERVER")]
        server: Option<String>,
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
    // Initialize config singleton (reads ~/.config/dx-terminal/config.json)
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
        Some(Commands::Mcp { server, web_port, no_web }) => {
            let port = web_port.unwrap_or(cfg.web_port);
            run_mcp_mode(application, port, no_web, server).await?;
        }
        None => {
            // Default: launch TUI dashboard with MCP + web running in background
            let web_app = Arc::clone(&application);
            let web_port = cfg.web_port;
            tokio::spawn(async move {
                if let Err(e) = web::run_web_server(web_app, web_port).await {
                    eprintln!("Web server error: {}", e);
                }
            });
            engine::start_background_tasks(Some(Arc::clone(&application.state))).await;

            let tui_app = application;
            let handle = std::thread::spawn(move || {
                tui::run_tui(tui_app)
            });
            handle.join().map_err(|_| anyhow::anyhow!("TUI thread panicked"))??;
        }
        Some(Commands::Tui) => {
            // TUI uses blocking_read() which panics inside tokio runtime.
            // Spawn on a dedicated OS thread outside the runtime.
            let tui_app = application;
            let handle = std::thread::spawn(move || {
                tui::run_tui(tui_app)
            });
            handle.join().map_err(|_| anyhow::anyhow!("TUI thread panicked"))??;
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

async fn run_mcp_mode(app: Arc<app::App>, web_port: u16, no_web: bool, server: Option<String>) -> anyhow::Result<()> {
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

    // Background engine: dead agent reaper, lock expiry, data retention, reconciler
    engine::start_background_tasks(Some(Arc::clone(&app.state))).await;

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

    // Gateway GC timer — shutdown idle micro MCPs every 5 minutes
    let gc_app = Arc::clone(&app);
    tokio::spawn(async move {
        let gc_interval = std::time::Duration::from_secs(300);
        let max_idle = std::time::Duration::from_secs(300);
        loop {
            tokio::time::sleep(gc_interval).await;
            let mut gw = gc_app.gateway.lock().await;
            gw.gc_idle(max_idle).await;
            let count = gw.running_count();
            if count > 0 {
                tracing::info!("Gateway GC: {} micro MCPs still running", count);
            }
        }
    });

    // Dispatch to the right server (split servers respond much faster to tools/list)
    match server.as_deref() {
        Some("core") => mcp::servers::core_server::run(app).await,
        Some("queue") => mcp::servers::queue::run(app).await,
        Some("tracker") => mcp::servers::tracker::run(app).await,
        Some("coord") => mcp::servers::coord::run(app).await,
        Some("intel") => mcp::servers::intel::run(app).await,
        Some(unknown) => {
            anyhow::bail!("Unknown MCP server '{}'. Options: core, queue, tracker, coord, intel", unknown);
        }
        None => {
            // Default: monolithic server (all 206 tools)
            mcp::run_mcp_server(app).await
        }
    }
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
