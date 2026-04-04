use clap::{Parser, Subcommand};
use dx_terminal::{
    agent_display, agent_prompt, agent_repl, agent_router, agent_setup, app, config,
    dxos_scheduler, dxos_supervisor, engine, go, ipc, machine, mcp, queue, services, swarm, sync,
    tui, web, workspace,
};
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(
    name = "dx",
    about = "DX Terminal: AI agent OS — code, orchestrate, ship"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
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
    /// Run external tool commands imported from Claude, Codex, and other runtimes
    #[command(visible_alias = "tools")]
    External {
        #[command(subcommand)]
        command: ExternalCommands,
    },
    /// CLI-first service catalog for internal and external microservices
    #[command(visible_alias = "svc")]
    Services {
        #[command(subcommand)]
        command: ServiceCommands,
    },
    /// Compatibility alias for the older gateway surface
    #[command(hide = true)]
    Gateway {
        #[command(subcommand)]
        command: ExternalCommands,
    },
    /// Run local CI gate (cargo check + test + clippy) — blocks push on failure
    Ci {
        /// Skip cargo test
        #[arg(long)]
        no_test: bool,
        /// Skip cargo clippy
        #[arg(long)]
        no_clippy: bool,
        /// Run all steps even if one fails
        #[arg(long)]
        no_fail_fast: bool,
    },

    // ─── Agent commands (merged from dxos) ───
    /// Interactive AI coding agent chat
    Chat {
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Run a single AI agent prompt
    Run {
        prompt: Vec<String>,
        #[arg(short, long)]
        model: Option<String>,
        #[arg(
            short,
            long,
            default_value = "workspace-write",
            value_parser = ["read-only", "workspace-write", "full-access"]
        )]
        permission: String,
        #[arg(long, default_value_t = 16, value_parser = parse_positive_usize)]
        max_turns: usize,
    },
    /// Find and fix issues automatically
    Fix,
    /// Review uncommitted changes
    Review,
    /// Explain the current codebase
    Explain,
    /// Run tests and fix failures
    Test,
    /// Generate commit message and commit
    Commit,
    /// Generate PR description and create PR
    Pr,
    /// Download and configure a local model
    Setup,
}

#[derive(Debug, Subcommand)]
enum SwarmCommands {
    /// Start a swarm for open GitHub issues in the current repository
    Start {
        #[arg(long)]
        repo: String,
        #[arg(long, default_value_t = 5, value_parser = parse_swarm_agent_count)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
enum ExternalCommands {
    /// List imported external tool servers
    List {
        #[arg(long)]
        running_only: bool,
    },
    /// Discover external servers by capability keyword
    Discover {
        capability: String,
        #[arg(long)]
        auto_start: bool,
    },
    /// Inspect the tools exposed by one external server
    #[command(alias = "tools")]
    Inspect { server: String },
    /// Run a tool on one external server
    #[command(alias = "call")]
    Run {
        server: String,
        tool: String,
        #[arg(long)]
        args: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ServiceCommands {
    /// List internal and external services in the CLI microservices architecture
    List {
        #[arg(long, default_value = "all", value_parser = ["all", "internal", "external"])]
        kind: String,
        #[arg(long)]
        running_only: bool,
    },
    /// Inspect one service and show its CLI contract
    Inspect { service: String },
    /// Print the static service topology for the CLI-first architecture
    Topology,
    /// Serve one internal service directly from the CLI
    Serve {
        service: String,
        /// Port for `web`
        #[arg(long)]
        port: Option<u16>,
        /// Companion dashboard port for MCP services
        #[arg(long)]
        web_port: Option<u16>,
        /// Disable the companion dashboard for MCP services
        #[arg(long)]
        no_web: bool,
    },
    /// Call a tool on one external microservice
    Call {
        service: String,
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
        Some(Commands::External { command }) | Some(Commands::Gateway { command }) => match command
        {
            ExternalCommands::List { .. } => format!("external-list-{:x}", cwd_hash),
            ExternalCommands::Discover { capability, .. } => {
                format!("external-discover-{}-{:x}", capability, cwd_hash)
            }
            ExternalCommands::Inspect { server } => {
                format!("external-inspect-{}-{:x}", server, cwd_hash)
            }
            ExternalCommands::Run { server, tool, .. } => {
                format!("external-run-{}-{}-{:x}", server, tool, cwd_hash)
            }
        },
        Some(Commands::Services { command }) => match command {
            ServiceCommands::List { kind, running_only } => {
                format!("services-list-{}-{}-{:x}", kind, running_only, cwd_hash)
            }
            ServiceCommands::Inspect { service } => {
                format!("services-inspect-{}-{:x}", service, cwd_hash)
            }
            ServiceCommands::Topology => format!("services-topology-{:x}", cwd_hash),
            ServiceCommands::Serve { service, .. } => {
                format!("services-serve-{}-{:x}", service, cwd_hash)
            }
            ServiceCommands::Call { service, tool, .. } => {
                format!("services-call-{}-{}-{:x}", service, tool, cwd_hash)
            }
        },
        Some(Commands::Ci { .. }) => format!("ci-{:x}", cwd_hash),
        Some(Commands::Chat { .. }) => format!("agent-chat-{:x}", cwd_hash),
        Some(Commands::Run { .. }) => format!("agent-run-{:x}", cwd_hash),
        Some(Commands::Fix) => format!("agent-fix-{:x}", cwd_hash),
        Some(Commands::Review) => format!("agent-review-{:x}", cwd_hash),
        Some(Commands::Explain) => format!("agent-explain-{:x}", cwd_hash),
        Some(Commands::Test) => format!("agent-test-{:x}", cwd_hash),
        Some(Commands::Commit) => format!("agent-commit-{:x}", cwd_hash),
        Some(Commands::Pr) => format!("agent-pr-{:x}", cwd_hash),
        Some(Commands::Setup) => format!("agent-setup-{:x}", cwd_hash),
        None => format!("default-{}-{:x}", default_web_port, cwd_hash),
    }
}

fn parse_positive_usize(value: &str) -> std::result::Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("'{value}' is not a valid positive integer"))?;
    if parsed == 0 {
        return Err("value must be at least 1".to_string());
    }
    Ok(parsed)
}

fn parse_swarm_agent_count(value: &str) -> std::result::Result<usize, String> {
    let parsed = parse_positive_usize(value)?;
    if parsed > 20 {
        return Err("swarm agent count must be between 1 and 20".to_string());
    }
    Ok(parsed)
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
            let _health_monitor = dx_terminal::health_monitor::start(Arc::clone(&application));

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
            let _health_monitor = dx_terminal::health_monitor::start(Arc::clone(&application));
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
            let _health_monitor =
                dx_terminal::health_monitor::start(Arc::clone(&application));
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
        Some(Commands::External { command }) | Some(Commands::Gateway { command }) => {
            run_external_cli(application, command).await?;
        }
        Some(Commands::Services { command }) => {
            run_services_cli(application, command, cfg.web_port).await?;
        }
        Some(Commands::Ci {
            no_test,
            no_clippy,
            no_fail_fast,
        }) => {
            let config = dx_terminal::ci::CiConfig {
                check: true,
                test: !no_test,
                clippy: !no_clippy,
                working_dir: None,
                fail_fast: !no_fail_fast,
            };
            let result = dx_terminal::ci::run(&config);
            print!("{result}");
            if !result.passed() {
                std::process::exit(1);
            }
        }

        // ─── Agent commands ───
        Some(Commands::Chat { model }) => {
            agent_repl::run_repl(model).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        Some(Commands::Run {
            prompt,
            model,
            permission,
            max_turns,
        }) => {
            run_agent(prompt.join(" "), model, permission, max_turns)?;
        }
        Some(Commands::Fix) => {
            run_agent("Find all bugs, issues, and code smells in this codebase. Fix them. Run tests to verify.".into(), None, "workspace-write".into(), 16)?;
        }
        Some(Commands::Review) => {
            run_agent("Run `git diff` to see uncommitted changes. Review each change for bugs, security issues, and code quality. Do not modify any files.".into(), None, "workspace-write".into(), 8)?;
        }
        Some(Commands::Explain) => {
            run_agent("Read the key files in this project. Give a concise explanation of what it does, its architecture, and key design decisions. Do not modify any files.".into(), None, "workspace-write".into(), 8)?;
        }
        Some(Commands::Test) => {
            run_agent("Run the test suite. If any tests fail, read the failing test and the code it tests, then fix the issue. Re-run tests to verify.".into(), None, "full-access".into(), 16)?;
        }
        Some(Commands::Commit) => {
            run_agent("Run `git diff --staged`. Generate a conventional commit message. Then run `git add -A && git commit -m \"<message>\"`.".into(), None, "full-access".into(), 4)?;
        }
        Some(Commands::Pr) => {
            run_agent("Run `git log --oneline main..HEAD`. Generate a PR title and description. Run `gh pr create`.".into(), None, "full-access".into(), 4)?;
        }
        Some(Commands::Setup) => {
            agent_setup::auto_setup().map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }

    Ok(())
}

fn run_agent(
    prompt: String,
    model: Option<String>,
    permission: String,
    max_turns: usize,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    let (client, model_name) = match dx_agent_api::ProviderClient::auto_detect(model.as_deref()) {
        Ok(r) => r,
        Err(_) => {
            let id = agent_setup::ensure_ready().map_err(|e| anyhow::anyhow!("{e}"))?;
            dx_agent_api::ProviderClient::auto_detect(Some(&id))
                .map_err(|e| anyhow::anyhow!("{e}"))?
        }
    };

    let mode = match permission.as_str() {
        "read-only" => dx_agent_harness::PermissionMode::ReadOnly,
        "full-access" => dx_agent_harness::PermissionMode::FullAccess,
        _ => dx_agent_harness::PermissionMode::WorkspaceWrite,
    };

    let policy = dx_agent_harness::PermissionPolicy::new(mode)
        .with_tool("read_file", dx_agent_harness::PermissionMode::ReadOnly)
        .with_tool("glob", dx_agent_harness::PermissionMode::ReadOnly)
        .with_tool("grep", dx_agent_harness::PermissionMode::ReadOnly)
        .with_tool("web_fetch", dx_agent_harness::PermissionMode::ReadOnly)
        .with_tool("repo_map", dx_agent_harness::PermissionMode::ReadOnly)
        .with_tool(
            "write_file",
            dx_agent_harness::PermissionMode::WorkspaceWrite,
        )
        .with_tool(
            "edit_file",
            dx_agent_harness::PermissionMode::WorkspaceWrite,
        )
        .with_tool("git", dx_agent_harness::PermissionMode::WorkspaceWrite)
        .with_tool("bash", dx_agent_harness::PermissionMode::FullAccess);

    let registry = dx_agent_tools::ToolRegistry::default_cli();
    let tools = registry.to_api_definitions();
    let system_prompt = agent_prompt::build_system_prompt(&cwd);

    let mut runtime =
        dx_agent_harness::ConversationRuntime::new(client, policy, system_prompt, tools, cwd)
            .with_max_iterations(max_turns);

    eprintln!(
        "\x1b[2mdx v{} | {model_name} | {permission}\x1b[0m\n",
        env!("CARGO_PKG_VERSION")
    );

    let start = std::time::Instant::now();
    let summary = runtime
        .run_turn(&prompt, None)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if !summary.was_streamed && !summary.text.is_empty() {
        println!("{}", summary.text);
    }

    agent_display::print_summary(
        summary.tool_calls,
        summary.iterations,
        summary.usage.total_tokens(),
        start.elapsed().as_secs_f64(),
    );
    Ok(())
}

fn rewrite_external_cli_terms(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(server) = map.remove("mcp") {
                map.insert("server".to_string(), server);
            }
            if let Some(servers) = map.remove("mcps") {
                map.insert("servers".to_string(), servers);
            }
            for child in map.values_mut() {
                rewrite_external_cli_terms(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_external_cli_terms(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

async fn run_external_cli(app: Arc<app::App>, command: ExternalCommands) -> anyhow::Result<()> {
    let output = match command {
        ExternalCommands::List { running_only } => {
            mcp::tools::gateway_tools::gateway_list(
                &app,
                mcp::types::GatewayListRequest {
                    running_only: Some(running_only),
                },
            )
            .await
        }
        ExternalCommands::Discover {
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
        ExternalCommands::Inspect { server } => {
            mcp::tools::gateway_tools::gateway_tools(
                &app,
                mcp::types::GatewayToolsRequest {
                    mcp: server,
                    auto_start: Some(true),
                },
            )
            .await
        }
        ExternalCommands::Run { server, tool, args } => {
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
                    mcp: server,
                    tool,
                    arguments: parsed_args,
                },
            )
            .await
        }
    };

    let mut value =
        serde_json::from_str::<Value>(&output).unwrap_or_else(|_| json!({ "raw": output }));
    rewrite_external_cli_terms(&mut value);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

async fn run_services_cli(
    app: Arc<app::App>,
    command: ServiceCommands,
    default_web_port: u16,
) -> anyhow::Result<()> {
    match command {
        ServiceCommands::List { kind, running_only } => {
            let value = services::list_services(app.as_ref(), &kind, running_only).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
        ServiceCommands::Inspect { service } => {
            let value = services::inspect_service(app.as_ref(), &service).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
        ServiceCommands::Topology => {
            println!("{}", serde_json::to_string_pretty(&services::topology())?);
            Ok(())
        }
        ServiceCommands::Call {
            service,
            tool,
            args,
        } => {
            let value = services::call_service(app.as_ref(), &service, &tool, args).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
        ServiceCommands::Serve {
            service,
            port,
            web_port,
            no_web,
        } => {
            let normalized = services::normalize_internal_service_name(&service).to_string();
            match normalized.as_str() {
                "web" => {
                    if web_port.is_some() {
                        anyhow::bail!("use --port with `dx services serve web`");
                    }
                    web::run_web_server(app, port.unwrap_or(default_web_port)).await
                }
                "mcp" => {
                    if port.is_some() {
                        anyhow::bail!("use --web-port with MCP services; --port is only for `web`");
                    }
                    run_mcp_mode(app, web_port.unwrap_or(default_web_port), no_web, None).await
                }
                "core" | "queue" | "tracker" | "coord" | "intel" => {
                    if port.is_some() {
                        anyhow::bail!("use --web-port with MCP services; --port is only for `web`");
                    }
                    run_mcp_mode(
                        app,
                        web_port.unwrap_or(default_web_port),
                        no_web,
                        Some(normalized),
                    )
                    .await
                }
                "gateway" => anyhow::bail!(
                    "`gateway` is an embedded service; use `dx services list --kind external` and `dx services inspect <service>`"
                ),
                _ => anyhow::bail!("unknown internal service '{}'", service),
            }
        }
    }
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
        SwarmCommands::Status => match swarm::status(app.as_ref()).await {
            Ok(report) => serde_json::to_value(report)?,
            Err(error) if is_missing_swarm_state(&error) => swarm_idle_cli_value(
                "No saved or active swarm state found. Nothing is currently running.",
            ),
            Err(error) => return Err(error),
        },
        SwarmCommands::Stop => match swarm::stop(app.as_ref()).await {
            Ok(report) => serde_json::to_value(report)?,
            Err(error) if is_missing_swarm_state(&error) => {
                swarm_idle_cli_value("No saved or active swarm state found. Nothing to stop.")
            }
            Err(error) => return Err(error),
        },
    };

    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn is_missing_swarm_state(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("no swarm state found"))
}

fn swarm_idle_cli_value(message: &str) -> Value {
    json!({
        "status": "idle",
        "message": message,
        "active": false,
        "repo": Value::Null,
        "repo_path": Value::Null,
        "provider": Value::Null,
        "max_agents": 0,
        "started_at": Value::Null,
        "labels": [],
        "results": [],
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_external_command_surface() {
        let cli = Cli::try_parse_from(["dx", "external", "list"]).expect("parse");
        match cli.command {
            Some(Commands::External {
                command: ExternalCommands::List { running_only },
            }) => assert!(!running_only),
            _ => panic!("expected external list"),
        }
    }

    #[test]
    fn parses_tools_alias() {
        let cli = Cli::try_parse_from(["dx", "tools", "discover", "browser"]).expect("parse");
        match cli.command {
            Some(Commands::External {
                command:
                    ExternalCommands::Discover {
                        capability,
                        auto_start,
                    },
            }) => {
                assert_eq!(capability, "browser");
                assert!(!auto_start);
            }
            _ => panic!("expected tools discover"),
        }
    }

    #[test]
    fn parses_services_alias() {
        let cli = Cli::try_parse_from(["dx", "svc", "list", "--kind", "internal"]).expect("parse");
        match cli.command {
            Some(Commands::Services {
                command: ServiceCommands::List { kind, running_only },
            }) => {
                assert_eq!(kind, "internal");
                assert!(!running_only);
            }
            _ => panic!("expected services list alias"),
        }
    }

    #[test]
    fn parses_legacy_gateway_alias() {
        let cli = Cli::try_parse_from(["dx", "gateway", "tools", "playwright"]).expect("parse");
        match cli.command {
            Some(Commands::Gateway {
                command: ExternalCommands::Inspect { server },
            }) => assert_eq!(server, "playwright"),
            _ => panic!("expected legacy gateway tools alias"),
        }
    }

    #[test]
    fn rewrites_mcp_terms_for_cli_output() {
        let mut value = json!({
            "mcp": "playwright",
            "mcps": [
                { "mcp": "filesystem" }
            ]
        });
        rewrite_external_cli_terms(&mut value);
        assert_eq!(value["server"], "playwright");
        assert_eq!(value["servers"][0]["server"], "filesystem");
        assert!(value.get("mcp").is_none());
        assert!(value.get("mcps").is_none());
    }

    #[test]
    fn rejects_invalid_agent_permission_mode() {
        let err = Cli::try_parse_from(["dx", "run", "ship", "it", "--permission", "danger-zone"])
            .expect_err("invalid permission should fail");
        assert!(err.to_string().contains("possible values"));
    }

    #[test]
    fn rejects_zero_agent_turn_budget() {
        let err = Cli::try_parse_from(["dx", "run", "ship", "it", "--max-turns", "0"])
            .expect_err("zero max-turns should fail");
        assert!(err.to_string().contains("at least 1"));
    }

    #[test]
    fn swarm_idle_cli_payload_is_stable() {
        let value = swarm_idle_cli_value("Nothing is currently running.");
        assert_eq!(value["status"], "idle");
        assert_eq!(value["active"], false);
        assert_eq!(value["results"], json!([]));
        assert!(value["repo"].is_null());
    }

    #[test]
    fn rejects_out_of_range_swarm_agent_count() {
        let err = Cli::try_parse_from([
            "dx",
            "swarm",
            "start",
            "--repo",
            "pdaxt/dx-terminal",
            "--max-agents",
            "21",
        ])
        .expect_err("out-of-range swarm agent count should fail");
        assert!(err.to_string().contains("between 1 and 20"));
    }
}
