//! Username OSINT — check username across 479 social networks.
//! Data from sherlock-project/sherlock (75K stars).

use clap::Subcommand;
use colored::Colorize;
use serde_json::Value;
use std::collections::HashMap;

use crate::display;

// Embed sherlock's site database at compile time
const SHERLOCK_DATA: &str = include_str!("../data/sherlock_sites.json");

#[derive(Subcommand)]
pub enum UsernameAction {
    /// Check if a username exists across 479 social networks
    #[command(alias = "c")]
    Check {
        /// Username to search for
        username: String,
        /// Only show found results (hide errors/not-found)
        #[arg(short, long)]
        found_only: bool,
        /// Max concurrent checks (default: 20)
        #[arg(short = 'j', long, default_value = "20")]
        jobs: usize,
    },
}

pub async fn run(action: UsernameAction) -> Result<(), String> {
    match action {
        UsernameAction::Check { username, found_only, jobs } => check(&username, found_only, jobs).await,
    }
}

async fn check(username: &str, found_only: bool, max_concurrent: usize) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username required".into());
    }

    let sites: HashMap<String, Value> = serde_json::from_str(SHERLOCK_DATA)
        .map_err(|e| format!("Failed to load site database: {}", e))?;

    println!(
        "  {} \"{}\" across {} sites ({})",
        "Hunting:".dimmed(),
        username.bold(),
        sites.len().to_string().cyan(),
        format!("{} concurrent", max_concurrent).dimmed()
    );
    println!();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut handles = Vec::new();

    for (site_name, site_data) in &sites {
        let url_template = match site_data.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => continue,
        };

        let error_type = site_data
            .get("errorType")
            .and_then(|v| v.as_str())
            .unwrap_or("status_code")
            .to_string();

        let error_msg = site_data
            .get("errorMsg")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let url = url_template.replace("{}", username);
        let name = site_name.clone();
        let client = client.clone();
        let sem = semaphore.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let result = check_site(&client, &name, &url, &error_type, error_msg.as_deref()).await;
            (name, url, result)
        }));
    }

    let mut found = Vec::new();
    let mut not_found = Vec::new();
    let mut errors = Vec::new();

    for handle in handles {
        match handle.await {
            Ok((name, url, SiteResult::Found)) => found.push((name, url)),
            Ok((name, url, SiteResult::NotFound)) => not_found.push((name, url)),
            Ok((name, url, SiteResult::Error(e))) => errors.push((name, url, e)),
            Err(_) => {}
        }
    }

    // Sort results
    found.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Display found
    if !found.is_empty() {
        println!("  {}", format!("Found on {} sites:", found.len()).green().bold());
        println!();
        for (name, url) in &found {
            println!("    {} {:<25} {}", "+".green().bold(), name, url.dimmed());
        }
    }

    if !found_only {
        if !errors.is_empty() {
            println!();
            println!("  {} {} sites had errors", "!".yellow(), errors.len());
        }
        println!();
        display::kv("Found", &found.len().to_string().green().to_string());
        display::kv("Not found", &not_found.len().to_string());
        display::kv("Errors", &errors.len().to_string());
        display::kv("Total checked", &(found.len() + not_found.len() + errors.len()).to_string());
    }

    println!();
    Ok(())
}

enum SiteResult {
    Found,
    NotFound,
    Error(String),
}

async fn check_site(
    client: &reqwest::Client,
    _name: &str,
    url: &str,
    error_type: &str,
    error_msg: Option<&str>,
) -> SiteResult {
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => return SiteResult::Error(e.to_string()),
    };

    let status = resp.status();

    match error_type {
        "status_code" => {
            if status.is_success() {
                SiteResult::Found
            } else {
                SiteResult::NotFound
            }
        }
        "message" => {
            let body = resp.text().await.unwrap_or_default();
            if let Some(msg) = error_msg {
                if body.contains(msg) {
                    SiteResult::NotFound
                } else if status.is_success() {
                    SiteResult::Found
                } else {
                    SiteResult::NotFound
                }
            } else if status.is_success() {
                SiteResult::Found
            } else {
                SiteResult::NotFound
            }
        }
        "response_url" => {
            // If we got redirected away from the profile URL, user doesn't exist
            let final_url = resp.url().to_string();
            if final_url.contains("error") || final_url.contains("notfound") || final_url.contains("404") {
                SiteResult::NotFound
            } else if status.is_success() {
                SiteResult::Found
            } else {
                SiteResult::NotFound
            }
        }
        _ => {
            if status.is_success() {
                SiteResult::Found
            } else {
                SiteResult::NotFound
            }
        }
    }
}
