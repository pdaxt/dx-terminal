use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRouter {
    providers: Vec<ProviderConfig>,
    routing_rules: Vec<RoutingRule>,
    usage_stats: HashMap<String, UsageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub command: String,
    pub cost_per_hour: f64,
    pub strengths: Vec<String>,
    #[serde(default)]
    pub available: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub task_pattern: String,
    pub prefer_provider: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub tasks_completed: u32,
    pub avg_time_secs: f64,
    pub success_rate: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRecommendation {
    pub provider: String,
    pub command: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub language: Option<String>,
    pub available: bool,
}

/// Optional config-file section that users can add to
/// `~/.config/dx-terminal/config.json` under the key `"router"`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouterConfig {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

static ROUTER: OnceLock<Mutex<AgentRouter>> = OnceLock::new();

fn builtin_providers() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            name: "claude".into(),
            command: "claude".into(),
            cost_per_hour: 18.0,
            strengths: vec!["rust".into(), "debugging".into(), "architecture".into()],
            available: None,
        },
        ProviderConfig {
            name: "codex".into(),
            command: "codex".into(),
            cost_per_hour: 12.0,
            strengths: vec![
                "typescript".into(),
                "javascript".into(),
                "docs".into(),
                "refactoring".into(),
            ],
            available: None,
        },
        ProviderConfig {
            name: "gemini".into(),
            command: "gemini".into(),
            cost_per_hour: 10.0,
            strengths: vec!["analysis".into(), "research".into(), "docs".into()],
            available: None,
        },
        ProviderConfig {
            name: "aider".into(),
            command: "aider".into(),
            cost_per_hour: 8.0,
            strengths: vec!["python".into(), "editing".into(), "git".into()],
            available: None,
        },
    ]
}

fn builtin_rules() -> Vec<RoutingRule> {
    vec![
        RoutingRule {
            task_pattern: r"(?i)\brust\b".into(),
            prefer_provider: "claude".into(),
            reason: "Rust work defaults to Claude for stronger systems-language performance.".into(),
        },
        RoutingRule {
            task_pattern: r"(?i)\b(type\s*script|typescript|ts|react|next\.?js)\b".into(),
            prefer_provider: "codex".into(),
            reason: "TypeScript and frontend work defaults to Codex.".into(),
        },
        RoutingRule {
            task_pattern: r"(?i)\b(debug|debugging|panic|traceback|regression|failing test)\b"
                .into(),
            prefer_provider: "claude".into(),
            reason: "Debug-heavy tasks default to Claude.".into(),
        },
        RoutingRule {
            task_pattern: r"(?i)\b(doc|docs|documentation|readme|guide)\b".into(),
            prefer_provider: "codex".into(),
            reason: "Documentation work defaults to Codex.".into(),
        },
    ]
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::with_config(RouterConfig::default())
    }
}

impl AgentRouter {
    /// Build a router by merging builtins with user-supplied config.
    /// Config providers with the same `name` as a builtin replace the builtin;
    /// others are appended. Config rules always append after builtins.
    pub fn with_config(config: RouterConfig) -> Self {
        let mut providers = builtin_providers();
        for cfg_provider in config.providers {
            if let Some(existing) = providers.iter_mut().find(|p| p.name == cfg_provider.name) {
                *existing = cfg_provider;
            } else {
                providers.push(cfg_provider);
            }
        }

        let mut routing_rules = builtin_rules();
        for cfg_rule in config.rules {
            routing_rules.push(cfg_rule);
        }

        let usage_stats = providers
            .iter()
            .map(|provider| (provider.name.clone(), UsageStats::default()))
            .collect();

        Self {
            providers,
            routing_rules,
            usage_stats,
        }
    }

    /// Check which providers have their CLI available on `$PATH`.
    pub fn check_availability(&mut self) {
        for provider in &mut self.providers {
            provider.available = Some(command_exists(&provider.command));
        }
    }

    pub fn is_available(&self, provider_name: &str) -> bool {
        self.providers
            .iter()
            .find(|p| p.name == provider_name)
            .and_then(|p| p.available)
            .unwrap_or_else(|| {
                self.providers
                    .iter()
                    .find(|p| p.name == provider_name)
                    .map(|p| command_exists(&p.command))
                    .unwrap_or(false)
            })
    }

    pub fn route_task(
        &self,
        description: &str,
        language: Option<&str>,
    ) -> Result<RouteRecommendation> {
        let description = description.trim();
        if description.is_empty() {
            bail!("description is required");
        }

        let language = language
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let search_text = match &language {
            Some(language) => format!("{description} {language}"),
            None => description.to_string(),
        };
        let lower_text = search_text.to_ascii_lowercase();

        let mut best: Option<(RouteRecommendation, f64)> = None;
        for provider in &self.providers {
            let (score, reasons) =
                self.provider_score(provider, &lower_text, language.as_deref())?;
            let available = provider
                .available
                .unwrap_or_else(|| command_exists(&provider.command));
            let recommendation = RouteRecommendation {
                provider: provider.name.clone(),
                command: provider.command.clone(),
                score,
                reasons,
                language: language.clone(),
                available,
            };
            match &best {
                Some((_, best_score)) if *best_score >= score => {}
                _ => best = Some((recommendation, score)),
            }
        }

        let (recommendation, _) = best.ok_or_else(|| anyhow!("no providers configured"))?;
        Ok(recommendation)
    }

    pub fn record_completion(
        &mut self,
        provider: &str,
        _task: &str,
        duration_secs: f64,
        success: bool,
    ) -> Result<()> {
        if !self.providers.iter().any(|p| p.name == provider) {
            bail!("unknown provider '{}'", provider);
        }
        let stats = self.usage_stats.entry(provider.to_string()).or_default();
        let cost_per_hour = self
            .providers
            .iter()
            .find(|p| p.name == provider)
            .map(|p| p.cost_per_hour)
            .unwrap_or(0.0);

        let completed_before = stats.tasks_completed as f64;
        let completed_after = completed_before + 1.0;
        let success_value = if success { 1.0 } else { 0.0 };
        let clamped_duration = duration_secs.max(0.0);

        stats.avg_time_secs =
            ((stats.avg_time_secs * completed_before) + clamped_duration) / completed_after;
        stats.success_rate =
            ((stats.success_rate * completed_before) + success_value) / completed_after;
        stats.total_cost += cost_per_hour * clamped_duration / 3600.0;
        stats.tasks_completed += 1;
        Ok(())
    }

    pub fn add_routing_rule(
        &mut self,
        pattern: &str,
        provider: &str,
        reason: &str,
    ) -> Result<RoutingRule> {
        Regex::new(pattern).with_context(|| format!("invalid regex '{}'", pattern))?;
        if !self.providers.iter().any(|p| p.name == provider) {
            bail!("unknown provider '{}'", provider);
        }
        let rule = RoutingRule {
            task_pattern: pattern.to_string(),
            prefer_provider: provider.to_string(),
            reason: reason.trim().to_string(),
        };
        self.routing_rules.push(rule.clone());
        Ok(rule)
    }

    /// Remove routing rules whose pattern matches `pattern` exactly.
    /// Returns the number of rules removed.
    pub fn remove_routing_rule(&mut self, pattern: &str) -> usize {
        let before = self.routing_rules.len();
        self.routing_rules
            .retain(|rule| rule.task_pattern != pattern);
        before - self.routing_rules.len()
    }

    pub fn cost_report(&self) -> Value {
        let providers = self
            .providers
            .iter()
            .map(|provider| {
                let stats = self
                    .usage_stats
                    .get(&provider.name)
                    .cloned()
                    .unwrap_or_default();
                let cost_per_task = if stats.tasks_completed == 0 {
                    0.0
                } else {
                    stats.total_cost / stats.tasks_completed as f64
                };
                json!({
                    "provider": provider.name,
                    "command": provider.command,
                    "cost_per_hour": provider.cost_per_hour,
                    "tasks_completed": stats.tasks_completed,
                    "avg_time_secs": stats.avg_time_secs,
                    "success_rate": stats.success_rate,
                    "total_cost": stats.total_cost,
                    "cost_per_task": cost_per_task,
                })
            })
            .collect::<Vec<_>>();
        let total_cost = self
            .usage_stats
            .values()
            .map(|stats| stats.total_cost)
            .sum::<f64>();

        json!({
            "providers": providers,
            "total_cost": total_cost,
        })
    }

    pub fn stats_json(&self) -> Value {
        let providers = self
            .providers
            .iter()
            .map(|provider| {
                let stats = self
                    .usage_stats
                    .get(&provider.name)
                    .cloned()
                    .unwrap_or_default();
                let available = provider
                    .available
                    .unwrap_or_else(|| command_exists(&provider.command));
                json!({
                    "provider": provider.name,
                    "command": provider.command,
                    "cost_per_hour": provider.cost_per_hour,
                    "strengths": provider.strengths,
                    "available": available,
                    "stats": stats,
                })
            })
            .collect::<Vec<_>>();

        json!({
            "providers": providers,
            "rules": self.routing_rules,
        })
    }

    fn provider_score(
        &self,
        provider: &ProviderConfig,
        text: &str,
        language: Option<&str>,
    ) -> Result<(f64, Vec<String>)> {
        let stats = self
            .usage_stats
            .get(&provider.name)
            .cloned()
            .unwrap_or_default();
        let mut score = 20.0;
        let mut reasons = Vec::new();

        if stats.tasks_completed > 0 {
            score += stats.success_rate * 40.0;
            score += 20.0 / (1.0 + (stats.avg_time_secs / 1800.0));
            reasons.push(format!(
                "historical success {:.0}% over {} tasks",
                stats.success_rate * 100.0,
                stats.tasks_completed
            ));
        } else {
            score += 15.0;
            reasons.push("no history yet; using baseline score".to_string());
        }

        let cost_penalty = provider.cost_per_hour / 2.0;
        score -= cost_penalty;
        reasons.push(format!("cost bias {:.2}/hr", provider.cost_per_hour));

        for strength in &provider.strengths {
            let strength_lower = strength.to_ascii_lowercase();
            if text.contains(&strength_lower)
                || language
                    .map(|language| language.eq_ignore_ascii_case(&strength_lower))
                    .unwrap_or(false)
            {
                score += 18.0;
                reasons.push(format!("matched strength '{}'", strength));
            }
        }

        for (index, rule) in self.routing_rules.iter().enumerate() {
            let regex = Regex::new(&rule.task_pattern)
                .with_context(|| format!("invalid stored regex '{}'", rule.task_pattern))?;
            if regex.is_match(text) {
                let weight = 45.0 + (index as f64 * 5.0);
                if rule.prefer_provider == provider.name {
                    score += weight;
                    reasons.push(rule.reason.clone());
                } else {
                    score -= weight / 6.0;
                }
            }
        }

        Ok((score, reasons))
    }
}

// ---------------------------------------------------------------------------
// Module-level functions (global singleton interface)
// ---------------------------------------------------------------------------

pub fn route_task(description: &str, language: Option<&str>) -> Result<RouteRecommendation> {
    let guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    guard.route_task(description, language)
}

pub fn record_completion(
    provider: &str,
    task: &str,
    duration_secs: f64,
    success: bool,
) -> Result<()> {
    let mut guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    guard.record_completion(provider, task, duration_secs, success)?;
    save_router(&guard)
}

pub fn cost_report() -> Result<Value> {
    let guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    Ok(guard.cost_report())
}

pub fn agent_stats() -> Result<Value> {
    let guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    Ok(guard.stats_json())
}

pub fn add_routing_rule(pattern: &str, provider: &str, reason: &str) -> Result<RoutingRule> {
    let mut guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    let rule = guard.add_routing_rule(pattern, provider, reason)?;
    save_router(&guard)?;
    Ok(rule)
}

pub fn remove_routing_rule(pattern: &str) -> Result<usize> {
    let mut guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    let removed = guard.remove_routing_rule(pattern);
    if removed > 0 {
        save_router(&guard)?;
    }
    Ok(removed)
}

pub fn current_router() -> Result<AgentRouter> {
    let guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    Ok(guard.clone())
}

// ---------------------------------------------------------------------------
// Initialization & persistence
// ---------------------------------------------------------------------------

fn router() -> &'static Mutex<AgentRouter> {
    ROUTER.get_or_init(|| {
        let router = load_router().unwrap_or_else(|_| AgentRouter::default());
        Mutex::new(router)
    })
}

fn save_router(router: &AgentRouter) -> Result<()> {
    let path = router_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create router state directory {}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(router)?)
        .with_context(|| format!("write temporary router state {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("persist router state to {}", path.display()))?;
    Ok(())
}

/// Load router state, merging config-file overrides with builtins.
/// Persisted usage_stats are restored on top of the merged provider set.
fn load_router() -> Result<AgentRouter> {
    let config = load_router_config();
    let state_path = router_path();

    if state_path.exists() {
        let contents =
            std::fs::read(&state_path).with_context(|| format!("read {}", state_path.display()))?;
        let persisted: AgentRouter =
            serde_json::from_slice(&contents).context("parse agent router state")?;

        // Re-merge config on top of builtins, then restore persisted stats.
        let mut router = AgentRouter::with_config(config);
        for (provider_name, stats) in persisted.usage_stats {
            if router.providers.iter().any(|p| p.name == provider_name) {
                router.usage_stats.insert(provider_name, stats);
            }
        }
        // Restore any custom rules that were persisted beyond builtins+config.
        for rule in &persisted.routing_rules {
            let already_exists = router
                .routing_rules
                .iter()
                .any(|r| r.task_pattern == rule.task_pattern);
            if !already_exists {
                router.routing_rules.push(rule.clone());
            }
        }
        Ok(router)
    } else {
        Ok(AgentRouter::with_config(config))
    }
}

fn load_router_config() -> RouterConfig {
    let config_path = crate::config::dx_root().join("config.json");
    if !config_path.exists() {
        return RouterConfig::default();
    }
    let Ok(bytes) = std::fs::read(&config_path) else {
        return RouterConfig::default();
    };
    let Ok(root) = serde_json::from_slice::<Value>(&bytes) else {
        return RouterConfig::default();
    };
    root.get("router")
        .and_then(|v| serde_json::from_value::<RouterConfig>(v.clone()).ok())
        .unwrap_or_default()
}

fn router_path() -> PathBuf {
    crate::config::dx_root().join("agent_router.json")
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_routes_to_claude_by_default() {
        let router = AgentRouter::default();
        let recommendation = router
            .route_task(
                "Fix a Rust lifetime bug in the swarm scheduler",
                Some("rust"),
            )
            .unwrap();
        assert_eq!(recommendation.provider, "claude");
    }

    #[test]
    fn typescript_routes_to_codex_by_default() {
        let router = AgentRouter::default();
        let recommendation = router
            .route_task(
                "Polish the dashboard TypeScript table sorting",
                Some("typescript"),
            )
            .unwrap();
        assert_eq!(recommendation.provider, "codex");
    }

    #[test]
    fn record_completion_updates_cost_and_success() {
        let mut router = AgentRouter::default();
        router
            .record_completion("claude", "debug build", 1800.0, true)
            .unwrap();
        let stats = router.usage_stats.get("claude").unwrap();
        assert_eq!(stats.tasks_completed, 1);
        assert_eq!(stats.success_rate, 1.0);
        assert!(stats.total_cost > 0.0);
        assert_eq!(stats.avg_time_secs, 1800.0);
    }

    #[test]
    fn custom_rule_overrides_default_bias() {
        let mut router = AgentRouter::default();
        router
            .add_routing_rule(r"(?i)\bsecurity review\b", "gemini", "Custom AppSec rule")
            .unwrap();
        let recommendation = router
            .route_task("Security review for auth flow", Some("rust"))
            .unwrap();
        assert_eq!(recommendation.provider, "gemini");
    }

    #[test]
    fn empty_description_is_rejected() {
        let router = AgentRouter::default();
        assert!(router.route_task("", None).is_err());
        assert!(router.route_task("   ", None).is_err());
    }

    #[test]
    fn unknown_provider_completion_is_rejected() {
        let mut router = AgentRouter::default();
        assert!(router
            .record_completion("nonexistent", "task", 10.0, true)
            .is_err());
    }

    #[test]
    fn negative_duration_clamped_to_zero() {
        let mut router = AgentRouter::default();
        router
            .record_completion("claude", "task", -500.0, true)
            .unwrap();
        let stats = router.usage_stats.get("claude").unwrap();
        assert_eq!(stats.avg_time_secs, 0.0);
        assert_eq!(stats.total_cost, 0.0);
    }

    #[test]
    fn remove_routing_rule_by_pattern() {
        let mut router = AgentRouter::default();
        let pattern = r"(?i)\brust\b";
        let removed = router.remove_routing_rule(pattern);
        assert_eq!(removed, 1);
        assert!(!router
            .routing_rules
            .iter()
            .any(|r| r.task_pattern == pattern));
    }

    #[test]
    fn remove_nonexistent_rule_returns_zero() {
        let mut router = AgentRouter::default();
        assert_eq!(router.remove_routing_rule("no-such-pattern"), 0);
    }

    #[test]
    fn config_providers_merge_with_builtins() {
        let config = RouterConfig {
            providers: vec![
                // Override builtin claude with different cost
                ProviderConfig {
                    name: "claude".into(),
                    command: "claude".into(),
                    cost_per_hour: 25.0,
                    strengths: vec!["rust".into()],
                    available: None,
                },
                // Add a brand-new provider
                ProviderConfig {
                    name: "gpt".into(),
                    command: "gpt-cli".into(),
                    cost_per_hour: 15.0,
                    strengths: vec!["general".into()],
                    available: None,
                },
            ],
            rules: vec![RoutingRule {
                task_pattern: r"(?i)\bml\b".into(),
                prefer_provider: "gpt".into(),
                reason: "ML tasks prefer GPT.".into(),
            }],
        };
        let router = AgentRouter::with_config(config);

        // Claude cost overridden
        let claude = router.providers.iter().find(|p| p.name == "claude").unwrap();
        assert_eq!(claude.cost_per_hour, 25.0);

        // GPT added
        assert!(router.providers.iter().any(|p| p.name == "gpt"));

        // Builtin codex still present
        assert!(router.providers.iter().any(|p| p.name == "codex"));

        // Config rule appended after builtins
        assert!(router
            .routing_rules
            .iter()
            .any(|r| r.prefer_provider == "gpt"));
    }

    #[test]
    fn route_recommendation_includes_available_field() {
        let router = AgentRouter::default();
        let rec = router
            .route_task("Fix a Rust bug", Some("rust"))
            .unwrap();
        // `available` is a bool — we just verify the field exists and is populated
        let _ = rec.available;
    }

    #[test]
    fn cost_report_shape() {
        let router = AgentRouter::default();
        let report = router.cost_report();
        assert!(report.get("providers").unwrap().is_array());
        assert!(report.get("total_cost").unwrap().is_number());
    }

    #[test]
    fn add_rule_rejects_invalid_regex() {
        let mut router = AgentRouter::default();
        assert!(router
            .add_routing_rule("[invalid", "claude", "bad regex")
            .is_err());
    }

    #[test]
    fn add_rule_rejects_unknown_provider() {
        let mut router = AgentRouter::default();
        assert!(router
            .add_routing_rule(r"\btest\b", "nonexistent", "no such provider")
            .is_err());
    }
}
