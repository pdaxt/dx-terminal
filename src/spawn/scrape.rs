//! Real scraping engines — Reddit, HN, Google, competitor sites.
//! No API keys. No templates. Real data.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// ── Reddit ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedditPost {
    pub title: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    pub url: String,
    pub selftext: String,
}

/// Search Reddit for real discussions about a topic. No auth needed.
pub async fn reddit_search(query: &str, limit: usize) -> Vec<RedditPost> {
    let http = client();
    let url = format!(
        "https://old.reddit.com/search.json?q={}&sort=relevance&limit={}&t=year&type=link",
        urlenc(query),
        limit.min(100)
    );

    let resp = match http
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return vec![],
    };

    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let children = match data
        .get("data")
        .and_then(|d| d.get("children"))
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => return vec![],
    };

    children
        .iter()
        .filter_map(|child| {
            let d = child.get("data")?;
            Some(RedditPost {
                title: d.get("title")?.as_str()?.to_string(),
                subreddit: d.get("subreddit")?.as_str()?.to_string(),
                score: d.get("score")?.as_i64()?,
                num_comments: d.get("num_comments")?.as_i64().unwrap_or(0),
                url: format!(
                    "https://reddit.com{}",
                    d.get("permalink")?.as_str()?
                ),
                selftext: d
                    .get("selftext")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(500)
                    .collect(),
            })
        })
        .collect()
}

/// Extract pain points from Reddit posts — real complaints and frustrations.
pub fn extract_pain_points(posts: &[RedditPost]) -> Vec<String> {
    let pain_words = [
        "hate", "frustrat", "annoying", "broken", "terrible", "worst",
        "expensive", "slow", "buggy", "useless", "waste", "awful",
        "disappoint", "complicated", "confusing", "unreliable", "overpriced",
        "lacking", "missing", "need", "wish", "want", "looking for",
        "alternative to", "replacement for", "better than", "instead of",
        "struggling with", "problem with", "issue with", "tired of",
    ];

    let mut pains: Vec<String> = Vec::new();

    for post in posts {
        let text = format!("{} {}", post.title, post.selftext).to_lowercase();
        for word in &pain_words {
            if text.contains(word) {
                // Extract the sentence containing the pain word
                let title = &post.title;
                if title.to_lowercase().contains(word) {
                    if !pains.iter().any(|p| p == title) {
                        pains.push(title.clone());
                    }
                }
                break;
            }
        }
    }

    pains.truncate(20);
    pains
}

// ── Hacker News ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HnPost {
    pub title: String,
    pub url: Option<String>,
    pub points: i64,
    pub num_comments: i64,
    pub hn_url: String,
}

/// Search Hacker News via Algolia API (free, no auth).
pub async fn hn_search(query: &str, limit: usize) -> Vec<HnPost> {
    let http = client();
    let url = format!(
        "https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}",
        urlenc(query),
        limit.min(50)
    );

    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let hits = match data.get("hits").and_then(|h| h.as_array()) {
        Some(h) => h,
        None => return vec![],
    };

    hits.iter()
        .filter_map(|hit| {
            let title = hit.get("title")?.as_str()?.to_string();
            let object_id = hit.get("objectID")?.as_str()?;
            Some(HnPost {
                title,
                url: hit.get("url").and_then(|u| u.as_str()).map(String::from),
                points: hit.get("points").and_then(|p| p.as_i64()).unwrap_or(0),
                num_comments: hit
                    .get("num_comments")
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0),
                hn_url: format!("https://news.ycombinator.com/item?id={}", object_id),
            })
        })
        .collect()
}

// ── Competitor Site Scraping ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompetitorProfile {
    pub domain: String,
    pub title: String,
    pub description: String,
    pub has_pricing: bool,
    pub pricing_url: Option<String>,
    pub has_signup: bool,
    pub has_free_trial: bool,
    pub tech_stack: Vec<String>,
    pub social_links: Vec<String>,
}

/// Actually visit a competitor's site and extract real data.
pub async fn scrape_competitor(domain: &str) -> Option<CompetitorProfile> {
    let http = client();
    let url = format!("https://{}", domain);

    let resp = http.get(&url).send().await.ok()?;
    let headers = resp.headers().clone();
    let html = resp.text().await.ok()?;
    let lower = html.to_lowercase();

    // Extract title
    let title = extract_between(&lower, "<title", "</title>")
        .and_then(|t| t.split('>').nth(1).map(|s| s.trim().to_string()))
        .unwrap_or_default();

    // Extract meta description
    let description = extract_meta_content(&html, "description").unwrap_or_default();

    // Check for pricing page
    let has_pricing = lower.contains("/pricing")
        || lower.contains("pricing-page")
        || lower.contains(">pricing<");
    let pricing_url = if has_pricing {
        Some(format!("https://{}/pricing", domain))
    } else {
        None
    };

    // Check for signup / free trial
    let has_signup = lower.contains("sign up")
        || lower.contains("signup")
        || lower.contains("get started")
        || lower.contains("create account");
    let has_free_trial = lower.contains("free trial")
        || lower.contains("try free")
        || lower.contains("start free")
        || lower.contains("free plan");

    // Tech stack from headers + HTML
    let tech_stack = detect_quick_tech(&lower, &headers);

    // Social links
    let social_links = extract_social_links(&html);

    Some(CompetitorProfile {
        domain: domain.to_string(),
        title: original_case_title(&html).unwrap_or(title),
        description,
        has_pricing,
        pricing_url,
        has_signup,
        has_free_trial,
        tech_stack,
        social_links,
    })
}

/// Scrape actual pricing page content.
pub async fn scrape_pricing(domain: &str) -> Option<String> {
    let http = client();
    for path in ["/pricing", "/plans", "/pricing.html"] {
        let url = format!("https://{}{}", domain, path);
        if let Ok(resp) = http.get(&url).send().await {
            if resp.status().is_success() {
                if let Ok(html) = resp.text().await {
                    // Extract price-like patterns
                    let re = regex::Regex::new(r"\$\d+[\d,.]*(?:/mo(?:nth)?|/yr|/year)?").ok()?;
                    let prices: Vec<String> = re
                        .find_iter(&html)
                        .map(|m| m.as_str().to_string())
                        .collect();
                    if !prices.is_empty() {
                        return Some(prices.join(", "));
                    }
                }
            }
        }
    }
    None
}

// ── Google SERP (real scraping) ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerpResult {
    pub position: u32,
    pub title: String,
    pub domain: String,
    pub url: String,
    pub snippet: String,
}

/// Scrape actual Google results.
pub async fn google_search(query: &str, num: u32) -> Vec<SerpResult> {
    let http = client();
    let url = format!(
        "https://www.google.com/search?q={}&num={}&gl=us&hl=en",
        urlenc(query),
        num
    );

    let html = match http.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };

    let doc = scraper::Html::parse_document(&html);
    let mut results = Vec::new();

    // Extract search results
    let h3_sel = scraper::Selector::parse("h3").unwrap();
    let link_sel = scraper::Selector::parse("a[href]").unwrap();

    let titles: Vec<String> = doc
        .select(&h3_sel)
        .map(|el| el.text().collect::<String>())
        .take(num as usize)
        .collect();

    let mut links = Vec::new();
    for el in doc.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if href.starts_with("http") && !href.contains("google.com") {
                if let Ok(parsed) = url::Url::parse(href) {
                    if let Some(host) = parsed.host_str() {
                        if !links.iter().any(|(_, d, _): &(String, String, String)| d == host) {
                            links.push((parsed.to_string(), host.to_string(), String::new()));
                        }
                    }
                }
            }
        }
    }

    for (i, title) in titles.iter().enumerate() {
        if let Some((url, domain, _)) = links.get(i) {
            results.push(SerpResult {
                position: (i + 1) as u32,
                title: title.clone(),
                domain: domain.clone(),
                url: url.clone(),
                snippet: String::new(),
            });
        }
    }

    results
}

// ── Google Suggest (expanded) ────────────────────────────────────────────

pub async fn google_suggest_expanded(query: &str) -> Vec<String> {
    let http = client();
    let mut all = google_suggest_raw(&http, query).await;

    // Focused expansion — fewer requests, higher quality
    let expansions: Vec<String> = [
        "best", "top", "free", "vs", "alternative", "review", "pricing",
        "open source", "how to", "tools", "software", "platform",
    ]
    .iter()
    .map(|s| format!("{} {}", query, s))
    .chain(
        ["what is", "how to", "why", "best"].iter()
            .map(|s| format!("{} {}", s, query))
    )
    .collect();

    for q in &expansions {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut extra = google_suggest_raw(&http, q).await;
        all.append(&mut extra);
    }

    all.sort();
    all.dedup();
    all
}

async fn google_suggest_raw(http: &reqwest::Client, q: &str) -> Vec<String> {
    let url = format!(
        "https://suggestqueries.google.com/complete/search?client=firefox&q={}&hl=en&gl=us",
        urlenc(q)
    );
    match http.get(&url).send().await {
        Ok(r) => {
            if let Ok(text) = r.text().await {
                if let Ok(val) = serde_json::from_str::<Value>(&text) {
                    if let Some(arr) = val.get(1).and_then(|v| v.as_array()) {
                        return arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                }
            }
            vec![]
        }
        Err(_) => vec![],
    }
}

// ── Ollama (local LLM) ──────────────────────────────────────────────────

/// Generate text using Ollama (open source local LLM). Returns None if Ollama isn't running.
pub async fn ollama_generate(prompt: &str, model: &str) -> Option<String> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .ok()?;

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048,
        }
    });

    let resp = http
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let data: Value = resp.json().await.ok()?;
    data.get("response")
        .and_then(|r| r.as_str())
        .map(|s| s.trim().to_string())
}

/// Check if Ollama is running and what models are available.
pub async fn ollama_status() -> Option<Vec<String>> {
    let http = client();
    let resp = http
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .ok()?;

    let data: Value = resp.json().await.ok()?;
    let models = data
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })?;

    Some(models)
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn extract_between<'a>(html: &'a str, start_tag: &str, end_tag: &str) -> Option<&'a str> {
    let start = html.find(start_tag)?;
    let end = html[start..].find(end_tag)?;
    Some(&html[start..start + end])
}

fn extract_meta_content(html: &str, name: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let pattern = format!("name=\"{}\"", name);
    let pos = lower.find(&pattern)?;
    let region = &html[pos.saturating_sub(200)..std::cmp::min(pos + 300, html.len())];
    let content_start = region.to_lowercase().find("content=\"")? + 9;
    let adjusted = &region[content_start..];
    let end = adjusted.find('"')?;
    Some(adjusted[..end].to_string())
}

fn original_case_title(html: &str) -> Option<String> {
    let start = html.to_lowercase().find("<title")?;
    let after_tag = html[start..].find('>')?;
    let content_start = start + after_tag + 1;
    let end = html[content_start..].to_lowercase().find("</title>")?;
    Some(html[content_start..content_start + end].trim().to_string())
}

fn detect_quick_tech(lower_html: &str, headers: &reqwest::header::HeaderMap) -> Vec<String> {
    let mut techs = Vec::new();
    if lower_html.contains("/_next/") { techs.push("Next.js".into()); }
    if lower_html.contains("react") { techs.push("React".into()); }
    if lower_html.contains("vue") { techs.push("Vue.js".into()); }
    if lower_html.contains("wp-content") { techs.push("WordPress".into()); }
    if lower_html.contains("shopify") { techs.push("Shopify".into()); }
    if lower_html.contains("stripe") { techs.push("Stripe".into()); }
    if lower_html.contains("intercom") { techs.push("Intercom".into()); }
    if lower_html.contains("hubspot") { techs.push("HubSpot".into()); }
    if lower_html.contains("tailwind") { techs.push("Tailwind".into()); }
    if headers.get("x-vercel-id").is_some() { techs.push("Vercel".into()); }
    if headers.get("cf-ray").is_some() { techs.push("Cloudflare".into()); }
    techs
}

fn extract_social_links(html: &str) -> Vec<String> {
    let re = regex::Regex::new(
        r#"href="(https?://(?:twitter\.com|x\.com|linkedin\.com|github\.com|facebook\.com|instagram\.com)/[^"]+)"#,
    )
    .unwrap();
    let mut links: Vec<String> = re
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    links.sort();
    links.dedup();
    links
}
