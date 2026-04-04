use colored::Colorize;
use serde::{Deserialize, Serialize};

use super::state::{Phase, SpawnState};
use crate::display;

#[derive(Debug, Serialize, Deserialize)]
struct Learnings {
    cycle: u32,
    emails_drafted: u32,
    emails_sent: u32,
    social_posts: u32,
    keywords_found: u32,
    competitors_found: u32,
    prospects_found: u32,
    recommendations: Vec<String>,
    next_actions: Vec<String>,
    timestamp: String,
}

pub async fn run(slug: Option<String>) -> Result<(), String> {
    let mut state = SpawnState::or_latest(slug)?;
    execute(&mut state).await
}

pub async fn execute(state: &mut SpawnState) -> Result<(), String> {
    state.transition(Phase::Learning)?;
    state.save()?;

    display::header(&format!("LEARN: {} (cycle {})", state.niche, state.cycle));

    let dir = state.dir();

    // ── Gather metrics from all phases ───────────────────────────────────
    let market: serde_json::Value = std::fs::read_to_string(dir.join("research/market.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let competitors: Vec<serde_json::Value> = std::fs::read_to_string(dir.join("research/competitors.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let prospects_count = std::fs::read_to_string(dir.join("research/prospects.csv"))
        .map(|s| s.lines().count().saturating_sub(1))
        .unwrap_or(0);

    let outreach: serde_json::Value = std::fs::read_to_string(dir.join("sell/outreach.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let keywords_count = market.get("keywords").and_then(|k| k.as_array()).map(|a| a.len()).unwrap_or(0);
    let questions_count = market.get("questions").and_then(|q| q.as_array()).map(|a| a.len()).unwrap_or(0);
    let emails_drafted = outreach.get("emails_drafted").and_then(|e| e.as_u64()).unwrap_or(0);
    let emails_sent = outreach.get("emails_sent").and_then(|e| e.as_u64()).unwrap_or(0);
    let social_posts = outreach.get("social_posts").and_then(|s| s.as_u64()).unwrap_or(0);

    // ── Generate recommendations ─────────────────────────────────────────
    let mut recommendations = Vec::new();
    let mut next_actions = Vec::new();

    if keywords_count < 20 {
        recommendations.push("Low keyword coverage — try broader seed terms next cycle".into());
        next_actions.push("Expand niche description for observe phase".into());
    } else {
        recommendations.push(format!("Good keyword coverage ({} found) — focus on long-tail", keywords_count));
    }

    if competitors.len() < 3 {
        recommendations.push("Few competitors found — niche may be too narrow or poorly defined".into());
        next_actions.push("Try related niches in next observe cycle".into());
    } else {
        recommendations.push(format!("{} competitors mapped — analyze their weaknesses", competitors.len()));
        next_actions.push("Enrich competitor tech stacks (dx recon tech <domain>)".into());
    }

    if prospects_count < 10 {
        recommendations.push("Low prospect count — need more lead sources".into());
        next_actions.push("Add LinkedIn scraping or industry directory sources".into());
    } else {
        recommendations.push(format!("{} prospects in pipeline", prospects_count));
    }

    if emails_sent == 0 {
        recommendations.push("No emails sent yet — outreach was dry-run".into());
        next_actions.push("Review drafts, then run: dx spawn sell --send".into());
    }

    if social_posts == 0 {
        next_actions.push("Post social content manually or via dx engage".into());
    }

    // Always suggest
    next_actions.push("Run next cycle: dx spawn observe --resume <slug>".into());
    next_actions.push("Monitor: set up dx monitor keyword for brand mentions".into());

    // ── Save learnings ───────────────────────────────────────────────────
    let learnings = Learnings {
        cycle: state.cycle,
        emails_drafted: emails_drafted as u32,
        emails_sent: emails_sent as u32,
        social_posts: social_posts as u32,
        keywords_found: keywords_count as u32,
        competitors_found: competitors.len() as u32,
        prospects_found: prospects_count as u32,
        recommendations: recommendations.clone(),
        next_actions: next_actions.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // Save as current + archive
    let learn_path = dir.join("learn/learnings.json");
    std::fs::write(&learn_path, serde_json::to_string_pretty(&learnings).unwrap_or_default())
        .map_err(|e| format!("write learnings: {}", e))?;

    let archive_path = dir.join(format!("learn/cycle-{}.json", state.cycle));
    let _ = std::fs::copy(&learn_path, &archive_path);

    state.learnings = Some("learn/learnings.json".into());

    // ── Display ──────────────────────────────────────────────────────────
    println!("  {}", "Metrics".bold());
    display::kv("Keywords", &keywords_count.to_string());
    display::kv("Questions", &questions_count.to_string());
    display::kv("Competitors", &competitors.len().to_string());
    display::kv("Prospects", &prospects_count.to_string());
    display::kv("Emails drafted", &emails_drafted.to_string());
    display::kv("Emails sent", &emails_sent.to_string());
    display::kv("Social posts", &social_posts.to_string());

    println!();
    println!("  {}", "Recommendations".bold());
    for (i, rec) in recommendations.iter().enumerate() {
        println!("    {}. {}", i + 1, rec);
    }

    println!();
    println!("  {}", "Next Actions".bold());
    for (i, action) in next_actions.iter().enumerate() {
        println!("    {} {}", format!("{}.", i + 1).dimmed(), action.cyan());
    }

    // ── Transition ───────────────────────────────────────────────────────
    state.cycle += 1;
    state.transition(Phase::Iterating)?;
    state.save()?;

    println!();
    display::status(&format!("LEARN complete — cycle {} done", state.cycle - 1), "✓");
    println!();

    Ok(())
}
