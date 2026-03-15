use clap::{Parser, Subcommand};
use dx_terminal::{
    agent_router, app, config, dxos_scheduler, dxos_supervisor, engine, go, ipc, machine, mcp,
    queue, swarm, sync, tui, web, workspace,
};
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "dx",
    about = "DX Terminal: AI-native terminal multiplexer for AI agent teams"
)]
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
    /// Zero-config project launch: tmux session, issues, agents, dashboard
    Go(go::GoArgs),
    /// Issue-to-PR swarm orchestration
    Swarm {
        #[command(subcommand)]
        command: SwarmCommands,
    },
    /// Provider-neutral agent routing
    Router {
        #[command(subcommand)]
        command: RouterCommands,
    },
    /// Use external MCP servers configured outside dx through the gateway bridge
    Gateway {
        #[command(subcommand)]
        command: GatewayCommands,
    },
}

#[derive(Subcommand)]
enum SwarmCommands {
    /// Start a swarm for open GitHub issues in the current repository
    Start {
        #[arg(long)]
        repo: String,
        #[arg(long, default_value_t = 5)]
        max_agents: usize,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long, default_value = "claude")]
        provider: String,
    },
    /// Show the current swarm state
    Status,
    /// Stop the current swarm and clean up worktrees
    Stop,
}

#[derive(Subcommand)]
enum RouterCommands {
    /// Recommend the best provider for a task
    Route {
        description: String,
        #[arg(long)]
        language: Option<String>,
    },
    /// Show provider usage statistics and cost history
    Stats,
    /// Show cost-per-provider summary
    Cost,
    /// Add a custom regex rule
    AddRule {
        pattern: String,
        provider: String,
        reason: String,
    },
}

#[derive(Subcommand)]
enum GatewayCommands {
    /// List all registered gateway MCPs
    List {
        #[arg(long)]
        running_only: bool,
    },
    /// Discover MCPs by capability keyword
    Discover {
        capability: String,
        #[arg(long)]
        auto_start: bool,
    },
    /// Inspect the tools exposed by one gateway MCP
    Tools { mcp: String },
    /// Call a tool on a gateway MCP
    Call {
        mcp: String,
        tool: String,
        #[arg(long)]
        args: Option<String>,
    },
}

fn runtime_identity(cli: &Cli, default_web_port: u16) -> String {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    let cwd_hash = hasher.finish();

    match cli.command.as_ref() {
        Some(Commands::Mcp {
            server,
            web_port,
            no_web,
        }) => format!(
            "mcp-{}-{}-{}-{:x}",
            server.as_deref().unwrap_or("all"),
            if *no_web { "noweb" } else { "web" },
            web_port.unwrap_or(default_web_port),
            cwd_hash,
        ),
        Some(Commands::Tui) => format!("tui-{:x}", cwd_hash),
        Some(Commands::Web { port }) => {
            format!("web-{}-{:x}", port.unwrap_or(default_web_port), cwd_hash)
        }
        Some(Commands::Go(args)) => {
            format!("go-{}-{}-{:x}", args.agents, args.max_issues, cwd_hash)
        }
        Some(Commands::Swarm { command }) => match command {
            SwarmCommands::Start {
                repo, max_agents, ..
            } => format!("swarm-start-{}-{}-{:x}", repo, max_agents, cwd_hash),
            SwarmCommands::Status => format!("swarm-status-{:x}", cwd_hash),
            SwarmCommands::Stop => format!("swarm-stop-{:x}", cwd_hash),
        },
        Some(Commands::Router { command }) => match command {
            RouterCommands::Route { description, .. } => {
                format!("router-route-{}-{:x}", description, cwd_hash)
            }
            RouterCommands::Stats => format!("router-stats-{:x}", cwd_hash),
            RouterCommands::Cost => format!("router-cost-{:x}", cwd_hash),
            RouterCommands::AddRule {
                provider, pattern, ..
            } => format!("router-rule-{}-{}-{:x}", provider, pattern, cwd_hash),
        },
        Some(Commands::Gateway { command }) => match command {
            GatewayCommands::List { .. } => format!("gateway-list-{:x}", cwd_hash),
            GatewayCommands::Discover { capability, .. } => {
                format!("gateway-discover-{}-{:x}", capability, cwd_hash)
            }
            GatewayCommands::Tools { mcp } => format!("gateway-tools-{}-{:x}", mcp, cwd_hash),
            GatewayCommands::Call { mcp, tool, .. } => {
                format!("gateway-call-{}-{}-{:x}", mcp, tool, cwd_hash)
            }
        },
        None => format!("default-{}-{:x}", default_web_port, cwd_hash),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize config singleton (reads ~/.config/dx-terminal/config.json)
    let cfg = config::init();

    let cli = Cli::parse();
    let application = Arc::new(app::App::new());
    let _ipc_guard = ipc::start_local_ipc(
        Arc::clone(&application),
        runtime_identity(&cli, cfg.web_port),
    );

    // Clean up stale worktrees from previous crashed sessions
    if let Ok(cleaned) = workspace::cleanup_stale_worktrees() {
        if !cleaned.is_empty() {
            eprintln!("Cleaned {} stale worktrees", cleaned.len());
        }
    }

    // Graceful shutdown: kill all PTY children when process exits
    let shutdown_app = Arc::clone(&application);
    let _shutdown_guard = ShutdownGuard(shutdown_app);

    // Start sync manager for current directory (if it's a git repo)
    start_sync_manager(&application).await;

    match cli.command {
        Some(Commands::Mcp {
            server,
            web_port,
            no_web,
        }) => {
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
            dxos_scheduler::start(Arc::clone(&application));
            dxos_supervisor::start(Arc::clone(&application));

            let tui_app = application;
            let handle = std::thread::spawn(move || tui::run_tui(tui_app));
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("TUI thread panicked"))??;
        }
        Some(Commands::Tui) => {
            // TUI uses blocking_read() which panics inside tokio runtime.
            // Spawn on a dedicated OS thread outside the runtime.
            dxos_scheduler::start(Arc::clone(&application));
            dxos_supervisor::start(Arc::clone(&application));
            let tui_app = application;
            let handle = std::thread::spawn(move || tui::run_tui(tui_app));
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("TUI thread panicked"))??;
        }
        Some(Commands::Web { port }) => {
            let port = port.unwrap_or(cfg.web_port);
            init_tracing();
            tracing::info!("Web dashboard at http://localhost:{}", port);
            dxos_scheduler::start(Arc::clone(&application));
            dxos_supervisor::start(Arc::clone(&application));
            web::run_web_server(application, port).await?;
        }
        Some(Commands::Go(args)) => {
            init_tracing();
            go::go(application, args).await?;
        }
        Some(Commands::Swarm { command }) => {
            init_tracing();
            run_swarm_cli(application, command).await?;
        }
        Some(Commands::Router { command }) => {
            run_router_cli(command)?;
        }
        Some(Commands::Gateway { command }) => {
            run_gateway_cli(application, command).await?;
        }
    }

    Ok(())
}

async fn run_gateway_cli(app: Arc<app::App>, command: GatewayCommands) -> anyhow::Result<()> {
    let output = match command {
        GatewayCommands::List { running_only } => {
            mcp::tools::gateway_tools::gateway_list(
                &app,
                mcp::types::GatewayListRequest {
                    running_only: Some(running_only),
                },
            )
            .await
        }
        GatewayCommands::Discover {
            capability,
            auto_start,
        } => {
            mcp::tools::gateway_tools::gateway_discover(
                &app,
                mcp::types::GatewayDiscoverRequest {
                    capability,
                    auto_start: Some(auto_start),
                },
            )
            .await
        }
        GatewayCommands::Tools { mcp } => {
            mcp::tools::gateway_tools::gateway_tools(
                &app,
                mcp::types::GatewayToolsRequest {
                    mcp,
                    auto_start: Some(true),
                },
            )
            .await
        }
        GatewayCommands::Call { mcp, tool, args } => {
            let parsed_args = match args {
                Some(raw) => Some(
                    serde_json::from_str::<Value>(&raw)
                        .map_err(|e| anyhow::anyhow!("Invalid --args JSON: {}", e))?,
                ),
                None => None,
            };
            mcp::tools::gateway_tools::gateway_call(
                &app,
                mcp::types::GatewayCallRequest {
                    mcp,
                    tool,
                    arguments: parsed_args,
                },
            )
            .await
        }
    };

    let value = serde_json::from_str::<Value>(&output).unwrap_or_else(|_| json!({ "raw": output }));
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

async fn run_swarm_cli(app: Arc<app::App>, command: SwarmCommands) -> anyhow::Result<()> {
    let value = match command {
        SwarmCommands::Start {
            repo,
            max_agents,
            labels,
            provider,
        } => serde_json::to_value(
            swarm::start(
                app,
                swarm::SwarmConfig {
                    repo,
                    max_agents,
                    issue_labels: labels,
                    agent_provider: provider,
                },
            )
            .await?,
        )?,
        SwarmCommands::Status => serde_json::to_value(swarm::status(app.as_ref()).await?)?,
        SwarmCommands::Stop => serde_json::to_value(swarm::stop(app.as_ref()).await?)?,
    };

    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn run_router_cli(command: RouterCommands) -> anyhow::Result<()> {
    let value = match command {
        RouterCommands::Route {
            description,
            language,
        } => serde_json::to_value(agent_router::route_task(&description, language.as_deref())?)?,
        RouterCommands::Stats => agent_router::agent_stats()?,
        RouterCommands::Cost => agent_router::cost_report()?,
        RouterCommands::AddRule {
            pattern,
            provider,
            reason,
        } => serde_json::to_value(agent_router::add_routing_rule(
            &pattern, &provider, &reason,
        )?)?,
    };

    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

async fn run_mcp_mode(
    app: Arc<app::App>,
    web_port: u16,
    no_web: bool,
    server: Option<String>,
) -> anyhow::Result<()> {
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
    dxos_scheduler::start(Arc::clone(&app));
    dxos_supervisor::start(Arc::clone(&app));

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
            if result.contains("auto_complete")
                || result.contains("auto_spawn")
                || result.contains("error_kill")
            {
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
            anyhow::bail!(
                "Unknown MCP server '{}'. Options: core, queue, tracker, coord, intel",
                unknown
            );
        }
        None => {
            // Default: monolithic server (all 206 tools)
            mcp::run_mcp_server(app).await
        }
    }
}

/// Start the Rust-native sync manager for file watching + auto git sync
async fn start_sync_manager(app: &Arc<app::App>) {
    let cwd = std::env::current_dir().unwrap_or_default();
    // Only start if current dir is a git repo
    let is_git = cwd.join(".git").exists();
    if !is_git {
        return;
    }

    let project_name = cwd
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    let config = sync::SyncConfig {
        root: cwd,
        project: project_name,
        ..sync::SyncConfig::default()
    };

    let mgr = Arc::new(sync::SyncManager::new(config));
    let mgr_clone = Arc::clone(&mgr);

    // Store in app
    {
        let mut sync_lock = app.sync_manager.write().unwrap();
        *sync_lock = Some(Arc::clone(&mgr));
    }

    // Start the sync system
    tokio::spawn(async move {
        if let Err(e) = mgr_clone.start().await {
            tracing::error!("Sync manager error: {}", e);
        }
    });
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
