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
    }
}

/// Parse a colon-command string into a TuiCommand
pub fn parse_command(input: &str) -> Option<TuiCommand> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();
    match parts.first().map(|s| s.to_lowercase()).as_deref() {
        Some("spawn" | "s") if parts.len() >= 3 => {
            Some(TuiCommand::Spawn {
                pane: parts[1].to_string(),
                project: parts[2].to_string(),
                role: None,
                task: None,
            })
        }
        Some("kill" | "k") if parts.len() >= 2 => {
            Some(TuiCommand::Kill {
                pane: parts[1].to_string(),
                reason: parts.get(2).map(|s| s.to_string()),
            })
        }
        Some("done" | "complete") if parts.len() >= 2 => {
            Some(TuiCommand::Complete {
                pane: parts[1].to_string(),
                summary: parts.get(2).map(|s| s.to_string()),
            })
        }
        Some("auto" | "cycle") => {
            Some(TuiCommand::AutoCycle)
        }
        Some("feature" | "feat") if parts.len() >= 3 => {
            Some(TuiCommand::FeatureCreate {
                space: parts[1].to_string(),
                title: parts[2].to_string(),
                issue_type: "feature".into(),
                priority: None,
            })
        }
        Some("queue-feature" | "qf") if parts.len() >= 3 => {
            let ids: Vec<String> = parts[2].split(',').map(|s| s.trim().to_string()).collect();
            Some(TuiCommand::FeatureToQueue {
                space: parts[1].to_string(),
                issue_ids: ids,
            })
        }
        _ => None,
    }
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}
