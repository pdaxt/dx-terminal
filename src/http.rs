use clap::Subcommand;
use colored::Colorize;
use std::time::Instant;

use crate::display;

#[derive(Subcommand)]
pub enum HttpAction {
    /// Send a GET request
    #[command(name = "GET", alias = "get")]
    Get {
        /// URL to request
        url: String,
        /// Custom headers (-H "Key: Value", repeatable)
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        /// Timeout in seconds (default: 30)
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    /// Send a POST request
    #[command(name = "POST", alias = "post")]
    Post {
        /// URL to request
        url: String,
        /// Request body (JSON)
        #[arg(short, long)]
        data: Option<String>,
        /// Custom headers
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        /// Timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    /// Send a PUT request
    #[command(name = "PUT", alias = "put")]
    Put {
        url: String,
        #[arg(short, long)]
        data: Option<String>,
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    /// Send a DELETE request
    #[command(name = "DELETE", alias = "delete")]
    Delete {
        url: String,
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    /// Send a HEAD request (headers only)
    #[command(name = "HEAD", alias = "head")]
    Head {
        url: String,
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
}

pub async fn run(action: HttpAction) -> Result<(), String> {
    let (method, url, data, custom_headers, timeout) = match action {
        HttpAction::Get { url, headers, timeout } => ("GET", url, None, headers, timeout),
        HttpAction::Post { url, data, headers, timeout } => ("POST", url, data, headers, timeout),
        HttpAction::Put { url, data, headers, timeout } => ("PUT", url, data, headers, timeout),
        HttpAction::Delete { url, headers, timeout } => ("DELETE", url, None, headers, timeout),
        HttpAction::Head { url, headers, timeout } => ("HEAD", url, None, headers, timeout),
    };

    send_request(method, &url, data.as_deref(), &custom_headers, timeout).await
}

async fn send_request(
    method: &str,
    url: &str,
    data: Option<&str>,
    custom_headers: &[String],
    timeout_secs: u64,
) -> Result<(), String> {
    // Ensure URL has scheme
    let url = if !url.starts_with("http") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    let method_colored = match method {
        "GET" => method.green().bold(),
        "POST" => method.yellow().bold(),
        "PUT" => method.blue().bold(),
        "DELETE" => method.red().bold(),
        "HEAD" => method.cyan().bold(),
        _ => method.white().bold(),
    };
    println!("  {} {}", method_colored, url.dimmed());
    println!();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .user_agent("dx-http/1.0")
        .build()
        .map_err(|e| format!("client error: {}", e))?;

    let mut req = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "HEAD" => client.head(&url),
        _ => return Err(format!("Unknown method: {}", method)),
    };

    // Add custom headers
    for h in custom_headers {
        if let Some((key, val)) = h.split_once(':') {
            req = req.header(key.trim(), val.trim());
        }
    }

    // Add body for POST/PUT
    if let Some(body) = data {
        req = req.header("Content-Type", "application/json").body(body.to_string());
    }

    let start = Instant::now();
    let resp = req.send().await.map_err(|e| format!("request failed: {}", e))?;
    let elapsed = start.elapsed();

    let status = resp.status();
    let status_colored = if status.is_success() {
        format!("{}", status).green().bold()
    } else if status.is_redirection() {
        format!("{}", status).yellow().bold()
    } else {
        format!("{}", status).red().bold()
    };

    // Status line
    println!("  {} {} ({}ms)", "Status:".dimmed(), status_colored, elapsed.as_millis());
    println!();

    // Headers
    println!("  {}", "Headers".bold());
    let important_headers = [
        "content-type", "content-length", "server", "date", "cache-control",
        "x-request-id", "x-ratelimit-remaining", "location", "set-cookie",
        "access-control-allow-origin", "strict-transport-security",
    ];
    for (name, value) in resp.headers() {
        let name_str = name.as_str();
        if important_headers.contains(&name_str) || custom_headers.iter().any(|h| h.to_lowercase().starts_with(name_str)) {
            if let Ok(val) = value.to_str() {
                println!("    {}: {}", name_str.cyan(), display::truncate(val, 80));
            }
        }
    }

    // Body (skip for HEAD)
    if method != "HEAD" {
        let body = resp.text().await.unwrap_or_default();

        if !body.is_empty() {
            println!();
            // Try to pretty-print JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                println!("  {}", "Body (JSON)".bold());
                let pretty = serde_json::to_string_pretty(&json).unwrap_or(body.clone());
                // Colorize JSON output (first 50 lines)
                for (i, line) in pretty.lines().take(50).enumerate() {
                    let colored_line = line
                        .replace('"', &"\"".dimmed().to_string());
                    println!("    {}", colored_line);
                }
                if pretty.lines().count() > 50 {
                    println!("    {} ({} more lines)", "...".dimmed(), pretty.lines().count() - 50);
                }
            } else {
                // Non-JSON body
                let preview = if body.len() > 500 {
                    format!("{}... ({} bytes total)", &body[..500], body.len())
                } else {
                    body.clone()
                };
                println!("  {}", "Body".bold());
                for line in preview.lines().take(20) {
                    println!("    {}", line);
                }
            }
        }
    }

    println!();
    Ok(())
}
