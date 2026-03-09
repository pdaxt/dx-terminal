use super::TuiCommand;

/// A single field in a form
#[derive(Clone)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub cursor: usize,
    pub required: bool,
    pub placeholder: String,
}

/// Form state for multi-field input
#[derive(Clone)]
pub struct FormState {
    pub title: String,
    pub fields: Vec<FormField>,
    pub focused: usize,
    pub kind: FormKind,
}

#[derive(Clone)]
pub enum FormKind {
    Spawn,
    QueueAdd,
    FeatureCreate,
    Orchestrate,
    FactoryGo,
    AddScreen,
    RemoveScreen,
}

/// Create a spawn form pre-filled with selected pane
pub fn create_spawn_form(selected_pane: u8) -> FormState {
    FormState {
        title: "Spawn Agent".into(),
        fields: vec![
            FormField {
                label: "Pane".into(),
                value: selected_pane.to_string(),
                cursor: 1,
                required: true,
                placeholder: "1-9".into(),
            },
            FormField {
                label: "Project".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "project name or path".into(),
            },
            FormField {
                label: "Role".into(),
                value: "developer".into(),
                cursor: 9,
                required: false,
                placeholder: "developer/reviewer/architect".into(),
            },
            FormField {
                label: "Task".into(),
                value: String::new(),
                cursor: 0,
                required: false,
                placeholder: "task description".into(),
            },
        ],
        focused: 1, // Start on project field
        kind: FormKind::Spawn,
    }
}

/// Create a queue-add form
pub fn create_queue_form() -> FormState {
    FormState {
        title: "Add to Queue".into(),
        fields: vec![
            FormField {
                label: "Project".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "project name".into(),
            },
            FormField {
                label: "Task".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "task description".into(),
            },
            FormField {
                label: "Role".into(),
                value: "developer".into(),
                cursor: 9,
                required: false,
                placeholder: "developer/reviewer".into(),
            },
            FormField {
                label: "Priority".into(),
                value: "3".into(),
                cursor: 1,
                required: false,
                placeholder: "1-5 (1=highest)".into(),
            },
        ],
        focused: 0,
        kind: FormKind::QueueAdd,
    }
}

/// Create a feature-create form
pub fn create_feature_form() -> FormState {
    FormState {
        title: "Create Feature".into(),
        fields: vec![
            FormField {
                label: "Space".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "collab space name".into(),
            },
            FormField {
                label: "Title".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "feature title".into(),
            },
            FormField {
                label: "Priority".into(),
                value: "medium".into(),
                cursor: 6,
                required: false,
                placeholder: "critical/high/medium/low".into(),
            },
        ],
        focused: 0,
        kind: FormKind::FeatureCreate,
    }
}

/// Create an orchestrate form — one field for natural language, optional project
pub fn create_orchestrate_form() -> FormState {
    FormState {
        title: "Orchestrate".into(),
        fields: vec![
            FormField {
                label: "Request".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "What do you want built? (natural language)".into(),
            },
            FormField {
                label: "Project".into(),
                value: String::new(),
                cursor: 0,
                required: false,
                placeholder: "auto-detected if empty".into(),
            },
        ],
        focused: 0,
        kind: FormKind::Orchestrate,
    }
}

/// Create an add-screen form
pub fn create_add_screen_form() -> FormState {
    FormState {
        title: "Add Screen".into(),
        fields: vec![
            FormField {
                label: "Name".into(),
                value: String::new(),
                cursor: 0,
                required: false,
                placeholder: "screen name (auto-generated if empty)".into(),
            },
            FormField {
                label: "Layout".into(),
                value: "grid2x2".into(),
                cursor: 6,
                required: false,
                placeholder: "single/split2/horizontal/vertical/grid2x2".into(),
            },
            FormField {
                label: "Panes".into(),
                value: "4".into(),
                cursor: 1,
                required: false,
                placeholder: "number of panes (1-6)".into(),
            },
        ],
        focused: 0,
        kind: FormKind::AddScreen,
    }
}

/// Create a remove-screen form
pub fn create_remove_screen_form() -> FormState {
    FormState {
        title: "Remove Screen".into(),
        fields: vec![
            FormField {
                label: "Screen".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "screen ID or name".into(),
            },
        ],
        focused: 0,
        kind: FormKind::RemoveScreen,
    }
}

/// Check if all required fields have values
pub fn form_is_valid(form: &FormState) -> bool {
    form.fields.iter().all(|f| !f.required || !f.value.trim().is_empty())
}

/// Convert a completed form into a TuiCommand
pub fn form_to_command(form: &FormState) -> Option<TuiCommand> {
    match form.kind {
        FormKind::Spawn => {
            let pane = form.fields[0].value.trim().to_string();
            let project = form.fields[1].value.trim().to_string();
            let role = non_empty(&form.fields[2].value);
            let task = non_empty(&form.fields[3].value);
            Some(TuiCommand::Spawn { pane, project, role, task })
        }
        FormKind::QueueAdd => {
            let project = form.fields[0].value.trim().to_string();
            let task = form.fields[1].value.trim().to_string();
            let role = non_empty(&form.fields[2].value);
            let priority = form.fields[3].value.trim().parse::<u8>().ok();
            Some(TuiCommand::QueueAdd { project, task, role, priority })
        }
        FormKind::FeatureCreate => {
            let space = form.fields[0].value.trim().to_string();
            let title = form.fields[1].value.trim().to_string();
            let priority = non_empty(&form.fields[2].value);
            Some(TuiCommand::FeatureCreate { space, title, issue_type: "feature".into(), priority })
        }
        FormKind::Orchestrate => {
            let request = form.fields[0].value.trim().to_string();
            let project = non_empty(&form.fields[1].value);
            Some(TuiCommand::Orchestrate { request, project })
        }
        FormKind::FactoryGo => {
            let project = non_empty(&form.fields[0].value);
            let request = form.fields[1].value.trim().to_string();
            let template = non_empty(&form.fields[2].value);
            Some(TuiCommand::FactoryGo { project, request, template })
        }
        FormKind::AddScreen => {
            let name = non_empty(&form.fields[0].value);
            let layout = non_empty(&form.fields[1].value);
            let panes = form.fields[2].value.trim().parse::<u8>().ok();
            Some(TuiCommand::AddScreen { name, layout, panes })
        }
        FormKind::RemoveScreen => {
            let screen_ref = form.fields[0].value.trim().to_string();
            Some(TuiCommand::RemoveScreen { screen_ref, force: false })
        }
    }
}

/// Create a factory-go form — project, description, template
pub fn create_factory_form() -> FormState {
    FormState {
        title: "Factory Go".into(),
        fields: vec![
            FormField {
                label: "Project".into(),
                value: String::new(),
                cursor: 0,
                required: false,
                placeholder: "auto-detected if empty".into(),
            },
            FormField {
                label: "Request".into(),
                value: String::new(),
                cursor: 0,
                required: true,
                placeholder: "What to build (natural language)".into(),
            },
            FormField {
                label: "Template".into(),
                value: "full".into(),
                cursor: 4,
                required: false,
                placeholder: "full / quick / secure".into(),
            },
        ],
        focused: 0,
        kind: FormKind::FactoryGo,
    }
}

/// Parse a colon-command string into a TuiCommand.
/// Known shorthands get fast-path treatment. Everything else becomes McpDispatch.
pub fn parse_command(input: &str) -> Option<TuiCommand> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();
    let cmd_lower = parts.first().map(|s| s.to_lowercase());

    // Built-in commands (full names + shorthands)
    match cmd_lower.as_deref() {
        Some("talk" | "msg" | "tell") if parts.len() >= 3 => {
            if let Ok(pane) = parts[1].parse::<u8>() {
                return Some(TuiCommand::Talk {
                    pane,
                    message: parts[2].to_string(),
                });
            }
        }
        // :pause pipe_xxx — pause a pipeline
        Some("pause") if parts.len() >= 2 => {
            return Some(TuiCommand::McpDispatch {
                tool: "factory_pause".into(),
                args: serde_json::json!({ "pipeline_id": parts[1] }),
            });
        }
        // :resume pipe_xxx — resume a paused pipeline
        Some("resume") if parts.len() >= 2 => {
            return Some(TuiCommand::McpDispatch {
                tool: "factory_resume".into(),
                args: serde_json::json!({ "pipeline_id": parts[1] }),
            });
        }
        // :retry pipe_xxx [stage] — retry failed stages or a specific stage
        Some("retry") if parts.len() >= 2 => {
            if parts.len() >= 3 {
                // :retry pipe_xxx dev — retry specific stage
                return Some(TuiCommand::McpDispatch {
                    tool: "factory_retry_stage".into(),
                    args: serde_json::json!({ "pipeline_id": parts[1], "stage": parts[2] }),
                });
            }
            // :retry pipe_xxx — retry all failed
            return Some(TuiCommand::McpDispatch {
                tool: "factory_retry".into(),
                args: serde_json::json!({ "pipeline_id": parts[1] }),
            });
        }
        // :abort pipe_xxx — cancel/abort a pipeline
        Some("abort" | "cancel") if parts.len() >= 2 => {
            return Some(TuiCommand::McpDispatch {
                tool: "factory_cancel".into(),
                args: serde_json::json!({ "pipeline_id": parts[1] }),
            });
        }
        Some("spawn" | "s") if parts.len() >= 3 => {
            return Some(TuiCommand::Spawn {
                pane: parts[1].to_string(),
                project: parts[2].to_string(),
                role: None,
                task: None,
            });
        }
        Some("kill" | "k") if parts.len() >= 2 => {
            return Some(TuiCommand::Kill {
                pane: parts[1].to_string(),
                reason: parts.get(2).map(|s| s.to_string()),
            });
        }
        Some("complete" | "done") if parts.len() >= 2 => {
            return Some(TuiCommand::Complete {
                pane: parts[1].to_string(),
                summary: parts.get(2).map(|s| s.to_string()),
            });
        }
        Some("auto" | "cycle") => {
            return Some(TuiCommand::AutoCycle);
        }
        Some("feat") if parts.len() >= 3 => {
            return Some(TuiCommand::FeatureCreate {
                space: parts[1].to_string(),
                title: parts[2].to_string(),
                issue_type: "feature".into(),
                priority: None,
            });
        }
        Some("orchestrate" | "orch" | "build") if parts.len() >= 2 => {
            let rest = if parts.len() == 3 {
                format!("{} {}", parts[1], parts[2])
            } else {
                parts[1].to_string()
            };
            return Some(TuiCommand::Orchestrate {
                request: rest,
                project: None,
            });
        }
        // :go dataxlr8 add OAuth login
        // :go dataxlr8 --quick fix login bug
        // :go dataxlr8 --secure add payments
        Some("go") | Some("factory") | Some("work") if parts.len() >= 2 => {
            let rest = if parts.len() == 3 {
                format!("{} {}", parts[1], parts[2])
            } else {
                parts[1].to_string()
            };
            // First word is project, rest is description. Check for --template flags.
            let words: Vec<&str> = rest.splitn(2, ' ').collect();
            let project = words[0].to_string();
            let desc_raw = words.get(1).unwrap_or(&"").to_string();

            let (template, description) = if desc_raw.starts_with("--quick ") {
                (Some("quick".to_string()), desc_raw[8..].to_string())
            } else if desc_raw.starts_with("--secure ") {
                (Some("secure".to_string()), desc_raw[9..].to_string())
            } else if desc_raw.starts_with("--full ") {
                (Some("full".to_string()), desc_raw[7..].to_string())
            } else {
                (None, desc_raw)
            };

            if description.is_empty() {
                return None; // need at least project + description
            }

            return Some(TuiCommand::FactoryGo {
                project: Some(project),
                request: description,
                template,
            });
        }
        // :screen add [name] — add a new screen
        Some("screen") if parts.len() >= 2 && parts[1] == "add" => {
            let name = if parts.len() >= 3 { Some(parts[2].to_string()) } else { None };
            return Some(TuiCommand::AddScreen { name, layout: None, panes: None });
        }
        // :screen rm <name> — remove a screen
        Some("screen") if parts.len() >= 3 && (parts[1] == "rm" || parts[1] == "remove") => {
            return Some(TuiCommand::RemoveScreen { screen_ref: parts[2].to_string(), force: false });
        }
        Some("qf") if parts.len() >= 3 => {
            let ids: Vec<String> = parts[2].split(',').map(|s| s.trim().to_string()).collect();
            return Some(TuiCommand::FeatureToQueue {
                space: parts[1].to_string(),
                issue_ids: ids,
            });
        }
        _ => {}
    }

    // Universal MCP dispatch — first word is tool name, rest is key=value args
    let tool = parts.first()?.to_string();
    if tool.is_empty() { return None; }

    let args = if parts.len() > 1 {
        let rest = if parts.len() == 3 {
            format!("{} {}", parts[1], parts[2])
        } else {
            parts[1].to_string()
        };
        parse_args(&rest)
    } else {
        serde_json::json!({})
    };

    Some(TuiCommand::McpDispatch { tool, args })
}

/// Parse `key=value key2="quoted value"` into a JSON object.
/// Bare words without `=` are ignored (positional args not supported in universal dispatch).
fn parse_args(input: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut chars = input.chars().peekable();

    while chars.peek().is_some() {
        // Skip whitespace
        while chars.peek().map_or(false, |c| c.is_whitespace()) { chars.next(); }

        // Read key
        let mut key = String::new();
        while chars.peek().map_or(false, |c| *c != '=' && !c.is_whitespace()) {
            key.push(chars.next().unwrap());
        }

        if chars.peek() == Some(&'=') {
            chars.next(); // consume '='
            let mut value = String::new();
            if chars.peek() == Some(&'"') {
                chars.next(); // consume opening quote
                while chars.peek().map_or(false, |c| *c != '"') {
                    value.push(chars.next().unwrap());
                }
                chars.next(); // consume closing quote
            } else {
                while chars.peek().map_or(false, |c| !c.is_whitespace()) {
                    value.push(chars.next().unwrap());
                }
            }
            if !key.is_empty() {
                // Try to parse as number/bool, fall back to string
                if let Ok(n) = value.parse::<i64>() {
                    map.insert(key, serde_json::Value::Number(n.into()));
                } else if let Ok(b) = value.parse::<bool>() {
                    map.insert(key, serde_json::Value::Bool(b));
                } else {
                    map.insert(key, serde_json::Value::String(value));
                }
            }
        } else {
            // Bare word without = — if it's the only thing, treat as first required arg
            // For common patterns: `:who`, `:port_list` (no args needed)
            // For `:kb_search auth` — treat bare word as "query" for convenience
            if !key.is_empty() && map.is_empty() {
                // Read rest of input as the value
                let mut rest = key;
                while chars.peek().is_some() {
                    rest.push(chars.next().unwrap());
                }
                // Common first-arg field names by convention
                map.insert("query".to_string(), serde_json::Value::String(rest));
            }
        }
    }

    serde_json::Value::Object(map)
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}
