use colored::Colorize;

/// All dx commands with descriptions and tags for fuzzy matching.
const COMMANDS: &[(&str, &str, &[&str])] = &[
    // Books
    ("dx book search <query>", "Search for books by title/author/ISBN", &["book", "search", "find", "library", "libgen", "pdf", "epub"]),
    ("dx book download <title>", "Download a book (auto-search + download)", &["book", "download", "get", "fetch", "pdf"]),
    ("dx book download --url <URL>", "Download book from direct URL", &["book", "download", "url", "direct"]),
    ("dx book list", "List all downloaded books", &["book", "list", "library", "collection", "ls"]),
    ("dx book process <file>", "Extract text + metadata from PDF/EPUB", &["book", "process", "extract", "text", "metadata", "ocr"]),
    ("dx book read <slug> <query>", "Full-text search within a book", &["book", "read", "search", "within", "find", "grep"]),
    ("dx book delete <title>", "Delete a book from library", &["book", "delete", "remove", "rm"]),

    // Media
    ("dx media search <query>", "Search YouTube/SoundCloud for videos/music", &["media", "search", "youtube", "soundcloud", "music", "video", "find"]),
    ("dx media video <URL>", "Download a video (best/720p/480p)", &["media", "video", "download", "youtube", "mp4"]),
    ("dx media audio <URL>", "Extract audio as mp3/flac/wav", &["media", "audio", "music", "mp3", "download", "extract", "song"]),
    ("dx media playlist <URL>", "Download entire playlist", &["media", "playlist", "bulk", "album", "download"]),
    ("dx media info <URL>", "Get media metadata without downloading", &["media", "info", "metadata", "preview"]),
    ("dx media list", "List all downloaded media files", &["media", "list", "library", "files", "ls"]),
    ("dx media status", "Check yt-dlp/aria2c availability", &["media", "status", "tools", "check"]),

    // Torrents
    ("dx torrent search <query>", "Search 1337x/PirateBay/TorrentGalaxy", &["torrent", "search", "piratebay", "1337x", "magnet", "find"]),
    ("dx torrent download <magnet>", "Download torrent via aria2c/transmission", &["torrent", "download", "magnet", "aria2c", "get"]),
    ("dx torrent status", "Check torrent client availability", &["torrent", "status", "tools", "aria2c", "transmission"]),

    // Recon
    ("dx recon domain <domain>", "Full domain intel: DNS, tech, meta, social", &["recon", "domain", "investigate", "lookup", "intel", "whois"]),
    ("dx recon email <email>", "Verify email: MX, SMTP, disposable check", &["recon", "email", "verify", "validate", "check"]),
    ("dx recon person <first> <last> <domain>", "Find probable email addresses for a person", &["recon", "person", "email", "find", "contact", "lookup"]),
    ("dx recon tech <domain>", "Detect tech stack from website", &["recon", "tech", "stack", "technology", "detect", "wappalyzer"]),
    ("dx recon hiring <domain>", "Check if company is actively hiring", &["recon", "hiring", "careers", "jobs", "recruiting"]),
    ("dx recon dns <domain>", "DNS records: A, AAAA, MX, NS, TXT, CNAME", &["recon", "dns", "records", "mx", "nameserver", "dig"]),
    ("dx recon subdomains <domain>", "Find subdomains via crt.sh, RapidDNS, HackerTarget", &["recon", "subdomains", "subdomain", "crt", "rapiddns", "enumerate"]),
    ("dx recon wayback <domain>", "Historical URLs from Wayback Machine", &["recon", "wayback", "archive", "historical", "urls", "web"]),

    // Username
    ("dx username check <username>", "Check username across 479 social networks", &["username", "sherlock", "social", "osint", "check", "hunt", "profile"]),

    // SEO
    ("dx seo keywords <keyword>", "Google Autocomplete keyword suggestions", &["seo", "keywords", "suggest", "autocomplete", "google", "ideas"]),
    ("dx seo serp <query>", "Analyze Google SERP: who ranks, titles", &["seo", "serp", "google", "ranking", "search", "results"]),
    ("dx seo domain <domain>", "Quick domain SEO overview", &["seo", "domain", "overview", "audit", "site"]),
    ("dx seo questions <topic>", "People Also Ask — related questions", &["seo", "questions", "paa", "faq", "people", "ask"]),
    ("dx seo compare <you> <them>", "Compare vs competitor in Google Suggest", &["seo", "compare", "competitor", "vs", "analysis"]),
    ("dx seo gap <you> <them> <topic>", "Find content gaps vs competitors", &["seo", "gap", "content", "missing", "competitor", "topics"]),

    // Spawn
    ("dx spawn run <niche>", "Full OBSERVE → BUILD → SELL → LEARN pipeline", &["spawn", "run", "autonomous", "pipeline", "niche", "business"]),
    ("dx spawn observe <niche>", "Phase 1: Research market, find prospects, analyze competitors", &["spawn", "observe", "research", "market", "competitors", "prospects"]),
    ("dx spawn build", "Phase 2: Generate spec, build landing page, create content", &["spawn", "build", "landing", "page", "spec", "content"]),
    ("dx spawn sell", "Phase 3: Draft cold emails and social posts (dry-run)", &["spawn", "sell", "email", "outreach", "cold", "social"]),
    ("dx spawn learn", "Phase 4: Collect metrics, score effectiveness", &["spawn", "learn", "metrics", "score", "iterate"]),
    ("dx spawn list", "List all spawns", &["spawn", "list", "ls", "all"]),
    ("dx spawn status <slug>", "Show detailed status of a spawn", &["spawn", "status", "detail", "progress"]),
    ("dx spawn kill <slug>", "Kill/archive a spawn", &["spawn", "kill", "archive", "delete", "remove"]),

    // WHOIS
    ("dx whois lookup <domain>", "WHOIS data: registrar, dates, nameservers, status", &["whois", "domain", "registrar", "expiry", "lookup"]),

    // Scan
    ("dx scan ports <host>", "TCP port scan with banner grabbing (44 common ports)", &["scan", "port", "ports", "nmap", "open", "service", "banner"]),
    ("dx scan ports <host> --all", "Full 65535 port scan", &["scan", "port", "all", "full", "65535"]),
    ("dx scan ssl <host>", "SSL cert: expiry, issuer, SANs, cipher, protocol", &["scan", "ssl", "tls", "cert", "certificate", "expiry", "https"]),

    // HTTP
    ("dx http GET <url>", "HTTP GET with pretty JSON output", &["http", "get", "api", "curl", "request", "json"]),
    ("dx http POST <url> -d '{}'", "HTTP POST with JSON body", &["http", "post", "api", "json", "send", "data"]),
    ("dx http PUT <url> -d '{}'", "HTTP PUT request", &["http", "put", "update", "api"]),
    ("dx http DELETE <url>", "HTTP DELETE request", &["http", "delete", "remove", "api"]),
    ("dx http HEAD <url>", "HTTP HEAD (headers only)", &["http", "head", "headers", "check"]),

    // Hash
    ("dx hash md5 <text>", "MD5 hash of text or file", &["hash", "md5", "checksum"]),
    ("dx hash sha256 <text>", "SHA-256 hash of text or file", &["hash", "sha256", "sha", "checksum"]),
    ("dx hash sha512 <text>", "SHA-512 hash of text or file", &["hash", "sha512", "checksum"]),
    ("dx hash encode base64 <text>", "Base64/hex/URL encode", &["hash", "encode", "base64", "hex", "url"]),
    ("dx hash decode base64 <text>", "Base64/hex decode", &["hash", "decode", "base64", "hex"]),
    ("dx hash password", "Generate secure random password", &["hash", "password", "generate", "random", "secure"]),

    // Discovery
    ("dx find <query>", "Fuzzy-search all dx commands", &["find", "help", "search", "discover", "what"]),
    ("dx completions zsh", "Generate zsh tab completions", &["completions", "zsh", "tab", "shell"]),
];

pub fn run(query: &str) {
    if query.is_empty() {
        // Show all commands grouped
        println!();
        println!("  {}", "All dx commands:".bold().underline());
        println!();

        let mut current_group = "";
        for (cmd, desc, _tags) in COMMANDS {
            let group = cmd.split_whitespace().nth(1).unwrap_or("");
            if group != current_group {
                current_group = group;
                println!("  {}", format!("── {} ──", group).cyan().bold());
            }
            println!("    {:<38} {}", cmd.green(), desc.dimmed());
        }
        println!();
        println!("  {} {}", "Tip:".yellow(), "dx find <keyword> to search".dimmed());
        println!();
        return;
    }

    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();

    let mut scored: Vec<(usize, &str, &str)> = Vec::new();

    for (cmd, desc, tags) in COMMANDS {
        let mut score = 0usize;
        let haystack = format!("{} {} {}", cmd, desc, tags.join(" ")).to_lowercase();

        for term in &terms {
            if haystack.contains(term) {
                score += 1;
            }
            // Bonus for tag match
            if tags.iter().any(|t| t == term) {
                score += 2;
            }
            // Bonus for command name match
            if cmd.to_lowercase().contains(term) {
                score += 1;
            }
        }

        if score > 0 {
            scored.push((score, cmd, desc));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    println!();
    if scored.is_empty() {
        println!("  {} No commands matching \"{}\"", "✗".red(), query);
        println!("  Run {} to see all commands", "dx find".cyan());
    } else {
        println!(
            "  {} for \"{}\":",
            format!("{} matches", scored.len()).green().bold(),
            query.bold()
        );
        println!();
        for (score, cmd, desc) in &scored {
            let stars = "●".repeat((*score).min(5));
            println!(
                "    {} {:<38} {}",
                stars.yellow(),
                cmd.green(),
                desc.dimmed()
            );
        }
    }
    println!();
}
