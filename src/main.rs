use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dx_terminal::app::Config;
use dx_terminal::ui::run_app;

/// Install a panic hook that restores the terminal before printing the panic.
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restore
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        original_hook(panic_info);
    }));
}

#[derive(Parser)]
#[command(name = "dx")]
#[command(author, version, about, long_about = None)]
#[command(about = "DX Terminal — AI-native terminal multiplexer")]
struct Cli {
    /// Polling interval (milliseconds)
    #[arg(short, long, default_value = "500", value_name = "MS")]
    poll_interval: u64,

    /// Lines to capture from each pane
    #[arg(short, long, default_value = "100", value_name = "LINES")]
    capture_lines: u32,

    /// Config file path
    #[arg(short = 'f', long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Hub API URL (overrides config file)
    #[arg(long, value_name = "URL")]
    api_url: Option<String>,

    /// Legacy tmux mode — monitor existing tmux panes instead of native PTY
    #[arg(long)]
    tmux: bool,

    /// Write debug logs to dx-terminal.log
    #[arg(short, long)]
    debug: bool,

    /// Show config file path
    #[arg(long)]
    show_config_path: bool,

    /// Generate default config file
    #[arg(long)]
    init_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_panic_hook();
    let cli = Cli::parse();

    // Show config path and exit
    if cli.show_config_path {
        if let Some(path) = Config::default_path() {
            println!("{}", path.display());
        } else {
            println!("Config directory not found");
        }
        return Ok(());
    }

    // Initialize config file and exit
    if cli.init_config {
        let config = Config::default();
        if let Err(e) = config.save() {
            eprintln!("Failed to create config: {}", e);
            std::process::exit(1);
        }
        if let Some(path) = Config::default_path() {
            println!("Config created: {}", path.display());
        }
        return Ok(());
    }

    // Setup logging
    if cli.debug {
        let log_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dx-terminal");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_file = std::fs::File::create(log_dir.join("debug.log"))?;
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(log_file)
            .with_ansi(false);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(tracing_subscriber::filter::LevelFilter::DEBUG)
            .init();
    }

    // Load config (from file or CLI args)
    let mut config = if let Some(config_path) = &cli.config {
        Config::load_from(config_path).unwrap_or_else(|e| {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        })
    } else {
        Config::load()
    };

    // CLI args override config file
    config.poll_interval_ms = cli.poll_interval;
    config.capture_lines = cli.capture_lines;
    config.native_mode = !cli.tmux;
    if let Some(url) = cli.api_url {
        config.api_url = Some(url);
    }
    // Default to localhost if no URL in config or CLI
    if config.api_url.is_none() {
        config.api_url = Some("http://localhost:3100".to_string());
    }

    // Run the application
    run_app(config).await
}
