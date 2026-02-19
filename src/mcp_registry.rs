use serde::{Deserialize, Serialize};

/// MCP capability descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInfo {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub projects: Vec<String>,
    pub keywords: Vec<String>,
    pub category: String,
}

/// Load the full MCP registry — combines static knowledge + dynamic from claude.json
pub fn load_registry() -> Vec<McpInfo> {
    let mut registry = built_in_registry();

    // Add any MCPs from claude.json that aren't in the static registry
    let claude_cfg = crate::claude::read_claude_config();
    if let Some(servers) = claude_cfg.get("mcpServers").and_then(|v| v.as_object()) {
        let known: Vec<String> = registry.iter().map(|m| m.name.clone()).collect();
        for name in servers.keys() {
            if !known.contains(name) {
                registry.push(McpInfo {
                    name: name.clone(),
                    description: format!("MCP server: {}", name),
                    capabilities: vec![],
                    projects: vec![],
                    keywords: vec![name.replace('-', " ").replace('_', " ")],
                    category: "unknown".into(),
                });
            }
        }
    }

    registry
}

/// Route: given a project and task description, return ranked MCP suggestions
pub fn route_mcps(project: &str, task: &str, role: &str) -> Vec<McpMatch> {
    let registry = load_registry();
    let query = format!("{} {} {}", project, task, role).to_lowercase();
    let project_lower = project.to_lowercase();

    let mut matches: Vec<McpMatch> = registry.iter().filter_map(|mcp| {
        let mut score: u32 = 0;
        let mut reasons = Vec::new();

        // Direct project match (highest signal)
        for p in &mcp.projects {
            if p.to_lowercase() == project_lower || project_lower.contains(&p.to_lowercase()) {
                score += 100;
                reasons.push(format!("project:{}", p));
            }
        }

        // Keyword match against task+project+role
        for kw in &mcp.keywords {
            if query.contains(&kw.to_lowercase()) {
                score += 30;
                reasons.push(format!("keyword:{}", kw));
            }
        }

        // Category match against role
        let role_categories = role_to_categories(role);
        if role_categories.contains(&mcp.category.as_str()) {
            score += 20;
            reasons.push(format!("role:{}", mcp.category));
        }

        // Infrastructure MCPs always get a baseline
        if mcp.category == "infrastructure" {
            score += 5;
        }

        if score > 0 {
            Some(McpMatch {
                name: mcp.name.clone(),
                score,
                reasons,
                description: mcp.description.clone(),
            })
        } else {
            None
        }
    }).collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

/// Lookup a single MCP by name
pub fn lookup(name: &str) -> Option<McpInfo> {
    load_registry().into_iter().find(|m| m.name == name)
}

/// Search MCPs by capability or keyword
pub fn search(query: &str) -> Vec<McpInfo> {
    let q = query.to_lowercase();
    load_registry().into_iter().filter(|mcp| {
        mcp.name.to_lowercase().contains(&q)
            || mcp.description.to_lowercase().contains(&q)
            || mcp.capabilities.iter().any(|c| c.to_lowercase().contains(&q))
            || mcp.keywords.iter().any(|k| k.to_lowercase().contains(&q))
            || mcp.category.to_lowercase().contains(&q)
    }).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct McpMatch {
    pub name: String,
    pub score: u32,
    pub reasons: Vec<String>,
    pub description: String,
}

fn role_to_categories(role: &str) -> Vec<&'static str> {
    match role {
        "frontend" => vec!["ui", "testing", "build"],
        "backend" => vec!["data", "api", "infrastructure"],
        "devops" => vec!["infrastructure", "monitoring", "deployment"],
        "qa" => vec!["testing", "monitoring"],
        "security" => vec!["security", "monitoring"],
        "pm" => vec!["project", "tracking", "communication"],
        "architect" => vec!["infrastructure", "data", "api"],
        "developer" => vec!["build", "testing", "data"],
        _ => vec![],
    }
}

/// Static registry of all known MCPs with rich metadata
fn built_in_registry() -> Vec<McpInfo> {
    vec![
        // === AgentOS Core ===
        McpInfo {
            name: "agentos".into(),
            description: "Agent orchestration: spawn, kill, assign, monitor Claude agents across 9 panes".into(),
            capabilities: vec!["spawn".into(), "kill".into(), "assign".into(), "collect".into(), "health".into(), "dashboard".into()],
            projects: vec![],
            keywords: vec!["agent".into(), "orchestrat".into(), "pane".into(), "spawn".into()],
            category: "infrastructure".into(),
        },
        McpInfo {
            name: "meta".into(),
            description: "Task routing: suggests which MCP to use for a given task".into(),
            capabilities: vec!["route_task".into()],
            projects: vec![],
            keywords: vec!["route".into(), "which mcp".into(), "help".into()],
            category: "infrastructure".into(),
        },

        // === Project Management ===
        McpInfo {
            name: "tracker".into(),
            description: "Issue tracking: create/update issues, sprints, milestones in collab spaces".into(),
            capabilities: vec!["issues".into(), "sprints".into(), "milestones".into(), "kanban".into()],
            projects: vec!["mailforge".into(), "dataxlr8".into(), "bskiller".into(), "triage-ai".into()],
            keywords: vec!["issue".into(), "sprint".into(), "milestone".into(), "kanban".into(), "ticket".into(), "bug".into(), "feature".into()],
            category: "tracking".into(),
        },
        McpInfo {
            name: "capacity".into(),
            description: "ACU capacity tracking: work logs, sprint burndown, role utilization".into(),
            capabilities: vec!["acu".into(), "work_log".into(), "burndown".into(), "utilization".into()],
            projects: vec![],
            keywords: vec!["capacity".into(), "acu".into(), "burndown".into(), "utilization".into(), "workload".into()],
            category: "tracking".into(),
        },
        McpInfo {
            name: "collab".into(),
            description: "Document collaboration: shared docs, comments, review workflows".into(),
            capabilities: vec!["documents".into(), "comments".into(), "reviews".into()],
            projects: vec![],
            keywords: vec!["document".into(), "collab".into(), "review".into(), "comment".into(), "share".into()],
            category: "communication".into(),
        },
        McpInfo {
            name: "hub".into(),
            description: "Live web dashboard: SSE events, agent overview, capacity gauges".into(),
            capabilities: vec!["dashboard".into(), "sse".into(), "monitoring".into()],
            projects: vec![],
            keywords: vec!["dashboard".into(), "monitor".into(), "overview".into()],
            category: "monitoring".into(),
        },
        McpInfo {
            name: "diagram".into(),
            description: "Architecture diagrams: Mermaid and D2 diagram generation".into(),
            capabilities: vec!["mermaid".into(), "d2".into(), "diagram".into()],
            projects: vec![],
            keywords: vec!["diagram".into(), "architecture".into(), "flowchart".into(), "sequence".into(), "mermaid".into()],
            category: "documentation".into(),
        },

        // === DataXLR8 ===
        McpInfo {
            name: "dataxlr8-employees".into(),
            description: "Employee management: CRUD, roles, training assignments".into(),
            capabilities: vec!["list_employees".into(), "add_employee".into(), "update_employee".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["employee".into(), "team".into(), "staff".into(), "hr".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "dataxlr8-deals".into(),
            description: "Sales pipeline: deals, activities, commissions tracking".into(),
            capabilities: vec!["list_deals".into(), "add_deal".into(), "activities".into(), "commissions".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["deal".into(), "sale".into(), "pipeline".into(), "commission".into(), "crm".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "dataxlr8-builds".into(),
            description: "Build management: deployment tracking, CI/CD status".into(),
            capabilities: vec!["list_builds".into(), "deploy_status".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["build".into(), "deploy".into(), "ci".into(), "cd".into()],
            category: "build".into(),
        },
        McpInfo {
            name: "dataxlr8-training".into(),
            description: "Training module management: courses, progress, completions".into(),
            capabilities: vec!["list_modules".into(), "track_progress".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["training".into(), "course".into(), "module".into(), "learning".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "dataxlr8-metrics".into(),
            description: "Business metrics: KPIs, dashboards, analytics".into(),
            capabilities: vec!["get_metrics".into(), "dashboards".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["metric".into(), "kpi".into(), "analytic".into(), "dashboard".into()],
            category: "monitoring".into(),
        },
        McpInfo {
            name: "dataxlr8-costs".into(),
            description: "Cost tracking: expenses, budgets, infrastructure costs".into(),
            capabilities: vec!["track_costs".into(), "budgets".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["cost".into(), "expense".into(), "budget".into(), "billing".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "dataxlr8-quotation".into(),
            description: "Quote generation: proposals, pricing, client quotes".into(),
            capabilities: vec!["create_quote".into(), "list_quotes".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["quote".into(), "proposal".into(), "pricing".into(), "estimate".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "dataxlr8-vision".into(),
            description: "Vision/image analysis: screenshot analysis, UI review".into(),
            capabilities: vec!["analyze_image".into(), "screenshot".into()],
            projects: vec!["dataxlr8".into()],
            keywords: vec!["vision".into(), "image".into(), "screenshot".into(), "ui review".into()],
            category: "testing".into(),
        },

        // === MailForge ===
        McpInfo {
            name: "mailforge-dns".into(),
            description: "DNS management: DKIM, SPF, DMARC records for email infrastructure".into(),
            capabilities: vec!["manage_dns".into(), "dkim".into(), "spf".into(), "dmarc".into()],
            projects: vec!["mailforge".into()],
            keywords: vec!["dns".into(), "dkim".into(), "spf".into(), "dmarc".into(), "domain".into()],
            category: "infrastructure".into(),
        },
        McpInfo {
            name: "mailforge-postal".into(),
            description: "Postal mail server: send emails, manage domains, check delivery".into(),
            capabilities: vec!["send_email".into(), "manage_domains".into(), "delivery_status".into()],
            projects: vec!["mailforge".into()],
            keywords: vec!["email".into(), "postal".into(), "smtp".into(), "send".into(), "deliver".into()],
            category: "infrastructure".into(),
        },
        McpInfo {
            name: "mailforge-monitor".into(),
            description: "Email monitoring: delivery rates, bounce tracking, reputation".into(),
            capabilities: vec!["delivery_stats".into(), "bounce_tracking".into(), "reputation".into()],
            projects: vec!["mailforge".into()],
            keywords: vec!["monitor".into(), "delivery".into(), "bounce".into(), "reputation".into(), "spam".into()],
            category: "monitoring".into(),
        },
        McpInfo {
            name: "mailforge-server".into(),
            description: "Server management: Postal instances, relay configuration".into(),
            capabilities: vec!["server_status".into(), "manage_relays".into()],
            projects: vec!["mailforge".into()],
            keywords: vec!["server".into(), "relay".into(), "postal".into(), "infrastructure".into()],
            category: "infrastructure".into(),
        },

        // === Knowledge & Storage ===
        McpInfo {
            name: "kgraph".into(),
            description: "Knowledge graph: store and query relationships, facts, entities".into(),
            capabilities: vec!["add_entity".into(), "query".into(), "relationships".into()],
            projects: vec![],
            keywords: vec!["knowledge".into(), "graph".into(), "entity".into(), "relationship".into(), "fact".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "vecstore".into(),
            description: "Vector store: semantic search, embeddings, similarity matching".into(),
            capabilities: vec!["store_embedding".into(), "search_similar".into()],
            projects: vec![],
            keywords: vec!["vector".into(), "embedding".into(), "semantic".into(), "search".into(), "similarity".into()],
            category: "data".into(),
        },
        McpInfo {
            name: "pqvault".into(),
            description: "Credential vault: secure storage for API keys, tokens, secrets".into(),
            capabilities: vec!["store_secret".into(), "get_secret".into()],
            projects: vec![],
            keywords: vec!["secret".into(), "credential".into(), "api key".into(), "token".into(), "vault".into(), "password".into()],
            category: "security".into(),
        },
        McpInfo {
            name: "experience".into(),
            description: "Experience tracking: lessons learned, patterns, debugging notes".into(),
            capabilities: vec!["record_experience".into(), "search_experiences".into()],
            projects: vec![],
            keywords: vec!["experience".into(), "lesson".into(), "pattern".into(), "debug".into(), "learn".into()],
            category: "documentation".into(),
        },
        McpInfo {
            name: "session-replay".into(),
            description: "Session replay: record and replay Claude sessions for analysis".into(),
            capabilities: vec!["record_session".into(), "replay".into(), "analyze".into()],
            projects: vec![],
            keywords: vec!["session".into(), "replay".into(), "record".into(), "history".into()],
            category: "monitoring".into(),
        },
    ]
}
