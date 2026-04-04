use colored::Colorize;
use serde_json::Value;

use super::state::{Phase, SpawnState};
use crate::display;

pub async fn run(slug: Option<String>) -> Result<(), String> {
    let mut state = SpawnState::or_latest(slug)?;
    execute(&mut state).await
}

pub async fn execute(state: &mut SpawnState) -> Result<(), String> {
    state.transition(Phase::Building)?;
    state.save()?;

    display::header(&format!("BUILD: {}", state.niche));

    let dir = state.dir();

    // ── Load research data ───────────────────────────────────────────────
    let market: Value = load_json(&dir.join("research/market.json"))?;
    let competitors: Vec<Value> = load_json(&dir.join("research/competitors.json"))?;

    let keywords = market.get("keywords").and_then(|k| k.as_array()).cloned().unwrap_or_default();
    let questions = market.get("questions").and_then(|q| q.as_array()).cloned().unwrap_or_default();
    let niche = state.niche.clone();

    // ── Step 1: Generate Product Spec ────────────────────────────────────
    println!("  {} Generating product spec...", "→".cyan());

    let top_keywords: Vec<String> = keywords.iter().take(10)
        .filter_map(|k| k.as_str().map(String::from)).collect();
    let top_questions: Vec<String> = questions.iter().take(10)
        .filter_map(|q| q.as_str().map(String::from)).collect();
    let top_competitors: Vec<String> = competitors.iter().take(5)
        .filter_map(|c| c.get("domain").and_then(|d| d.as_str()).map(String::from)).collect();

    let spec = format!(
        r#"# Product Spec: {niche}

## Problem
People searching for "{niche}" have these pain points:
{questions_list}

## Solution
A focused tool that addresses the top unmet needs in this space.

## Target Audience
Companies and individuals searching for: {keyword_sample}

## Competitive Landscape
Top competitors:
{competitor_list}

## Key Features (Based on Market Gaps)
1. **Simplicity** — Most competitors are overbuilt. Ship the 20% that solves 80%.
2. **Speed** — Faster than every competitor (Rust-native, no bloat)
3. **Price** — Undercut by 50% or freemium with upgrade path

## Pricing Strategy
- Free tier: core feature, limited usage
- Pro: $29/mo — unlimited usage
- Team: $79/mo — collaboration features

## Tech Stack
- Landing page: Static HTML + Tailwind (fast, no framework bloat)
- Backend: Rust + Axum (if needed)
- Database: Neon PostgreSQL (if needed)
- Hosting: Cloudflare Pages (free tier)
- Payments: Stripe

## Go-to-Market
1. SEO content targeting long-tail keywords (5 articles)
2. Cold email to {prospect_count} prospects
3. LinkedIn/Twitter content positioning

## Top Keywords to Target
{keyword_list}

## Content Ideas (from questions people ask)
{content_ideas}
"#,
        niche = niche,
        questions_list = top_questions.iter().enumerate()
            .map(|(i, q)| format!("{}. {}", i + 1, q))
            .collect::<Vec<_>>().join("\n"),
        keyword_sample = top_keywords.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
        competitor_list = top_competitors.iter().enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect::<Vec<_>>().join("\n"),
        prospect_count = std::fs::read_to_string(dir.join("research/prospects.csv"))
            .map(|s| s.lines().count().saturating_sub(1))
            .unwrap_or(0),
        keyword_list = top_keywords.iter().enumerate()
            .map(|(i, k)| format!("{}. {}", i + 1, k))
            .collect::<Vec<_>>().join("\n"),
        content_ideas = top_questions.iter().enumerate()
            .map(|(i, q)| format!("{}. Article: \"{}\"", i + 1, q))
            .collect::<Vec<_>>().join("\n"),
    );

    let spec_path = dir.join("build/spec.md");
    std::fs::write(&spec_path, &spec).map_err(|e| format!("write spec: {}", e))?;
    state.product_spec = Some("build/spec.md".into());
    state.log("build", "spec", true, "product spec generated");
    println!("    {} spec.md generated", "✓".green());

    // ── Step 2: Generate Landing Page ────────────────────────────────────
    println!("  {} Building landing page...", "→".cyan());

    let slug = &state.slug;
    let headline = format!("The Fastest {} Solution", titlecase(&niche));
    let subheadline = top_questions.first()
        .map(|q| q.as_str())
        .unwrap_or("Built for speed. Priced for everyone.");

    let landing_html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{headline}</title>
<meta name="description" content="{subheadline}">
<script src="https://cdn.tailwindcss.com"></script>
</head>
<body class="bg-gray-950 text-white">
<nav class="border-b border-gray-800 px-6 py-4 flex justify-between items-center max-w-5xl mx-auto">
  <span class="text-xl font-bold">{slug}</span>
  <a href="#pricing" class="bg-white text-black px-4 py-2 rounded-lg font-medium hover:bg-gray-200">Get Started</a>
</nav>

<section class="max-w-3xl mx-auto text-center py-24 px-6">
  <h1 class="text-5xl font-bold mb-6 leading-tight">{headline}</h1>
  <p class="text-xl text-gray-400 mb-8">{subheadline}</p>
  <a href="#pricing" class="bg-blue-600 px-8 py-3 rounded-lg text-lg font-medium hover:bg-blue-500">Start Free</a>
</section>

<section class="max-w-4xl mx-auto py-16 px-6 grid md:grid-cols-3 gap-8">
  <div class="bg-gray-900 rounded-xl p-6">
    <h3 class="text-lg font-bold mb-2">Lightning Fast</h3>
    <p class="text-gray-400">Built with Rust. No bloat. Results in milliseconds.</p>
  </div>
  <div class="bg-gray-900 rounded-xl p-6">
    <h3 class="text-lg font-bold mb-2">Simple Pricing</h3>
    <p class="text-gray-400">Free tier included. Pro at $29/mo. No surprises.</p>
  </div>
  <div class="bg-gray-900 rounded-xl p-6">
    <h3 class="text-lg font-bold mb-2">Open & Extensible</h3>
    <p class="text-gray-400">API-first. CLI-native. Works with your existing tools.</p>
  </div>
</section>

<section id="pricing" class="max-w-4xl mx-auto py-16 px-6">
  <h2 class="text-3xl font-bold text-center mb-12">Pricing</h2>
  <div class="grid md:grid-cols-3 gap-6">
    <div class="border border-gray-800 rounded-xl p-6 text-center">
      <h3 class="font-bold text-lg">Free</h3>
      <p class="text-4xl font-bold my-4">$0</p>
      <p class="text-gray-400 mb-6">For getting started</p>
      <a href="#" class="block bg-gray-800 py-2 rounded-lg hover:bg-gray-700">Start Free</a>
    </div>
    <div class="border-2 border-blue-600 rounded-xl p-6 text-center">
      <h3 class="font-bold text-lg">Pro</h3>
      <p class="text-4xl font-bold my-4">$29<span class="text-lg text-gray-400">/mo</span></p>
      <p class="text-gray-400 mb-6">For professionals</p>
      <a href="#" class="block bg-blue-600 py-2 rounded-lg hover:bg-blue-500">Go Pro</a>
    </div>
    <div class="border border-gray-800 rounded-xl p-6 text-center">
      <h3 class="font-bold text-lg">Team</h3>
      <p class="text-4xl font-bold my-4">$79<span class="text-lg text-gray-400">/mo</span></p>
      <p class="text-gray-400 mb-6">For collaboration</p>
      <a href="#" class="block bg-gray-800 py-2 rounded-lg hover:bg-gray-700">Contact Us</a>
    </div>
  </div>
</section>

<footer class="border-t border-gray-800 py-8 text-center text-gray-500 text-sm">
  &copy; 2026 {slug}. Built with speed.
</footer>
</body>
</html>"##,
        headline = headline,
        subheadline = subheadline,
        slug = slug,
    );

    let project_dir = dir.join("build/project");
    std::fs::write(project_dir.join("index.html"), &landing_html)
        .map_err(|e| format!("write index.html: {}", e))?;
    state.log("build", "landing_page", true, "index.html generated");
    println!("    {} index.html generated", "✓".green());

    // ── Step 3: Generate SEO Content ─────────────────────────────────────
    println!("  {} Generating SEO content...", "→".cyan());

    let content_dir = dir.join("build/content");
    let mut content_count = 0;

    for (i, question) in top_questions.iter().take(5).enumerate() {
        let q_str = question.as_str();
        let article = format!(
            r#"# {}

## Introduction
If you're searching for answers about {}, you're in the right place. This guide covers everything you need to know.

## What You Need to Know
{}

## How It Works
The key to understanding {} is breaking it down into simple steps.

## Why This Matters
In the {} space, getting this right can save you time and money.

## Getting Started
Ready to try it? [Start free]({}) and see results in minutes.

## FAQ
**Q: Is this free?**
A: Yes — there's a free tier. Pro starts at $29/mo.

**Q: How is this different from alternatives?**
A: Speed, simplicity, and price. Built with Rust for maximum performance.
"#,
            q_str,
            niche,
            q_str,
            q_str,
            niche,
            state.deployed_url.as_deref().unwrap_or("#"),
        );

        let filename = format!("{:02}-{}.md", i + 1, slugify_short(q_str));
        std::fs::write(content_dir.join(&filename), &article)
            .map_err(|e| format!("write content: {}", e))?;
        content_count += 1;
    }

    state.log("build", "content", true, &format!("{} articles generated", content_count));
    println!("    {} {} articles generated", "✓".green(), content_count);

    // ── Step 4: Deploy (if wrangler available) ───────────────────────────
    println!("  {} Checking deploy options...", "→".cyan());

    let has_wrangler = tokio::process::Command::new("which")
        .arg("wrangler")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_wrangler {
        println!("    {} wrangler found — deploy with:", "→".yellow());
        println!("      {}", format!("cd {} && wrangler pages deploy .", project_dir.to_string_lossy()).cyan());
    } else {
        println!("    {} No wrangler — files ready for manual deploy", "−".dimmed());
        println!("      {}", format!("Files at: {}", project_dir.to_string_lossy()).dimmed());
    }

    state.deployed_url = Some(format!("https://{}.pages.dev", state.slug));
    state.log("build", "deploy_check", true, if has_wrangler { "wrangler available" } else { "manual deploy" });

    // ── Complete ─────────────────────────────────────────────────────────
    state.transition(Phase::Built)?;
    state.save()?;

    println!();
    display::status("BUILD complete", "✓");
    display::kv("Spec", &spec_path.to_string_lossy());
    display::kv("Landing page", &project_dir.join("index.html").to_string_lossy());
    display::kv("Articles", &content_count.to_string());
    display::kv("Deploy URL", state.deployed_url.as_deref().unwrap_or("pending"));
    println!();
    println!("  Next: {}", "dx spawn sell --dry-run".cyan().bold());
    println!();

    Ok(())
}

fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Read {}: {}", path.to_string_lossy(), e))?;
    serde_json::from_str(&json)
        .map_err(|e| format!("Parse {}: {}", path.to_string_lossy(), e))
}

fn titlecase(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify_short(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-")
}
