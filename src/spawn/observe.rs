use colored::Colorize;
use serde::{Deserialize, Serialize};

use super::scrape;
use super::state::{Phase, SpawnState};
use crate::display;

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketResearch {
    pub niche: String,
    pub keywords: Vec<String>,
    pub questions: Vec<String>,
    pub reddit_posts: Vec<scrape::RedditPost>,
    pub hn_posts: Vec<scrape::HnPost>,
    pub pain_points: Vec<String>,
    pub competitors: Vec<scrape::CompetitorProfile>,
    pub serp_results: Vec<scrape::SerpResult>,
    pub timestamp: String,
}

pub async fn run(niche: Option<Vec<String>>, resume: Option<String>) -> Result<(), String> {
    let mut state = if let Some(slug) = resume {
        SpawnState::load(&slug)?
    } else {
        let niche_str = niche
            .map(|v| v.join(" "))
            .ok_or("Niche required. Usage: dx spawn observe \"your niche\"")?;
        let mut s = SpawnState::new(&niche_str);
        s.save()?;
        s
    };
    execute(&mut state).await
}

pub async fn execute(state: &mut SpawnState) -> Result<(), String> {
    state.transition(Phase::Observing)?;
    state.save()?;

    let niche = state.niche.clone();
    display::header(&format!("OBSERVE: {}", niche));

    // ── 1. Keywords (Google Suggest, expanded a-z + modifiers) ───────────
    println!("  {} Keyword research (a-z expansion)...", "→".cyan());
    let keywords = scrape::google_suggest_expanded(&niche).await;
    let kw_count = keywords.len();
    state.log("observe", "keywords", true, &format!("{} keywords", kw_count));
    println!("    {} keywords found", kw_count.to_string().green());

    // ── 2. Questions ─────────────────────────────────────────────────────
    let questions: Vec<String> = keywords
        .iter()
        .filter(|k| {
            let l = k.to_lowercase();
            l.starts_with("how ") || l.starts_with("what ") || l.starts_with("why ")
                || l.starts_with("when ") || l.starts_with("who ")
                || l.starts_with("can ") || l.starts_with("is ")
                || l.starts_with("does ") || l.starts_with("which ")
                || l.contains('?')
        })
        .cloned()
        .collect();
    println!("    {} questions extracted", questions.len().to_string().green());

    // ── 3. Reddit (real posts, real pain points) ─────────────────────────
    println!("  {} Scraping Reddit...", "→".cyan());
    let reddit_posts = scrape::reddit_search(&niche, 25).await;
    let pain_points = scrape::extract_pain_points(&reddit_posts);
    state.log("observe", "reddit", true, &format!("{} posts, {} pain points", reddit_posts.len(), pain_points.len()));
    println!(
        "    {} posts, {} pain points",
        reddit_posts.len().to_string().green(),
        pain_points.len().to_string().yellow()
    );

    // Show top pain points
    for (i, pain) in pain_points.iter().take(5).enumerate() {
        println!("      {}. {}", i + 1, display::truncate(pain, 70).red());
    }

    // ── 4. Hacker News ──────────────────────────────────────────────────
    println!("  {} Scraping Hacker News...", "→".cyan());
    let hn_posts = scrape::hn_search(&niche, 20).await;
    state.log("observe", "hackernews", true, &format!("{} posts", hn_posts.len()));
    println!("    {} posts found", hn_posts.len().to_string().green());

    // ── 5. SERP + Competitor deep scrape ─────────────────────────────────
    println!("  {} Google SERP analysis...", "→".cyan());
    let serp = scrape::google_search(&niche, 10).await;
    state.log("observe", "serp", true, &format!("{} results", serp.len()));
    println!("    {} SERP results", serp.len().to_string().green());

    println!("  {} Deep-scraping competitors...", "→".cyan());
    let skip_domains = [
        "wikipedia.org", "youtube.com", "reddit.com", "medium.com",
        "linkedin.com", "twitter.com", "facebook.com", "g2.com",
        "capterra.com", "quora.com", "amazon.com", "forbes.com",
    ];

    let mut competitors: Vec<scrape::CompetitorProfile> = Vec::new();
    for result in &serp {
        if skip_domains.iter().any(|s| result.domain.contains(s)) {
            continue;
        }
        if competitors.len() >= 8 {
            break;
        }
        print!("    {} {}... ", "→".dimmed(), result.domain);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match scrape::scrape_competitor(&result.domain).await {
            Some(mut profile) => {
                // Try to get pricing
                if profile.has_pricing {
                    if let Some(prices) = scrape::scrape_pricing(&result.domain).await {
                        profile.description = format!("{} | Pricing: {}", profile.description, prices);
                    }
                }
                println!(
                    "{} ({}{}{})",
                    "✓".green(),
                    if profile.has_pricing { "pricing " } else { "" },
                    if profile.has_free_trial { "free-trial " } else { "" },
                    profile.tech_stack.join(", ")
                );
                competitors.push(profile);
            }
            None => println!("{}", "✗".red()),
        }
    }
    state.log("observe", "competitors", true, &format!("{} scraped", competitors.len()));

    // ── 6. Prospect generation (from competitors + crt.sh) ───────────────
    println!("  {} Generating prospects...", "→".cyan());
    let mut prospects = Vec::new();

    for comp in &competitors {
        let domain = &comp.domain;
        // Generate email patterns for each competitor (they might be partners/targets)
        for pattern in ["info", "hello", "contact", "sales", "team", "ceo", "founder"] {
            prospects.push(Prospect {
                company: comp.title.clone(),
                domain: domain.clone(),
                email: format!("{}@{}", pattern, domain),
                source: "competitor".into(),
                context: format!("Uses: {}. {}", comp.tech_stack.join(", "),
                    if comp.has_pricing { "Has pricing page" } else { "No public pricing" }),
            });
        }
    }

    // Also find companies from Reddit discussions
    for post in reddit_posts.iter().take(10) {
        let words: Vec<&str> = post.title.split_whitespace().collect();
        if words.len() >= 2 {
            // Look for mentioned products/companies in titles
            for word in &words {
                if word.len() > 3 && word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    let domain_guess = format!("{}.com", word.to_lowercase());
                    if !prospects.iter().any(|p: &Prospect| p.domain == domain_guess) {
                        prospects.push(Prospect {
                            company: word.to_string(),
                            domain: domain_guess.clone(),
                            email: format!("info@{}", domain_guess),
                            source: "reddit-mention".into(),
                            context: format!("Mentioned in: r/{} ({}pts)", post.subreddit, post.score),
                        });
                    }
                }
            }
        }
    }

    state.log("observe", "prospects", true, &format!("{} prospects", prospects.len()));
    println!("    {} prospects generated", prospects.len().to_string().green());

    // ── Save everything ──────────────────────────────────────────────────
    let dir = state.dir();

    let research = MarketResearch {
        niche: niche.clone(),
        keywords,
        questions,
        reddit_posts,
        hn_posts,
        pain_points,
        competitors,
        serp_results: serp,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    std::fs::write(
        dir.join("research/market.json"),
        serde_json::to_string_pretty(&research).unwrap_or_default(),
    )
    .map_err(|e| format!("write market.json: {}", e))?;
    state.market_research = Some("research/market.json".into());

    // Prospects CSV
    let mut csv = String::from("company,domain,email,source,context\n");
    for p in &prospects {
        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
            p.company.replace('"', "\"\""),
            p.domain,
            p.email,
            p.source,
            p.context.replace('"', "\"\""),
        ));
    }
    std::fs::write(dir.join("research/prospects.csv"), &csv)
        .map_err(|e| format!("write prospects.csv: {}", e))?;
    state.prospects = Some("research/prospects.csv".into());

    // ── Transition ───────────────────────────────────────────────────────
    state.transition(Phase::Observed)?;
    state.save()?;

    // ── Summary ──────────────────────────────────────────────────────────
    println!();
    display::status("OBSERVE complete", "✓");
    display::kv("Keywords", &research.keywords.len().to_string());
    display::kv("Questions", &research.questions.len().to_string());
    display::kv("Reddit posts", &research.reddit_posts.len().to_string());
    display::kv("HN posts", &research.hn_posts.len().to_string());
    display::kv("Pain points", &research.pain_points.len().to_string());
    display::kv("Competitors scraped", &research.competitors.len().to_string());
    display::kv("Prospects", &prospects.len().to_string());
    display::kv("Output", &dir.join("research").to_string_lossy());
    println!();
    println!("  Next: {}", "dx spawn build".cyan().bold());
    println!();

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Prospect {
    company: String,
    domain: String,
    email: String,
    source: String,
    context: String,
}
