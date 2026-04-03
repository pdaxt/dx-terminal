use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

mod book;
mod media;
mod torrent;
mod recon;
mod seo;
mod osint;
mod username;
mod spawn;
mod whois;
mod scan;
mod http;
mod hash;
mod display;
mod find;

#[derive(Parser)]
#[command(
    name = "dx",
    about = "Lightning-fast books, media, torrent & recon tools",
    version,
    after_help = "Examples:\n  dx book search \"Clean Code\"\n  dx media audio https://youtube.com/watch?v=dQw4w9WgXcQ\n  dx torrent search \"ubuntu 24.04\"\n  dx find \"download video\"     # fuzzy search all commands\n  dx completions zsh           # shell tab-completion"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search, download, and manage books
    #[command(alias = "b")]
    Book {
        #[command(subcommand)]
        action: book::BookAction,
    },
    /// Search and download videos/audio from YouTube, SoundCloud, etc.
    #[command(alias = "m")]
    Media {
        #[command(subcommand)]
        action: media::MediaAction,
    },
    /// Search and download torrents
    #[command(alias = "t")]
    Torrent {
        #[command(subcommand)]
        action: torrent::TorrentAction,
    },
    /// Recon: domain intel, email verify, person finder, tech stack, DNS
    #[command(alias = "r")]
    Recon {
        #[command(subcommand)]
        action: recon::ReconAction,
    },
    /// SEO: keywords, SERP analysis, questions, competitor comparison
    #[command(alias = "s")]
    Seo {
        #[command(subcommand)]
        action: seo::SeoAction,
    },
    /// Autonomous economic engine: OBSERVE → BUILD → SELL → LEARN
    #[command(alias = "x")]
    Spawn {
        #[command(subcommand)]
        action: spawn::SpawnAction,
    },
    /// WHOIS lookup for domains and IPs
    #[command(alias = "w")]
    Whois {
        #[command(subcommand)]
        action: whois::WhoisAction,
    },
    /// Port scanning and SSL certificate analysis
    Scan {
        #[command(subcommand)]
        action: scan::ScanAction,
    },
    /// HTTP client for API testing (like curl but pretty)
    Http {
        #[command(subcommand)]
        action: http::HttpAction,
    },
    /// Hashing, encoding, decoding, password generation
    Hash {
        #[command(subcommand)]
        action: hash::HashAction,
    },
    /// Hunt usernames across 479 social networks (Sherlock-powered)
    #[command(alias = "u")]
    Username {
        #[command(subcommand)]
        action: username::UsernameAction,
    },
    /// Fuzzy-search all dx commands by keyword
    #[command(alias = "f", alias = "?")]
    Find {
        /// Search terms (e.g. "download video", "search book", "torrent")
        query: Vec<String>,
    },
    /// Generate shell completions (zsh, bash, fish)
    Completions {
        /// Shell type
        shell: clap_complete::Shell,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Book { action } => book::run(action).await,
        Commands::Media { action } => media::run(action).await,
        Commands::Torrent { action } => torrent::run(action).await,
        Commands::Recon { action } => recon::run(action).await,
        Commands::Seo { action } => seo::run(action).await,
        Commands::Spawn { action } => spawn::run(action).await,
        Commands::Whois { action } => whois::run(action).await,
        Commands::Scan { action } => scan::run(action).await,
        Commands::Http { action } => http::run(action).await,
        Commands::Hash { action } => hash::run(action).await,
        Commands::Username { action } => username::run(action).await,
        Commands::Find { query } => {
            find::run(&query.join(" "));
            Ok(())
        }
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "dx",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}
