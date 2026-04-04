use clap::Subcommand;
use colored::Colorize;

use crate::display;

pub mod state;
pub mod scrape;
pub mod observe;
pub mod build;
pub mod sell;
pub mod learn;

use state::{SpawnState, Phase};

#[derive(Subcommand)]
pub enum SpawnAction {
    /// Run the full OBSERVE → BUILD → SELL → LEARN loop
    #[command(alias = "go")]
    Run {
        /// Niche description (e.g. "AI contract review for Australian law firms")
        niche: Vec<String>,
    },
    /// Phase 1: Research market, find prospects, analyze competitors
    Observe {
        /// Niche to research
        niche: Vec<String>,
        /// Resume existing spawn by slug
        #[arg(short, long)]
        resume: Option<String>,
    },
    /// Phase 2: Generate spec, build landing page, create content
    Build {
        /// Spawn slug (default: most recent)
        #[arg(short, long)]
        spawn: Option<String>,
    },
    /// Phase 3: Draft cold emails and social posts (dry-run by default)
    Sell {
        /// Spawn slug (default: most recent)
        #[arg(short, long)]
        spawn: Option<String>,
        /// Actually send emails (default: dry-run)
        #[arg(long)]
        send: bool,
    },
    /// Phase 4: Collect metrics, score effectiveness, recommend next actions
    Learn {
        /// Spawn slug (default: most recent)
        #[arg(short, long)]
        spawn: Option<String>,
    },
    /// List all spawns
    #[command(alias = "ls")]
    List,
    /// Show detailed status of a spawn
    Status {
        /// Spawn slug (default: most recent)
        slug: Option<String>,
    },
    /// Kill/archive a spawn
    Kill {
        /// Spawn slug
        slug: String,
    },
}

pub async fn run(action: SpawnAction) -> Result<(), String> {
    match action {
        SpawnAction::Run { niche } => run_full(&niche.join(" ")).await,
        SpawnAction::Observe { niche, resume } => {
            let niche_opt = if niche.is_empty() { None } else { Some(niche) };
            observe::run(niche_opt, resume).await
        }
        SpawnAction::Build { spawn } => build::run(spawn).await,
        SpawnAction::Sell { spawn, send } => {
            let mut state = SpawnState::or_latest(spawn)?;
            sell::execute(&mut state, !send).await
        }
        SpawnAction::Learn { spawn } => learn::run(spawn).await,
        SpawnAction::List => list_spawns(),
        SpawnAction::Status { slug } => show_status(slug),
        SpawnAction::Kill { slug } => kill_spawn(&slug),
    }
}

async fn run_full(niche: &str) -> Result<(), String> {
    if niche.is_empty() {
        return Err("Niche required. Example: dx spawn run \"AI contract review for law firms\"".into());
    }

    println!();
    println!("  {}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!("  {}  {}", "SPAWN".bold().cyan(), niche.bold());
    println!("  {}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!("  {} OBSERVE → BUILD → SELL → LEARN", "Pipeline:".dimmed());
    println!();

    // Create state
    let mut state = SpawnState::new(niche);
    state.save()?;

    // Phase 1: OBSERVE
    observe::execute(&mut state).await?;

    // Phase 2: BUILD
    build::execute(&mut state).await?;

    // Phase 3: SELL (dry-run)
    sell::execute(&mut state, true).await?;

    // Phase 4: LEARN
    learn::execute(&mut state).await?;

    // Summary
    println!("  {}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!("  {} Spawn complete: {}", "✓".green().bold(), state.slug.cyan());
    println!("  {} {}", "Dir:".dimmed(), state.dir().to_string_lossy());
    println!("  {} dx spawn status {}", "Check:".dimmed(), state.slug);
    println!("  {} dx spawn sell --send -s {}", "Send:".dimmed(), state.slug);
    println!("  {}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();

    Ok(())
}

fn list_spawns() -> Result<(), String> {
    let spawns = SpawnState::list_all();

    display::header("Spawns");

    if spawns.is_empty() {
        println!("  {}", "No spawns yet. Run: dx spawn run \"your niche\"".dimmed());
        println!();
        return Ok(());
    }

    let rows: Vec<Vec<String>> = spawns
        .iter()
        .map(|s| {
            let phase_str = format!("{}", s.phase);
            let phase_colored = match &s.phase {
                Phase::Idle => phase_str.dimmed().to_string(),
                Phase::Observing | Phase::Building | Phase::Selling | Phase::Learning => {
                    phase_str.yellow().to_string()
                }
                Phase::Observed | Phase::Built | Phase::Sold | Phase::Iterating => {
                    phase_str.green().to_string()
                }
                Phase::Failed { .. } => phase_str.red().to_string(),
            };
            vec![
                s.slug.clone(),
                phase_colored,
                format!("#{}", s.cycle),
                display::truncate(&s.niche, 40),
                s.updated_at.chars().take(10).collect(),
            ]
        })
        .collect();

    display::table(&["Slug", "Phase", "Cycle", "Niche", "Updated"], &rows);
    println!();
    Ok(())
}

fn show_status(slug: Option<String>) -> Result<(), String> {
    let state = SpawnState::or_latest(slug)?;

    display::header(&format!("Spawn: {}", state.slug));

    display::kv("Niche", &state.niche);
    display::kv("Phase", &format!("{}", state.phase));
    display::kv("Cycle", &state.cycle.to_string());
    display::kv("Created", &state.created_at);
    display::kv("Updated", &state.updated_at);

    println!();
    println!("  {}", "Outputs".bold());
    let outputs = [
        ("Market research", &state.market_research),
        ("Competitors", &state.competitors),
        ("Prospects", &state.prospects),
        ("Product spec", &state.product_spec),
        ("Deployed URL", &state.deployed_url),
        ("Outreach stats", &state.outreach_stats),
        ("Learnings", &state.learnings),
    ];
    for (label, value) in &outputs {
        let v = match value {
            Some(p) => p.as_str().green().to_string(),
            None => "−".dimmed().to_string(),
        };
        display::kv(label, &v);
    }

    if !state.log.is_empty() {
        println!();
        println!("  {} (last 10)", "Log".bold());
        for entry in state.log.iter().rev().take(10) {
            let icon = if entry.ok { "✓".green() } else { "✗".red() };
            println!(
                "    {} {} {} {}",
                entry.ts.chars().take(19).collect::<String>().dimmed(),
                icon,
                format!("[{}]", entry.phase).cyan(),
                entry.action
            );
        }
    }

    println!();
    println!("  {} {}", "Dir:".dimmed(), state.dir().to_string_lossy());
    println!();
    Ok(())
}

fn kill_spawn(slug: &str) -> Result<(), String> {
    let state = SpawnState::load(slug)?;
    let dir = state.dir();

    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("remove: {}", e))?;
        println!("  {} Spawn '{}' killed", "✓".green(), slug);
    } else {
        println!("  {} Spawn '{}' not found", "✗".red(), slug);
    }
    println!();
    Ok(())
}
