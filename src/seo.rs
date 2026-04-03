use clap::Subcommand;
use colored::Colorize;

use crate::display;

#[derive(Subcommand)]
pub enum SeoAction {
    /// Get keyword suggestions from Google Autocomplete
    #[command(alias = "kw")]
    Keywords {
        /// Seed keyword
        keyword: Vec<String>,
        /// Language (default: en)
        #[arg(short, long, default_value = "en")]
        lang: String,
        /// Country (default: us)
        #[arg(short, long, default_value = "us")]
        country: String,
    },
    /// Analyze Google SERP for a query
    #[command(alias = "s")]
    Serp {
        /// Search query to analyze
        query: Vec<String>,
        /// Number of results (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        num: u32,
    },
    /// Quick domain overview (title, meta, headers, structure)
    #[command(alias = "d")]
    Domain {
        /// Domain to analyze
        domain: String,
    },
    /// Find related questions people ask (PAA)
    #[command(alias = "q")]
    Questions {
        /// Topic to find questions about
        topic: Vec<String>,
    },
    /// Compare your domain vs a competitor
    #[command(alias = "vs")]
    Compare {
        /// Your domain
        domain: String,
        /// Competitor domain
        competitor: String,
        /// Keywords to compare (comma-separated)
        #[arg(short, long)]
        keywords: Option<String>,
    },
    /// Find content gaps vs competitors
    #[command(alias = "gap")]
    Gap {
        /// Your domain
        domain: String,
        /// Competitor domains (comma-separated)
        competitors: String,
        /// Topic/keyword area
        topic: Vec<String>,
    },
}

pub async fn run(action: SeoAction) -> Result<(), String> {
    match action {
        SeoAction::Keywords { keyword, lang, country } => keywords(&keyword.join(" "), &lang, &country).await,
        SeoAction::Serp { query, num } => serp(&query.join(" "), num).await,
        SeoAction::Domain { domain } => domain_overview(&domain).await,
        SeoAction::Questions { topic } => questions(&topic.join(" ")).await,
        SeoAction::Compare { domain, competitor, keywords } => compare(&domain, &competitor, keywords.as_deref()).await,
        SeoAction::Gap { domain, competitors, topic } => gap(&domain, &competitors, &topic.join(" ")).await,
    }
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn google_suggest(client: &reqwest::Client, keyword: &str, lang: &str, country: &str) -> Vec<String> {
    let url = format!(
        "https://suggestqueries.google.com/complete/search?client=firefox&q={}&hl={}&gl={}",
        urlenc(keyword), lang, country
    );
    match client.get(&url).send().await {
        Ok(resp) => {
            if let Ok(text) = resp.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get(1).and_then(|v| v.as_array()) {
                        return arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                    }
                }
            }
            vec![]
        }
        Err(_) => vec![],
    }
}

fn urlenc(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => result.push(byte as char),
            b' ' => result.push('+'),
            _ => { result.push('%'); result.push_str(&format!("{:02X}", byte)); }
        }
    }
    result
}

async fn keywords(keyword: &str, lang: &str, country: &str) -> Result<(), String> {
    if keyword.is_empty() { return Err("Keyword required".into()); }

    println!("  {} \"{}\" ({}/{})", "Keywords for:".dimmed(), keyword.bold(), lang, country);
    println!();

    let client = build_client();
    let mut all = google_suggest(&client, keyword, lang, country).await;

    // Expand with suffixes
    for suffix in ['a', 'b', 'c', 'h', 'w', 's', 't'] {
        let q = format!("{} {}", keyword, suffix);
        let mut extra = google_suggest(&client, &q, lang, country).await;
        all.append(&mut extra);
    }
    all.sort();
    all.dedup();

    for (i, s) in all.iter().enumerate() {
        let highlighted = if s.to_lowercase().starts_with(&keyword.to_lowercase()) {
            format!("{}{}", keyword.green(), &s[keyword.len()..])
        } else {
            s.clone()
        };
        println!("  {:>3}  {}", (i + 1).to_string().dimmed(), highlighted);
    }
    println!();
    println!("  {} suggestions", all.len().to_string().green().bold());
    println!();
    Ok(())
}

async fn serp(query: &str, num: u32) -> Result<(), String> {
    if query.is_empty() { return Err("Query required".into()); }

    println!("  {} \"{}\"", "SERP analysis:".dimmed(), query.bold());
    println!();

    let client = build_client();
    let url = format!(
        "https://www.google.com/search?q={}&num={}&gl=us&hl=en",
        urlenc(query), num
    );

    let html = client.get(&url).send().await
        .map_err(|e| format!("Google fetch failed: {}", e))?
        .text().await
        .map_err(|e| format!("Read failed: {}", e))?;

    let doc = scraper::Html::parse_document(&html);
    let title_sel = scraper::Selector::parse("h3").unwrap();
    let link_sel = scraper::Selector::parse("a[href]").unwrap();

    let titles: Vec<String> = doc.select(&title_sel)
        .map(|el| el.text().collect::<String>())
        .take(num as usize)
        .collect();

    let mut domains = Vec::new();
    for el in doc.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if href.starts_with("http") && !href.contains("google.com") {
                if let Ok(parsed) = url::Url::parse(href) {
                    if let Some(host) = parsed.host_str() {
                        let h = host.to_string();
                        if !domains.contains(&h) { domains.push(h); }
                    }
                }
            }
        }
    }
    domains.truncate(num as usize);

    let rows: Vec<Vec<String>> = titles.iter().enumerate().map(|(i, t)| {
        let domain = domains.get(i).cloned().unwrap_or_default();
        vec![
            (i + 1).to_string().dimmed().to_string(),
            display::truncate(t, 55),
            domain.cyan().to_string(),
        ]
    }).collect();

    display::table(&["#", "Title", "Domain"], &rows);

    if !titles.is_empty() {
        let avg_len = titles.iter().map(|t| t.len()).sum::<usize>() / titles.len();
        display::kv("Avg title length", &format!("{} chars", avg_len));
    }
    display::kv("Domains ranking", &domains.len().to_string());
    println!();
    Ok(())
}

async fn domain_overview(domain: &str) -> Result<(), String> {
    // Delegate to recon domain (same logic)
    super::recon::run(super::recon::ReconAction::Domain { domain: domain.to_string() }).await
}

async fn questions(topic: &str) -> Result<(), String> {
    if topic.is_empty() { return Err("Topic required".into()); }

    println!("  {} \"{}\"", "Questions about:".dimmed(), topic.bold());
    println!();

    let client = build_client();
    let prefixes = ["what is", "how to", "why does", "when to", "where to", "who uses", "can you", "is it", "does", "which"];

    let mut questions = Vec::new();
    for prefix in &prefixes {
        let q = format!("{} {}", prefix, topic);
        let suggestions = google_suggest(&client, &q, "en", "us").await;
        for s in suggestions {
            let l = s.to_lowercase();
            if (l.contains('?') || l.starts_with("what ") || l.starts_with("how ") ||
                l.starts_with("why ") || l.starts_with("when ") || l.starts_with("who ") ||
                l.starts_with("can ") || l.starts_with("is ") || l.starts_with("does ") ||
                l.starts_with("which ")) && !questions.contains(&s)
            {
                questions.push(s);
            }
        }
    }

    for (i, q) in questions.iter().enumerate() {
        println!("  {:>3}  {}", (i + 1).to_string().dimmed(), q);
    }
    println!();
    println!("  {} questions found", questions.len().to_string().green().bold());
    println!();
    Ok(())
}

async fn compare(domain: &str, competitor: &str, keywords: Option<&str>) -> Result<(), String> {
    println!("  {} {} vs {}", "Comparing:".dimmed(), domain.green(), competitor.yellow());
    println!();

    let client = build_client();
    let your = google_suggest(&client, domain, "en", "us").await;
    let comp = google_suggest(&client, competitor, "en", "us").await;

    let your_set: std::collections::HashSet<_> = your.iter().collect();
    let comp_set: std::collections::HashSet<_> = comp.iter().collect();
    let shared: Vec<_> = your_set.intersection(&comp_set).collect();
    let unique_you: Vec<_> = your_set.difference(&comp_set).collect();
    let unique_comp: Vec<_> = comp_set.difference(&your_set).collect();

    display::kv("Your suggestions", &your.len().to_string());
    display::kv("Competitor's", &comp.len().to_string());
    display::kv("Shared", &shared.len().to_string().yellow().to_string());
    display::kv("Unique to you", &unique_you.len().to_string().green().to_string());
    display::kv("Unique to them", &unique_comp.len().to_string().red().to_string());

    if let Some(kws) = keywords {
        println!();
        println!("  {}", "Keyword presence:".bold());
        for kw in kws.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let your_kw = google_suggest(&client, &format!("{} {}", domain, kw), "en", "us").await;
            let comp_kw = google_suggest(&client, &format!("{} {}", competitor, kw), "en", "us").await;
            println!("    {}: you={} them={}", kw.bold(), your_kw.len().to_string().green(), comp_kw.len().to_string().yellow());
        }
    }

    println!();
    Ok(())
}

async fn gap(domain: &str, competitors: &str, topic: &str) -> Result<(), String> {
    if topic.is_empty() { return Err("Topic required".into()); }

    println!("  {} for {} on \"{}\"", "Content gaps".dimmed(), domain.green(), topic.bold());
    println!();

    let client = build_client();
    let your = google_suggest(&client, &format!("{} {}", domain, topic), "en", "us").await;
    let your_set: std::collections::HashSet<String> = your.into_iter().collect();

    let mut gaps = Vec::new();
    for comp in competitors.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let comp_suggestions = google_suggest(&client, &format!("{} {}", comp, topic), "en", "us").await;
        for s in &comp_suggestions {
            if !your_set.contains(s) && !gaps.contains(s) {
                gaps.push(s.clone());
            }
        }
    }

    if gaps.is_empty() {
        println!("  {}", "No content gaps found — you're covering this topic well!".green());
    } else {
        println!("  {}", "Topics competitors cover that you don't:".bold());
        for (i, g) in gaps.iter().enumerate() {
            println!("  {:>3}  {}", (i + 1).to_string().dimmed(), g.yellow());
        }
        println!();
        println!("  {} content gaps found", gaps.len().to_string().red().bold());
    }

    println!();
    Ok(())
}
