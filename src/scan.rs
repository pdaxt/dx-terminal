use clap::Subcommand;
use colored::Colorize;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Command;

use crate::display;

const COMMON_PORTS: &[(u16, &str)] = &[
    (21, "FTP"), (22, "SSH"), (23, "Telnet"), (25, "SMTP"), (53, "DNS"),
    (80, "HTTP"), (81, "HTTP-Alt"), (110, "POP3"), (111, "RPCBind"), (135, "MSRPC"),
    (139, "NetBIOS"), (143, "IMAP"), (443, "HTTPS"), (445, "SMB"), (465, "SMTPS"),
    (587, "Submission"), (993, "IMAPS"), (995, "POP3S"), (1433, "MSSQL"),
    (1521, "Oracle"), (2049, "NFS"), (2082, "cPanel"), (2083, "cPanelSSL"),
    (2086, "WHM"), (2087, "WHMSSL"), (3000, "Dev/Grafana"), (3001, "Dev"),
    (3306, "MySQL"), (3389, "RDP"), (4443, "HTTPS-Alt"), (5432, "PostgreSQL"),
    (5900, "VNC"), (6379, "Redis"), (6443, "K8s-API"), (8000, "HTTP-Alt"),
    (8080, "HTTP-Proxy"), (8443, "HTTPS-Alt"), (8888, "HTTP-Alt"),
    (9090, "Prometheus"), (9200, "Elasticsearch"), (9443, "HTTPS-Alt"),
    (27017, "MongoDB"), (27018, "MongoDB-Shard"), (50000, "SAP"),
];

#[derive(Subcommand)]
pub enum ScanAction {
    /// Scan open TCP ports on a host
    #[command(alias = "p")]
    Ports {
        /// Target host (domain or IP)
        host: String,
        /// Scan all 65535 ports (default: top 22 common ports)
        #[arg(long)]
        all: bool,
        /// Max concurrent connections (default: 200)
        #[arg(short = 'j', long, default_value = "200")]
        jobs: usize,
    },
    /// Check SSL/TLS certificate and configuration
    #[command(alias = "s")]
    Ssl {
        /// Target host (domain)
        host: String,
        /// Port (default: 443)
        #[arg(short, long, default_value = "443")]
        port: u16,
    },
}

pub async fn run(action: ScanAction) -> Result<(), String> {
    match action {
        ScanAction::Ports { host, all, jobs } => port_scan(&host, all, jobs).await,
        ScanAction::Ssl { host, port } => ssl_scan(&host, port).await,
    }
}

async fn port_scan(host: &str, all: bool, max_concurrent: usize) -> Result<(), String> {
    let host = host.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');

    let ports: Vec<(u16, String)> = if all {
        (1..=65535).map(|p| {
            let name = COMMON_PORTS.iter().find(|(port, _)| *port == p).map(|(_, n)| n.to_string()).unwrap_or_default();
            (p, name)
        }).collect()
    } else {
        COMMON_PORTS.iter().map(|(p, n)| (*p, n.to_string())).collect()
    };

    let total = ports.len();
    println!("  {} {} ({} ports, {} concurrent)", "Scanning:".dimmed(), host.bold(), total, max_concurrent);
    println!();

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let host_owned = host.to_string();
    let mut handles = Vec::new();

    for (port, service) in ports {
        let sem = sem.clone();
        let h = host_owned.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let addr = format!("{}:{}", h, port);
            match tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await {
                Ok(Ok(mut stream)) => {
                    // Try banner grab (read first bytes with short timeout)
                    let mut buf = vec![0u8; 256];
                    let banner = match tokio::time::timeout(
                        Duration::from_millis(500),
                        tokio::io::AsyncReadExt::read(&mut stream, &mut buf),
                    ).await {
                        Ok(Ok(n)) if n > 0 => {
                            let raw = String::from_utf8_lossy(&buf[..n]);
                            raw.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(60).collect::<String>()
                        }
                        _ => String::new(),
                    };
                    (port, service, true, banner)
                }
                _ => (port, service, false, String::new()),
            }
        }));
    }

    let mut open_ports = Vec::new();
    for handle in handles {
        if let Ok((port, service, open, banner)) = handle.await {
            if open {
                open_ports.push((port, service, banner));
            }
        }
    }

    open_ports.sort_by_key(|(p, _, _)| *p);

    if open_ports.is_empty() {
        println!("  {} No open ports found on {}", "−".dimmed(), host);
    } else {
        let rows: Vec<Vec<String>> = open_ports.iter().map(|(port, service, banner)| {
            vec![
                port.to_string().green().to_string(),
                if service.is_empty() { "unknown".dimmed().to_string() } else { service.clone() },
                "OPEN".green().bold().to_string(),
                if banner.is_empty() { String::new() } else { display::truncate(banner, 40).dimmed().to_string() },
            ]
        }).collect();
        display::table(&["Port", "Service", "Status", "Banner"], &rows);
        println!("  {} open ports on {}", open_ports.len().to_string().green().bold(), host);
    }

    println!();
    Ok(())
}

async fn ssl_scan(host: &str, port: u16) -> Result<(), String> {
    let host = host.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');

    println!("  {} {}:{}", "SSL scan:".dimmed(), host.bold(), port);
    println!();

    // Use openssl s_client to get cert info
    let output = Command::new("openssl")
        .args(["s_client", "-connect", &format!("{}:{}", host, port), "-servername", host])
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("openssl not found: {}. Install: brew install openssl", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    // Parse certificate details
    let subject = extract_field(&combined, "subject=").or_else(|| extract_field(&combined, "subject ="));
    let issuer = extract_field(&combined, "issuer=").or_else(|| extract_field(&combined, "issuer ="));
    let protocol = extract_field(&combined, "Protocol  :");
    let cipher = extract_field(&combined, "Cipher    :");

    // Get cert dates via openssl x509
    let cert_text = Command::new("bash")
        .args(["-c", &format!(
            "echo | openssl s_client -connect {}:{} -servername {} 2>/dev/null | openssl x509 -noout -dates -subject -issuer -ext subjectAltName 2>/dev/null",
            host, port, host
        )])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let not_before = extract_field(&cert_text, "notBefore=");
    let not_after = extract_field(&cert_text, "notAfter=");

    // Calculate days until expiry
    let days_left = not_after.as_ref().and_then(|date_str| {
        chrono::NaiveDateTime::parse_from_str(date_str.trim(), "%b %d %H:%M:%S %Y GMT")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str.trim(), "%b  %d %H:%M:%S %Y GMT"))
            .ok()
            .map(|expiry| {
                let now = chrono::Utc::now().naive_utc();
                (expiry - now).num_days()
            })
    });

    // Extract SANs
    let sans: Vec<String> = cert_text.lines()
        .filter(|l| l.contains("DNS:"))
        .flat_map(|l| l.split(','))
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.starts_with("DNS:") {
                Some(trimmed[4..].to_string())
            } else {
                None
            }
        })
        .collect();

    // Display
    if let Some(ref s) = subject {
        display::kv("Subject", s);
    }
    if let Some(ref i) = issuer {
        display::kv("Issuer", i);
    }
    if let Some(ref p) = protocol {
        display::kv("Protocol", p);
    }
    if let Some(ref c) = cipher {
        display::kv("Cipher", c);
    }
    if let Some(ref nb) = not_before {
        display::kv("Valid from", nb);
    }
    if let Some(ref na) = not_after {
        let expiry_display = match days_left {
            Some(d) if d < 0 => format!("{} ({})", na, "EXPIRED".red().bold()),
            Some(d) if d < 30 => format!("{} ({} days — {})", na, d, "EXPIRING SOON".yellow().bold()),
            Some(d) => format!("{} ({} days)", na, d.to_string().green()),
            None => na.clone(),
        };
        display::kv("Valid until", &expiry_display);
    }
    if !sans.is_empty() {
        display::kv("SANs", &sans.iter().take(10).cloned().collect::<Vec<_>>().join(", "));
        if sans.len() > 10 {
            display::kv("", &format!("... and {} more", sans.len() - 10).dimmed().to_string());
        }
    }

    // Verdict
    println!();
    match days_left {
        Some(d) if d < 0 => display::error("Certificate has EXPIRED"),
        Some(d) if d < 30 => println!("  {} Certificate expires in {} days", "!".yellow().bold(), d),
        Some(d) => display::status(&format!("Certificate valid for {} days", d), "✓"),
        None => println!("  {} Could not determine expiry", "?".yellow()),
    }

    println!();
    Ok(())
}

fn extract_field(text: &str, prefix: &str) -> Option<String> {
    text.lines()
        .find(|l| l.trim().starts_with(prefix) || l.contains(prefix))
        .map(|l| {
            l.split_once(prefix)
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_else(|| l.trim().to_string())
        })
}
