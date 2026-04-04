//! OSINT data sources — free, no API keys needed.
//! Ported from theHarvester + subfinder patterns.

use serde_json::{json, Value};

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Certificate Transparency search via crt.sh — finds subdomains + emails from SSL certs.
pub async fn crtsh(domain: &str) -> Vec<String> {
    let url = format!("https://crt.sh/?q=%25.{}&output=json", domain);
    let resp = match client().get(&url).send().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let items: Vec<Value> = resp.json().await.unwrap_or_default();
    let mut names: Vec<String> = items
        .iter()
        .filter_map(|item| item.get("name_value").and_then(|v| v.as_str()))
        .flat_map(|name| name.split('\n'))
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && s.contains('.'))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// RapidDNS — fast subdomain enumeration.
pub async fn rapiddns(domain: &str) -> Vec<String> {
    let url = format!("https://rapiddns.io/subdomain/{}?full=1", domain);
    let html = match client().get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    // Parse table rows for subdomains
    let re = regex::Regex::new(&format!(r"([a-zA-Z0-9._-]+\.{})", regex::escape(domain))).unwrap();
    let mut subs: Vec<String> = re
        .find_iter(&html)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    subs.sort();
    subs.dedup();
    subs
}

/// HackerTarget — free subdomain finder (no API key for basic).
pub async fn hackertarget(domain: &str) -> Vec<String> {
    let url = format!("https://api.hackertarget.com/hostsearch/?q={}", domain);
    let text = match client().get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    if text.contains("error") || text.contains("API count") {
        return vec![];
    }
    let mut subs: Vec<String> = text
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.contains('.'))
        .collect();
    subs.sort();
    subs.dedup();
    subs
}

/// Wayback Machine — find historical URLs for a domain.
pub async fn wayback_urls(domain: &str, limit: usize) -> Vec<String> {
    let url = format!(
        "https://web.archive.org/cdx/search/cdx?url=*.{}/*&output=json&fl=original&collapse=urlkey&limit={}",
        domain, limit
    );
    let resp = match client().get(&url).send().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let items: Vec<Vec<String>> = resp.json().await.unwrap_or_default();
    items
        .into_iter()
        .skip(1) // skip header row
        .filter_map(|row| row.into_iter().next())
        .collect()
}

/// Aggregate subdomains from all free sources.
pub async fn find_subdomains(domain: &str) -> Vec<(String, String)> {
    let (crt, rapid, hacker) = tokio::join!(
        crtsh(domain),
        rapiddns(domain),
        hackertarget(domain),
    );

    let mut results: Vec<(String, String)> = Vec::new();
    for sub in crt {
        results.push((sub, "crt.sh".to_string()));
    }
    for sub in rapid {
        if !results.iter().any(|(s, _)| s == &sub) {
            results.push((sub, "rapiddns".to_string()));
        }
    }
    for sub in hacker {
        if !results.iter().any(|(s, _)| s == &sub) {
            results.push((sub, "hackertarget".to_string()));
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}
