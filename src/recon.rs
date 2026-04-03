use clap::Subcommand;
use colored::Colorize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;

use crate::display;

// 5,201 tech fingerprints from wappalyzergo (embedded at compile time)
const TECH_FINGERPRINTS_RAW: &str = include_str!("../data/tech_fingerprints.json");

#[derive(Subcommand)]
pub enum ReconAction {
    /// Look up a domain: DNS, MX, tech stack, meta tags, social profiles
    #[command(alias = "d")]
    Domain {
        /// Domain to investigate (e.g. stripe.com)
        domain: String,
    },
    /// Verify an email address (MX + SMTP + disposable check)
    #[command(alias = "e")]
    Email {
        /// Email to verify
        email: String,
    },
    /// Find probable email addresses for a person at a company
    #[command(alias = "p")]
    Person {
        /// First name
        first: String,
        /// Last name
        last: String,
        /// Company domain
        domain: String,
    },
    /// Detect tech stack from a website's HTML/headers
    #[command(alias = "t")]
    Tech {
        /// Domain or URL
        domain: String,
    },
    /// Check if a company is actively hiring
    #[command(alias = "h")]
    Hiring {
        /// Company domain
        domain: String,
    },
    /// Reverse DNS lookup (A, MX, NS records)
    #[command(alias = "dns")]
    Dns {
        /// Domain to look up
        domain: String,
    },
    /// Find ALL emails at a domain (like Hunter.io domain search)
    #[command(alias = "em")]
    Emails {
        /// Target domain (e.g. stripe.com)
        domain: String,
        /// Max results (default: 50)
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,
    },
    /// Find subdomains via crt.sh, RapidDNS, HackerTarget (no API key)
    #[command(alias = "sub")]
    Subdomains {
        /// Target domain
        domain: String,
    },
    /// Find historical URLs from Wayback Machine
    #[command(alias = "wb")]
    Wayback {
        /// Target domain
        domain: String,
        /// Max URLs to return (default: 100)
        #[arg(short = 'n', long, default_value = "100")]
        limit: usize,
    },
}

pub async fn run(action: ReconAction) -> Result<(), String> {
    match action {
        ReconAction::Domain { domain } => domain_recon(&domain).await,
        ReconAction::Email { email } => verify_email(&email).await,
        ReconAction::Person { first, last, domain } => find_person(&first, &last, &domain).await,
        ReconAction::Tech { domain } => tech_stack(&domain).await,
        ReconAction::Hiring { domain } => hiring_check(&domain).await,
        ReconAction::Dns { domain } => dns_lookup(&domain).await,
        ReconAction::Emails { domain, limit } => find_domain_emails(&domain, limit).await,
        ReconAction::Subdomains { domain } => subdomains(&domain).await,
        ReconAction::Wayback { domain, limit } => wayback(&domain, limit).await,
    }
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn clean_domain(d: &str) -> String {
    d.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

// ── Domain Recon ─────────────────────────────────────────────────────────

async fn domain_recon(domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!("  {} {}", "Investigating:".dimmed(), domain.bold());
    println!();

    let client = build_client();
    let url = format!("https://{}", domain);

    // Fetch homepage
    let (status, headers, html) = match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let headers = resp.headers().clone();
            let html = resp.text().await.unwrap_or_default();
            (status, headers, html)
        }
        Err(e) => return Err(format!("Failed to reach {}: {}", domain, e)),
    };

    // Extract meta
    let doc = scraper::Html::parse_document(&html);
    let title = doc
        .select(&scraper::Selector::parse("title").unwrap())
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();
    let description = doc
        .select(&scraper::Selector::parse("meta[name='description']").unwrap())
        .next()
        .and_then(|el| el.value().attr("content").map(String::from))
        .unwrap_or_default();
    let server = headers
        .get("server")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Tech stack
    let techs = detect_techs(&html, &headers);

    // Social profiles
    let slug = domain.split('.').next().unwrap_or(&domain);

    // DNS (via dig)
    let mx = run_dig(&domain, "MX").await;
    let a_records = run_dig(&domain, "A").await;
    let ns = run_dig(&domain, "NS").await;

    // Display
    display::header(&format!("{} — {}", domain, title));

    display::kv("Status", &format!("{}", status));
    display::kv("Server", server);
    display::kv("Title", &display::truncate(&title, 60));
    if !description.is_empty() {
        display::kv("Description", &display::truncate(&description, 80));
    }
    display::kv("HTML size", &format!("{:.0} KB", html.len() as f64 / 1024.0));

    println!();
    println!("  {}", "Tech Stack".bold());
    if techs.is_empty() {
        println!("    {}", "No technologies detected".dimmed());
    } else {
        for t in &techs {
            println!("    {} {}", "•".cyan(), t);
        }
    }

    println!();
    println!("  {}", "DNS Records".bold());
    if !a_records.is_empty() {
        display::kv("A", &a_records.join(", "));
    }
    if !mx.is_empty() {
        display::kv("MX", &mx.join(", "));
    }
    if !ns.is_empty() {
        display::kv("NS", &ns.join(", "));
    }

    println!();
    println!("  {}", "Social Profiles".bold());
    println!("    {} https://linkedin.com/company/{}", "LinkedIn".cyan(), slug);
    println!("    {} https://twitter.com/{}", "Twitter".cyan(), slug);
    println!("    {} https://github.com/{}", "GitHub".cyan(), slug);

    println!();
    Ok(())
}

// ── Email Verify ─────────────────────────────────────────────────────────

// 5,359 disposable email domains (from disposable-email-domains project)
const DISPOSABLE_DOMAINS_RAW: &str = include_str!("../data/disposable_domains.txt");

fn is_disposable(domain: &str) -> bool {
    let lower = domain.to_lowercase();
    DISPOSABLE_DOMAINS_RAW.lines().any(|d| d.trim() == lower)
}

async fn verify_email(email: &str) -> Result<(), String> {
    println!("  {} {}", "Verifying:".dimmed(), email.bold());
    println!();

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err("Invalid email format".into());
    }
    let domain = parts[1];

    // Check disposable (5,359 domains)
    let is_disposable = is_disposable(domain);

    // MX lookup
    let mx = run_dig(domain, "MX").await;
    let has_mx = !mx.is_empty();

    // SMTP check (try to connect to MX and RCPT TO)
    let smtp_result = if has_mx {
        check_smtp(email, &mx[0]).await
    } else {
        "no MX records".to_string()
    };

    // Determine email provider
    let provider = if mx.iter().any(|m| m.contains("google")) {
        "Google Workspace"
    } else if mx.iter().any(|m| m.contains("outlook") || m.contains("microsoft")) {
        "Microsoft 365"
    } else if mx.iter().any(|m| m.contains("zoho")) {
        "Zoho"
    } else if mx.iter().any(|m| m.contains("proton")) {
        "ProtonMail"
    } else {
        "Other"
    };

    let verdict = if is_disposable {
        "DISPOSABLE".red().bold()
    } else if !has_mx {
        "INVALID (no MX)".red().bold()
    } else if smtp_result.contains("250") {
        "VALID".green().bold()
    } else {
        "UNCERTAIN".yellow().bold()
    };

    display::kv("Email", email);
    display::kv("Domain", domain);
    display::kv("Verdict", &verdict.to_string());
    display::kv("MX Records", &if has_mx { mx.join(", ") } else { "none".to_string() });
    display::kv("Provider", provider);
    display::kv("Disposable", &if is_disposable { "YES".red().to_string() } else { "no".green().to_string() });
    display::kv("SMTP Check", &smtp_result);

    println!();
    Ok(())
}

/// Read an SMTP response line (wait for line ending with \r\n, return status code + text)
async fn smtp_read_response(stream: &mut TcpStream) -> Result<(u16, String), String> {
    let mut buf = vec![0u8; 4096];
    let mut response = String::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
            Ok(Ok(0)) => return Err("connection closed".into()),
            Ok(Ok(n)) => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains("\r\n") {
                    break;
                }
            }
            Ok(Err(e)) => return Err(format!("read error: {}", e)),
            Err(_) => return Err("timeout".into()),
        }
    }
    let code = response
        .get(..3)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    Ok((code, response.trim().to_string()))
}

/// Send an SMTP command and read the response
async fn smtp_command(stream: &mut TcpStream, cmd: &str) -> Result<(u16, String), String> {
    let data = format!("{}\r\n", cmd);
    tokio::time::timeout(Duration::from_secs(5), stream.write_all(data.as_bytes()))
        .await
        .map_err(|_| "write timeout".to_string())?
        .map_err(|e| format!("write error: {}", e))?;
    smtp_read_response(stream).await
}

/// Pure-Rust SMTP verification using tokio TcpStream
async fn check_smtp(email: &str, mx_host: &str) -> String {
    // Clean MX host (remove trailing dot, priority prefix like "10 mx.example.com")
    let host = mx_host
        .split_whitespace()
        .last()
        .unwrap_or(mx_host)
        .trim_end_matches('.');
    let addr = format!("{}:25", host);

    // Connect with timeout
    let mut stream = match tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return format!("connect failed: {}", e),
        Err(_) => return "connect timeout".into(),
    };

    // Read banner
    match smtp_read_response(&mut stream).await {
        Ok((code, _)) if code >= 200 && code < 300 => {}
        Ok((code, msg)) => return format!("banner rejected: {} {}", code, msg),
        Err(e) => return format!("banner: {}", e),
    }

    // EHLO
    match smtp_command(&mut stream, "EHLO check.local").await {
        Ok((code, _)) if code >= 200 && code < 300 => {}
        Ok((code, msg)) => return format!("EHLO rejected: {} {}", code, msg),
        Err(e) => return format!("EHLO: {}", e),
    }

    // MAIL FROM
    match smtp_command(&mut stream, "MAIL FROM:<test@check.local>").await {
        Ok((code, _)) if code >= 200 && code < 300 => {}
        Ok((code, msg)) => return format!("MAIL FROM rejected: {} {}", code, msg),
        Err(e) => return format!("MAIL FROM: {}", e),
    }

    // RCPT TO — this is the actual verification
    let rcpt_result = match smtp_command(
        &mut stream,
        &format!("RCPT TO:<{}>", email),
    )
    .await
    {
        Ok((code, msg)) => (code, msg),
        Err(e) => return format!("RCPT TO: {}", e),
    };

    // Catch-all detection: test with random address
    let catch_all = {
        let random_addr = format!("{}@{}", uuid_v4_simple(), email.split('@').nth(1).unwrap_or(""));
        match smtp_command(
            &mut stream,
            &format!("RCPT TO:<{}>", random_addr),
        )
        .await
        {
            Ok((code, _)) if code >= 200 && code < 300 => true,
            _ => false,
        }
    };

    // QUIT
    let _ = smtp_command(&mut stream, "QUIT").await;

    let (code, msg) = rcpt_result;
    let verdict = if code >= 200 && code < 300 {
        if catch_all {
            format!("{} (catch-all server — accepts everything)", msg)
        } else {
            msg
        }
    } else {
        msg
    };
    verdict
}

/// Simple pseudo-UUID v4 for catch-all detection (no external crate needed)
fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("test{:x}{:x}", seed & 0xFFFFFFFF, (seed >> 32) & 0xFFFF)
}

// ── Person Finder ────────────────────────────────────────────────────────

async fn find_person(first: &str, last: &str, domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!(
        "  {} {} {} @ {}",
        "Finding:".dimmed(),
        first.bold(),
        last.bold(),
        domain.cyan()
    );
    println!();

    let first_l = first.to_lowercase();
    let last_l = last.to_lowercase();
    let fi = &first_l[..1]; // first initial

    // Generate email patterns
    let patterns = vec![
        format!("{}@{}", first_l, domain),
        format!("{}.{}@{}", first_l, last_l, domain),
        format!("{}{}@{}", fi, last_l, domain),
        format!("{}_{}@{}", first_l, last_l, domain),
        format!("{}{}@{}", first_l, &last_l[..1], domain),
        format!("{}.{}@{}", fi, last_l, domain),
        format!("{}@{}", last_l, domain),
    ];

    // Check MX first
    let mx = run_dig(&domain, "MX").await;
    if mx.is_empty() {
        return Err(format!("No MX records for {} — domain may not have email", domain));
    }

    let provider = if mx.iter().any(|m| m.contains("google")) {
        "Google Workspace"
    } else if mx.iter().any(|m| m.contains("outlook") || m.contains("microsoft")) {
        "Microsoft 365"
    } else {
        "Other"
    };

    display::kv("Domain", &domain);
    display::kv("Provider", provider);
    display::kv("MX", &mx[0]);
    println!();
    println!("  {}", "Email candidates (most likely first):".bold());
    println!();

    let rows: Vec<Vec<String>> = patterns
        .iter()
        .enumerate()
        .map(|(i, email)| {
            let confidence = match i {
                0 => "●●●●●",
                1 => "●●●●●",
                2 => "●●●●",
                3 => "●●●",
                4 => "●●●",
                5 => "●●",
                _ => "●",
            };
            vec![
                confidence.yellow().to_string(),
                email.clone(),
            ]
        })
        .collect();

    display::table(&["Likely", "Email"], &rows);
    println!(
        "  {} {}",
        "Verify with:".dimmed(),
        format!("dx recon email {}", patterns[1]).cyan()
    );
    println!();
    Ok(())
}

// ── Tech Stack ───────────────────────────────────────────────────────────

async fn tech_stack(domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!("  {} {}", "Scanning:".dimmed(), domain.bold());

    let client = build_client();
    let url = format!("https://{}", domain);

    let resp = client.get(&url).send().await
        .map_err(|e| format!("Failed to reach {}: {}", domain, e))?;

    let headers = resp.headers().clone();
    let html = resp.text().await.unwrap_or_default();
    let techs = detect_techs(&html, &headers);

    display::header(&format!("{} — Tech Stack", domain));

    if techs.is_empty() {
        println!("  {}", "No technologies detected from HTML/headers.".dimmed());
    } else {
        let rows: Vec<Vec<String>> = techs
            .iter()
            .map(|t| vec![t.clone()])
            .collect();
        display::table(&["Technology"], &rows);
        println!("  {} technologies detected", techs.len().to_string().green().bold());
    }

    // Show relevant headers
    println!();
    println!("  {}", "Revealing Headers".bold());
    for key in &["server", "x-powered-by", "x-frame-options", "x-content-type-options", "strict-transport-security", "content-security-policy"] {
        if let Some(val) = headers.get(*key).and_then(|v| v.to_str().ok()) {
            display::kv(key, &display::truncate(val, 60));
        }
    }

    println!();
    Ok(())
}

/// Strip wappalyzer version extraction suffix (everything after `\;`)
fn strip_version_suffix(pattern: &str) -> &str {
    if let Some(pos) = pattern.find("\\;") {
        &pattern[..pos]
    } else {
        pattern
    }
}

/// Try to match a pattern — first as regex, fall back to case-insensitive contains
fn pattern_matches(haystack: &str, raw_pattern: &str) -> bool {
    let pattern = strip_version_suffix(raw_pattern);
    if pattern.is_empty() {
        return false;
    }
    // Try regex first
    match regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(1 << 20) // 1MB limit to avoid pathological patterns
        .build()
    {
        Ok(re) => re.is_match(haystack),
        Err(_) => {
            // Fall back to simple case-insensitive contains
            haystack.to_lowercase().contains(&pattern.to_lowercase())
        }
    }
}

/// Extract all <script src="..."> values from HTML
fn extract_script_srcs(html: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"<script[^>]+src=["']([^"']+)["']"#).unwrap();
    re.captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Extract <meta name="X" content="Y"> pairs from HTML
fn extract_meta_tags(html: &str) -> HashMap<String, String> {
    let mut metas = HashMap::new();
    let re = regex::Regex::new(
        r#"(?i)<meta\s+(?:name|property)=["']([^"']+)["']\s+content=["']([^"']*?)["']"#
    ).unwrap();
    for cap in re.captures_iter(html) {
        if let (Some(name), Some(content)) = (cap.get(1), cap.get(2)) {
            metas.insert(name.as_str().to_lowercase(), content.as_str().to_string());
        }
    }
    // Also match reversed order: content before name
    let re2 = regex::Regex::new(
        r#"(?i)<meta\s+content=["']([^"']*?)["']\s+(?:name|property)=["']([^"']+)["']"#
    ).unwrap();
    for cap in re2.captures_iter(html) {
        if let (Some(content), Some(name)) = (cap.get(1), cap.get(2)) {
            metas.entry(name.as_str().to_lowercase())
                .or_insert_with(|| content.as_str().to_string());
        }
    }
    metas
}

fn detect_techs(html: &str, headers: &reqwest::header::HeaderMap) -> Vec<String> {
    // Parse fingerprints (cached parse — happens once per call, but the JSON is embedded)
    let fingerprints: HashMap<String, Value> = match serde_json::from_str(TECH_FINGERPRINTS_RAW) {
        Ok(v) => v,
        Err(_) => return vec!["[fingerprint parse error]".into()],
    };

    let script_srcs = extract_script_srcs(html);
    let meta_tags = extract_meta_tags(html);

    // Build a lowercase header map for matching
    let mut header_map: HashMap<String, String> = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(val_str) = value.to_str() {
            header_map
                .entry(name.as_str().to_lowercase())
                .and_modify(|existing| {
                    existing.push_str("; ");
                    existing.push_str(val_str);
                })
                .or_insert_with(|| val_str.to_string());
        }
    }

    let mut techs: Vec<String> = Vec::new();

    for (tech_name, info) in &fingerprints {
        let mut matched = false;

        // Match HTML patterns
        if !matched {
            if let Some(html_patterns) = info.get("html").and_then(|v| v.as_array()) {
                for pat_val in html_patterns {
                    if let Some(pat) = pat_val.as_str() {
                        if pattern_matches(html, pat) {
                            matched = true;
                            break;
                        }
                    }
                }
            }
        }

        // Match script src patterns
        if !matched {
            if let Some(script_patterns) = info.get("scripts").and_then(|v| v.as_array()) {
                for pat_val in script_patterns {
                    if let Some(pat) = pat_val.as_str() {
                        let clean = strip_version_suffix(pat);
                        if !clean.is_empty() {
                            for src in &script_srcs {
                                if pattern_matches(src, clean) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        if matched { break; }
                    }
                }
            }
        }

        // Match header patterns
        if !matched {
            if let Some(header_patterns) = info.get("headers").and_then(|v| v.as_object()) {
                for (header_name, pat_val) in header_patterns {
                    if let Some(pat) = pat_val.as_str() {
                        let hdr_key = header_name.to_lowercase();
                        if let Some(hdr_val) = header_map.get(&hdr_key) {
                            let clean = strip_version_suffix(pat);
                            if clean.is_empty() {
                                // Empty pattern = just check header exists
                                matched = true;
                            } else if pattern_matches(hdr_val, clean) {
                                matched = true;
                            }
                        }
                    }
                    if matched { break; }
                }
            }
        }

        // Match meta tag patterns
        if !matched {
            if let Some(meta_patterns) = info.get("meta").and_then(|v| v.as_object()) {
                for (meta_name, pat_val) in meta_patterns {
                    let meta_key = meta_name.to_lowercase();
                    if let Some(meta_content) = meta_tags.get(&meta_key) {
                        if let Some(pats) = pat_val.as_array() {
                            for p in pats {
                                if let Some(pat) = p.as_str() {
                                    if pattern_matches(meta_content, pat) {
                                        matched = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if matched { break; }
                }
            }
        }

        if matched {
            techs.push(tech_name.clone());
        }
    }

    techs.sort();
    techs
}

// ── Hiring Check ─────────────────────────────────────────────────────────

async fn hiring_check(domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!("  {} {}", "Checking hiring signals:".dimmed(), domain.bold());
    println!();

    let client = build_client();
    let paths = ["/careers", "/jobs", "/join", "/hiring", "/work-with-us", "/join-us"];
    let mut found = Vec::new();

    for path in &paths {
        let url = format!("https://{}{}", domain, path);
        match client.head(&url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 301 || resp.status().as_u16() == 302 => {
                found.push((path.to_string(), resp.status().as_u16()));
            }
            _ => {}
        }
    }

    // Check for common job board integrations
    let main_html = match client.get(&format!("https://{}", domain)).send().await {
        Ok(r) => r.text().await.unwrap_or_default().to_lowercase(),
        Err(_) => String::new(),
    };

    let boards: Vec<&str> = [
        ("lever.co", "Lever"),
        ("greenhouse.io", "Greenhouse"),
        ("ashbyhq.com", "Ashby"),
        ("workable.com", "Workable"),
        ("jobs.lever.co", "Lever"),
        ("boards.greenhouse.io", "Greenhouse"),
        ("apply.workable.com", "Workable"),
    ]
    .iter()
    .filter(|(pattern, _)| main_html.contains(pattern))
    .map(|(_, name)| *name)
    .collect();

    if found.is_empty() && boards.is_empty() {
        println!("  {} No hiring pages found for {}", "−".dimmed(), domain);
    } else {
        if !found.is_empty() {
            println!("  {}", "Career pages found:".bold());
            for (path, status) in &found {
                println!("    {} https://{}{} ({})", "✓".green(), domain, path, status);
            }
        }
        if !boards.is_empty() {
            println!();
            println!("  {}", "Job board integrations:".bold());
            for board in &boards {
                println!("    {} {}", "•".cyan(), board);
            }
        }
        println!();
        println!("  {} {} is actively hiring", "→".green().bold(), domain);
    }

    println!();
    Ok(())
}

// ── DNS Lookup ───────────────────────────────────────────────────────────

async fn dns_lookup(domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    display::header(&format!("DNS: {}", domain));

    let a = run_dig(&domain, "A").await;
    let aaaa = run_dig(&domain, "AAAA").await;
    let mx = run_dig(&domain, "MX").await;
    let ns = run_dig(&domain, "NS").await;
    let txt = run_dig(&domain, "TXT").await;
    let cname = run_dig(&domain, "CNAME").await;

    if !a.is_empty() { display::kv("A", &a.join("\n                ")); }
    if !aaaa.is_empty() { display::kv("AAAA", &aaaa.join("\n                ")); }
    if !cname.is_empty() { display::kv("CNAME", &cname.join("\n                ")); }
    if !mx.is_empty() { display::kv("MX", &mx.join("\n                ")); }
    if !ns.is_empty() { display::kv("NS", &ns.join("\n                ")); }
    if !txt.is_empty() {
        println!();
        println!("  {}", "TXT Records".bold());
        for record in &txt {
            println!("    {}", record.dimmed());
        }
    }

    println!();
    Ok(())
}

// ── Domain Email Finder (Hunter.io killer) ───────────────────────────────

async fn find_domain_emails(domain: &str, limit: usize) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!("  {} {} (multi-source scan)", "Finding emails at:".dimmed(), domain.bold());
    println!();

    let http = build_client();
    let mut all_emails: Vec<(String, String)> = Vec::new(); // (email, source)

    // Source 1: Scrape the company's own website for emails
    println!("  {} Website scrape...", "→".cyan());
    let email_re = regex::Regex::new(&format!(r"[a-zA-Z0-9._%+-]+@{}", regex::escape(&domain))).unwrap();
    let paths = ["", "/about", "/contact", "/team", "/about-us", "/contact-us",
                  "/legal", "/privacy", "/careers", "/support", "/company"];
    for path in &paths {
        let url = format!("https://{}{}", domain, path);
        if let Ok(resp) = http.get(&url).send().await {
            if let Ok(html) = resp.text().await {
                for m in email_re.find_iter(&html) {
                    let email = m.as_str().to_lowercase();
                    if !all_emails.iter().any(|(e, _)| e == &email) && !email.contains("example") && !email.contains("test@") {
                        all_emails.push((email, format!("website{}", path)));
                    }
                }
            }
        }
    }
    let site_count = all_emails.iter().filter(|(_, s)| s.starts_with("website")).count();
    println!("    {} from website scrape ({} pages)", site_count.to_string().green(), paths.len());

    // Source 2: DuckDuckGo search (less rate-limited than Google)
    println!("  {} DuckDuckGo...", "→".cyan());
    let ddg_url = format!(
        "https://html.duckduckgo.com/html/?q=%22%40{}%22",
        domain
    );
    if let Ok(resp) = http.get(&ddg_url).send().await {
        if let Ok(html) = resp.text().await {
            for m in email_re.find_iter(&html) {
                let email = m.as_str().to_lowercase();
                if !all_emails.iter().any(|(e, _)| e == &email) && !email.contains("example") {
                    all_emails.push((email, "duckduckgo".into()));
                }
            }
        }
    }
    let ddg_count = all_emails.iter().filter(|(_, s)| s == "duckduckgo").count();
    println!("    {} from DuckDuckGo", ddg_count.to_string().green());

    // Source 2: crt.sh certificate transparency (emails in certs)
    println!("  {} crt.sh...", "→".cyan());
    let crt_url = format!("https://crt.sh/?q=%25.{}&output=json", domain);
    if let Ok(resp) = http.get(&crt_url).send().await {
        if let Ok(items) = resp.json::<Vec<Value>>().await {
            let email_re = regex::Regex::new(&format!(r"[a-zA-Z0-9._%+-]+@{}", regex::escape(&domain))).unwrap();
            for item in &items {
                if let Some(names) = item.get("name_value").and_then(|v| v.as_str()) {
                    for m in email_re.find_iter(names) {
                        let email = m.as_str().to_lowercase();
                        if !all_emails.iter().any(|(e, _)| e == &email) {
                            all_emails.push((email, "crt.sh".into()));
                        }
                    }
                }
            }
        }
    }
    let crt_count = all_emails.iter().filter(|(_, s)| s == "crt.sh").count();
    println!("    {} from crt.sh", crt_count.to_string().green());

    // Source 3: HackerTarget API
    println!("  {} HackerTarget...", "→".cyan());
    let ht_url = format!("https://api.hackertarget.com/hostsearch/?q={}", domain);
    if let Ok(resp) = http.get(&ht_url).send().await {
        if let Ok(text) = resp.text().await {
            let email_re = regex::Regex::new(&format!(r"[a-zA-Z0-9._%+-]+@{}", regex::escape(&domain))).unwrap();
            for m in email_re.find_iter(&text) {
                let email = m.as_str().to_lowercase();
                if !all_emails.iter().any(|(e, _)| e == &email) {
                    all_emails.push((email, "hackertarget".into()));
                }
            }
        }
    }

    // Source 4: Generate common role-based patterns + SMTP verify
    println!("  {} Pattern generation + SMTP verify...", "→".cyan());
    let common_roles = [
        "info", "hello", "contact", "sales", "support", "team", "admin",
        "hr", "careers", "press", "media", "legal", "billing", "ceo",
        "cto", "founder", "engineering", "marketing", "partnerships",
        "investors", "privacy", "security", "abuse", "postmaster", "webmaster",
    ];

    let mx = run_dig(&domain, "MX").await;
    let mx_host = mx.first().cloned().unwrap_or_default();

    if !mx_host.is_empty() {
        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let mut handles = Vec::new();

        for role in &common_roles {
            let email = format!("{}@{}", role, domain);
            if all_emails.iter().any(|(e, _)| e == &email) { continue; }

            let mx_h = mx_host.clone();
            let em = email.clone();
            let sem = sem.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let result = check_smtp(&em, &mx_h).await;
                let valid = result.contains("250") && !result.contains("catch-all");
                (em, valid, result)
            }));
        }

        let mut verified_count = 0;
        for handle in handles {
            if let Ok((email, valid, _result)) = handle.await {
                if valid {
                    if !all_emails.iter().any(|(e, _)| e == &email) {
                        all_emails.push((email, "smtp-verified".into()));
                        verified_count += 1;
                    }
                }
            }
        }
        println!("    {} SMTP-verified role addresses", verified_count.to_string().green());
    } else {
        println!("    {} No MX records — skipping SMTP", "−".dimmed());
    }

    // Deduplicate and sort
    all_emails.sort_by(|a, b| a.0.cmp(&b.0));
    all_emails.dedup_by(|a, b| a.0 == b.0);
    all_emails.truncate(limit);

    // Display
    println!();
    if all_emails.is_empty() {
        println!("  {} No emails found for {}", "−".dimmed(), domain);
    } else {
        let rows: Vec<Vec<String>> = all_emails.iter().map(|(email, source)| {
            let source_colored = match source.as_str() {
                "smtp-verified" => source.green().to_string(),
                "google" => source.cyan().to_string(),
                "crt.sh" => source.yellow().to_string(),
                _ => source.dimmed().to_string(),
            };
            vec![email.clone(), source_colored]
        }).collect();

        display::table(&["Email", "Source"], &rows);
        println!(
            "  {} emails found at {}",
            all_emails.len().to_string().green().bold(),
            domain
        );
    }

    // Provider info
    if !mx.is_empty() {
        let provider = if mx.iter().any(|m| m.contains("google")) {
            "Google Workspace"
        } else if mx.iter().any(|m| m.contains("outlook") || m.contains("microsoft")) {
            "Microsoft 365"
        } else if mx.iter().any(|m| m.contains("zoho")) {
            "Zoho"
        } else {
            "Other"
        };
        display::kv("Email provider", provider);
    }

    println!();
    Ok(())
}

// ── Subdomains ───────────────────────────────────────────────────────────

async fn subdomains(domain: &str) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!(
        "  {} {} (crt.sh + RapidDNS + HackerTarget)",
        "Finding subdomains:".dimmed(),
        domain.bold()
    );
    println!();

    let results = crate::osint::find_subdomains(&domain).await;

    if results.is_empty() {
        println!("  {}", "No subdomains found.".dimmed());
    } else {
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|(sub, source)| vec![sub.clone(), source.dimmed().to_string()])
            .collect();
        display::table(&["Subdomain", "Source"], &rows);
        println!(
            "  {} unique subdomains from 3 sources",
            results.len().to_string().green().bold()
        );
    }

    println!();
    Ok(())
}

// ── Wayback Machine ──────────────────────────────────────────────────────

async fn wayback(domain: &str, limit: usize) -> Result<(), String> {
    let domain = clean_domain(domain);
    println!(
        "  {} {} (limit: {})",
        "Wayback Machine:".dimmed(),
        domain.bold(),
        limit
    );
    println!();

    let urls = crate::osint::wayback_urls(&domain, limit).await;

    if urls.is_empty() {
        println!("  {}", "No archived URLs found.".dimmed());
    } else {
        for (i, url) in urls.iter().enumerate() {
            println!("  {:>4}  {}", (i + 1).to_string().dimmed(), url);
        }
        println!();
        println!(
            "  {} historical URLs found",
            urls.len().to_string().green().bold()
        );
    }

    println!();
    Ok(())
}

async fn run_dig(domain: &str, record_type: &str) -> Vec<String> {
    let output = Command::new("dig")
        .args(["+short", record_type, domain])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.trim_end_matches('.').to_string())
                .collect()
        }
        _ => Vec::new(),
    }
}
