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
}

static ROUTER: OnceLock<Mutex<AgentRouter>> = OnceLock::new();

impl Default for AgentRouter {
    fn default() -> Self {
        let providers = vec![
            ProviderConfig {
                name: "claude".to_string(),
                command: "claude".to_string(),
                cost_per_hour: 18.0,
                strengths: vec![
                    "rust".to_string(),
                    "debugging".to_string(),
                    "architecture".to_string(),
                ],
            },
            ProviderConfig {
                name: "codex".to_string(),
                command: "codex".to_string(),
                cost_per_hour: 12.0,
                strengths: vec![
                    "typescript".to_string(),
                    "javascript".to_string(),
                    "docs".to_string(),
                    "refactoring".to_string(),
                ],
            },
            ProviderConfig {
                name: "gemini".to_string(),
                command: "gemini".to_string(),
                cost_per_hour: 10.0,
                strengths: vec![
                    "analysis".to_string(),
                    "research".to_string(),
                    "docs".to_string(),
                ],
            },
            ProviderConfig {
                name: "aider".to_string(),
                command: "aider".to_string(),
                cost_per_hour: 8.0,
                strengths: vec![
                    "python".to_string(),
                    "editing".to_string(),
                    "git".to_string(),
                ],
            },
        ];

        let routing_rules = vec![
            RoutingRule {
                task_pattern: r"(?i)\brust\b".to_string(),
                prefer_provider: "claude".to_string(),
                reason: "Rust work defaults to Claude for stronger systems-language performance."
                    .to_string(),
            },
            RoutingRule {
                task_pattern: r"(?i)\b(type\s*script|typescript|ts|react|next\.?js)\b".to_string(),
                prefer_provider: "codex".to_string(),
                reason: "TypeScript and frontend work defaults to Codex.".to_string(),
            },
            RoutingRule {
                task_pattern: r"(?i)\b(debug|debugging|panic|traceback|regression|failing test)\b"
                    .to_string(),
                prefer_provider: "claude".to_string(),
                reason: "Debug-heavy tasks default to Claude.".to_string(),
            },
            RoutingRule {
                task_pattern: r"(?i)\b(doc|docs|documentation|readme|guide)\b".to_string(),
                prefer_provider: "codex".to_string(),
                reason: "Documentation work defaults to Codex.".to_string(),
            },
        ];

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
}

impl AgentRouter {
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
            let recommendation = RouteRecommendation {
                provider: provider.name.clone(),
                command: provider.command.clone(),
                score,
                reasons,
                language: language.clone(),
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
        let provider_cfg = self
            .providers
            .iter()
            .find(|candidate| candidate.name == provider)
            .ok_or_else(|| anyhow!("unknown provider '{}'", provider))?;
        let stats = self
            .usage_stats
            .entry(provider.to_string())
            .or_insert_with(UsageStats::default);

        let completed_before = stats.tasks_completed as f64;
        let completed_after = completed_before + 1.0;
        let success_value = if success { 1.0 } else { 0.0 };

        stats.avg_time_secs =
            ((stats.avg_time_secs * completed_before) + duration_secs.max(0.0)) / completed_after;
        stats.success_rate =
            ((stats.success_rate * completed_before) + success_value) / completed_after;
        stats.total_cost += provider_cfg.cost_per_hour * duration_secs.max(0.0) / 3600.0;
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
        if !self
            .providers
            .iter()
            .any(|candidate| candidate.name == provider)
        {
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
                json!({
                    "provider": provider.name,
                    "command": provider.command,
                    "cost_per_hour": provider.cost_per_hour,
                    "strengths": provider.strengths,
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

pub fn current_router() -> Result<AgentRouter> {
    let guard = router()
        .lock()
        .map_err(|_| anyhow!("agent router lock poisoned"))?;
    Ok(guard.clone())
}

fn router() -> &'static Mutex<AgentRouter> {
    ROUTER.get_or_init(|| {
        let router = load_router().unwrap_or_else(|_| AgentRouter::default());
        Mutex::new(router)
    })
}

fn save_router(router: &AgentRouter) -> Result<()> {
    let path = router_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(router)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn load_router() -> Result<AgentRouter> {
    let path = router_path();
    if !path.exists() {
        return Ok(AgentRouter::default());
    }
    let contents = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&contents).context("parse agent router state")
}

fn router_path() -> PathBuf {
    crate::config::dx_root().join("agent_router.json")
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
}
