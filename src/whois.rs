use clap::Subcommand;
use colored::Colorize;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::display;

#[derive(Subcommand)]
pub enum WhoisAction {
    /// Look up WHOIS data for a domain or IP
    #[command(alias = "l")]
    Lookup {
        /// Domain or IP address
        target: String,
    },
}

pub async fn run(action: WhoisAction) -> Result<(), String> {
    match action {
        WhoisAction::Lookup { target } => lookup(&target).await,
    }
}

async fn whois_query(server: &str, query: &str) -> Result<String, String> {
    let addr = format!("{}:43", server);
    let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("timeout connecting to {}", server))?
        .map_err(|e| format!("connect {}: {}", server, e))?;

    let q = format!("{}\r\n", query);
    stream.write_all(q.as_bytes()).await.map_err(|e| format!("write: {}", e))?;

    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut buf))
        .await
        .map_err(|_| "read timeout".to_string())?
        .map_err(|e| format!("read: {}", e))?;

    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn is_ip(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':')
}

async fn lookup(target: &str) -> Result<(), String> {
    let target = target.trim().to_lowercase();
    println!("  {} {}", "WHOIS:".dimmed(), target.bold());
    println!();

    let (server, query) = if is_ip(&target) {
        ("whois.arin.net", format!("n + {}", target))
    } else {
        ("whois.iana.org", target.clone())
    };

    // First query to find referral server
    let initial = whois_query(server, &query).await?;

    // For domains, find the referral whois server
    let referral = if !is_ip(&target) {
        initial.lines()
            .find(|l| l.to_lowercase().starts_with("refer:") || l.to_lowercase().starts_with("whois:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    // Query the authoritative server if we found one
    let full_response = if let Some(ref auth_server) = referral {
        match whois_query(auth_server, &target).await {
            Ok(r) => r,
            Err(_) => initial.clone(),
        }
    } else {
        initial.clone()
    };

    // Parse and display key fields
    let fields: Vec<(&str, Vec<&str>)> = vec![
        ("Domain Name", vec!["domain name:"]),
        ("Registrar", vec!["registrar:", "registrar name:"]),
        ("Created", vec!["creation date:", "created:", "created on:", "registration date:"]),
        ("Updated", vec!["updated date:", "last updated:", "changed:"]),
        ("Expires", vec!["registry expiry date:", "expiration date:", "expires:", "expire date:"]),
        ("Status", vec!["domain status:", "status:"]),
        ("Nameservers", vec!["name server:", "nserver:"]),
        ("DNSSEC", vec!["dnssec:"]),
        ("NetRange", vec!["netrange:"]),
        ("NetName", vec!["netname:"]),
        ("Organization", vec!["organization:", "org-name:", "orgname:"]),
        ("Country", vec!["country:"]),
        ("CIDR", vec!["cidr:"]),
    ];

    let mut found_any = false;
    for (label, prefixes) in &fields {
        let mut values: Vec<String> = Vec::new();
        for line in full_response.lines() {
            let lower = line.to_lowercase();
            let trimmed = lower.trim();
            for prefix in prefixes {
                if trimmed.starts_with(prefix) {
                    let val = line[prefix.len()..].trim().to_string();
                    // For original case, find the value part
                    let orig_val = line.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or(val);
                    if !orig_val.is_empty() && !values.contains(&orig_val) {
                        values.push(orig_val);
                    }
                }
            }
        }
        if !values.is_empty() {
            found_any = true;
            if values.len() == 1 {
                display::kv(label, &values[0]);
            } else if *label == "Nameservers" || *label == "Status" {
                display::kv(label, &values.join("\n                "));
            } else {
                display::kv(label, &values.join(", "));
            }
        }
    }

    if !found_any {
        // Show raw response if we couldn't parse fields
        println!("  {}", "Raw WHOIS response:".dimmed());
        for line in full_response.lines().take(30) {
            if !line.trim().is_empty() && !line.starts_with('%') && !line.starts_with('#') {
                println!("    {}", line);
            }
        }
    }

    if let Some(ref srv) = referral {
        println!();
        display::kv("Server", srv);
    }

    println!();
    Ok(())
}
