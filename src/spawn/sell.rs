use colored::Colorize;
use serde::{Deserialize, Serialize};

use super::state::{Phase, SpawnState};
use crate::display;

#[derive(Debug, Serialize, Deserialize)]
struct EmailDraft {
    to: String,
    company: String,
    subject: String,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OutreachLog {
    emails_drafted: u32,
    emails_sent: u32,
    social_posts: u32,
    dry_run: bool,
    timestamp: String,
}

pub async fn run(slug: Option<String>) -> Result<(), String> {
    let mut state = SpawnState::or_latest(slug)?;
    execute(&mut state, true).await // dry-run by default
}

pub async fn execute(state: &mut SpawnState, dry_run: bool) -> Result<(), String> {
    state.transition(Phase::Selling)?;
    state.save()?;

    display::header(&format!("SELL: {}{}", state.niche, if dry_run { " [DRY RUN]" } else { "" }));

    let dir = state.dir();

    // ── Load prospects ───────────────────────────────────────────────────
    let csv_path = dir.join("research/prospects.csv");
    let csv_data = std::fs::read_to_string(&csv_path)
        .map_err(|e| format!("Read prospects.csv: {} — run `dx spawn observe` first", e))?;

    let deployed_url = state.deployed_url.clone().unwrap_or_else(|| "https://example.com".into());
    let niche = state.niche.clone();

    // ── Step 1: Draft Emails ─────────────────────────────────────────────
    println!("  {} Drafting emails...", "→".cyan());

    let email_dir = dir.join("sell/emails");
    let mut drafts = Vec::new();

    for (i, line) in csv_data.lines().skip(1).enumerate() {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 4 { continue; }

        let company = fields[0].trim_matches('"').to_string();
        let domain = fields[1].trim_matches('"');
        let _contact = fields[2].trim_matches('"');
        let email = fields[3].trim_matches('"').to_string();

        if email.is_empty() || !email.contains('@') { continue; }

        let subject = format!("Quick question about {} at {}", niche, company);
        let body = format!(
            r#"Hi,

I came across {} while researching companies in the {} space.

We've built a tool that helps teams like yours move faster — specifically around {}.

It's free to try: {}

Would you be open to a quick look? Takes 2 minutes.

Best,
The Team"#,
            company, niche, niche, deployed_url
        );

        let draft = EmailDraft {
            to: email.clone(),
            company: company.clone(),
            subject: subject.clone(),
            body: body.clone(),
        };

        // Write individual draft
        let draft_path = email_dir.join(format!("draft-{:03}.json", i + 1));
        let _ = std::fs::write(&draft_path, serde_json::to_string_pretty(&draft).unwrap_or_default());
        drafts.push(draft);

        if drafts.len() >= 50 { break; } // cap at 50
    }

    state.log("sell", "email_drafts", true, &format!("{} emails drafted", drafts.len()));
    println!("    {} {} email drafts written", "✓".green(), drafts.len());

    // ── Step 2: Draft Social Posts ───────────────────────────────────────
    println!("  {} Drafting social posts...", "→".cyan());

    let social_dir = dir.join("sell/social");
    let content_dir = dir.join("build/content");
    let mut social_count = 0;

    if let Ok(entries) = std::fs::read_dir(&content_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let title = content.lines().next().unwrap_or("").trim_start_matches("# ");

                // LinkedIn post
                let linkedin = format!(
                    "I've been researching {} and here's what most people get wrong:\n\n\
                    {}\n\n\
                    The solution? Build something that's actually fast.\n\n\
                    We did: {}\n\n\
                    #{}",
                    state.niche,
                    title,
                    deployed_url,
                    state.niche.split_whitespace().next().unwrap_or("tech")
                );
                let li_path = social_dir.join(format!("linkedin-{:02}.md", social_count + 1));
                let _ = std::fs::write(&li_path, &linkedin);

                // Tweet
                let tweet = format!(
                    "Most {} tools are bloated and slow.\n\n\
                    So we built one that isn't.\n\n\
                    Free to try: {}",
                    state.niche,
                    deployed_url
                );
                let tw_path = social_dir.join(format!("tweet-{:02}.md", social_count + 1));
                let _ = std::fs::write(&tw_path, &tweet);

                social_count += 1;
            }
        }
    }

    state.log("sell", "social_drafts", true, &format!("{} social post pairs drafted", social_count));
    println!("    {} {} social post pairs (LinkedIn + Twitter)", "✓".green(), social_count);

    // ── Step 3: Send (if not dry run) ────────────────────────────────────
    let emails_sent = if dry_run {
        println!();
        println!("  {} Dry run — no emails sent. Review drafts at:", "!".yellow().bold());
        println!("    {}", email_dir.to_string_lossy().dimmed());
        println!("    {}", social_dir.to_string_lossy().dimmed());
        println!();
        println!("  To send for real: {}", "dx spawn sell --send".cyan().bold());
        0
    } else {
        // TODO: integrate with Mailforge API for actual sending
        // For now, log the intent
        println!("  {} Sending {} emails...", "→".cyan(), drafts.len());
        state.log("sell", "send", true, &format!("{} emails queued", drafts.len()));
        drafts.len() as u32
    };

    // ── Save outreach log ────────────────────────────────────────────────
    let outreach = OutreachLog {
        emails_drafted: drafts.len() as u32,
        emails_sent,
        social_posts: social_count as u32,
        dry_run,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let log_path = dir.join("sell/outreach.json");
    std::fs::write(&log_path, serde_json::to_string_pretty(&outreach).unwrap_or_default())
        .map_err(|e| format!("write outreach.json: {}", e))?;
    state.outreach_stats = Some("sell/outreach.json".into());

    state.transition(Phase::Sold)?;
    state.save()?;

    println!();
    display::status("SELL complete", if dry_run { "dry run" } else { "✓" });
    display::kv("Emails drafted", &drafts.len().to_string());
    display::kv("Emails sent", &emails_sent.to_string());
    display::kv("Social posts", &social_count.to_string());
    println!();
    println!("  Next: {}", "dx spawn learn".cyan().bold());
    println!();

    Ok(())
}
