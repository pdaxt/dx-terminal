use anyhow::Result;
use serde::Serialize;
use std::process::Command;

const DX_BOOTSTRAP_FILES: &[&str] = &[
    "DX_SHARED_GUIDANCE_PATH",
    "DX_PROVIDER_GUIDANCE_PATH",
    "DX_AUTOMATION_GUIDE_PATH",
    "DX_PROVIDER_BRIDGE_PATH",
    "DX_AUTOMATION_BRIDGE_PROJECT_PATH",
    "DX_AUTOMATION_BRIDGE_USER_PATH",
    "DX_WORKFLOW_CATALOG_PROJECT_PATH",
    "DX_WORKFLOW_CATALOG_USER_PATH",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProvider {
    Claude,
    Codex,
    Gemini,
    OpenCode,
}

impl RuntimeProvider {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "codex" | "openai" => Self::Codex,
            "gemini" | "google" => Self::Gemini,
            "opencode" | "open-code" => Self::OpenCode,
            _ => Self::Claude,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex CLI",
            Self::Gemini => "Gemini CLI",
            Self::OpenCode => "OpenCode",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini",
            Self::OpenCode => "Open",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeLaunchPlan {
    pub adapter: String,
    pub provider: String,
    pub provider_label: String,
    pub binary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub autonomous: bool,
    pub project_path: String,
    pub window_name: String,
    pub command: String,
    pub bootstrap_mode: String,
    pub bootstrap_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderAvailability {
    pub provider: String,
    pub label: String,
    pub preferred_adapter: String,
    pub supported_adapters: Vec<String>,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdapterAvailability {
    pub adapter: String,
    pub label: String,
    pub substrate: String,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeAdapter {
    TmuxMigration,
    PtyNative,
}

impl RuntimeAdapter {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "pty" | "pty_native" | "pty_native_adapter" | "dx_pty" => Self::PtyNative,
            _ => Self::TmuxMigration,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::TmuxMigration => "tmux_migration_adapter",
            Self::PtyNative => "pty_native_adapter",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::TmuxMigration => "tmux migration adapter",
            Self::PtyNative => "DX PTY adapter",
        }
    }

    pub fn substrate(&self) -> &'static str {
        match self {
            Self::TmuxMigration => "tmux_window",
            Self::PtyNative => "custom_pty_target",
        }
    }
}

pub fn provider_label(provider: &str) -> &'static str {
    RuntimeProvider::from_str(provider).label()
}

pub fn provider_short(provider: &str) -> &'static str {
    RuntimeProvider::from_str(provider).short_label()
}

pub fn normalize_provider_id(provider: &str) -> &'static str {
    RuntimeProvider::from_str(provider).id()
}

pub fn provider_inventory() -> Vec<ProviderAvailability> {
    [
        RuntimeProvider::Claude,
        RuntimeProvider::Codex,
        RuntimeProvider::Gemini,
        RuntimeProvider::OpenCode,
    ]
    .into_iter()
    .map(|provider| {
        let binary = resolve_provider_binary(provider);
        ProviderAvailability {
            provider: provider.id().to_string(),
            label: provider.label().to_string(),
            preferred_adapter: RuntimeAdapter::PtyNative.id().to_string(),
            supported_adapters: vec![
                RuntimeAdapter::PtyNative.id().to_string(),
                RuntimeAdapter::TmuxMigration.id().to_string(),
            ],
            available: binary.is_some(),
            binary,
        }
    })
    .collect()
}

pub fn adapter_inventory() -> Vec<AdapterAvailability> {
    [RuntimeAdapter::PtyNative, RuntimeAdapter::TmuxMigration]
        .into_iter()
        .map(|adapter| AdapterAvailability {
            adapter: adapter.id().to_string(),
            label: adapter.label().to_string(),
            substrate: adapter.substrate().to_string(),
            available: true,
        })
        .collect()
}

pub fn normalize_adapter_id(adapter: Option<&str>) -> &'static str {
    RuntimeAdapter::from_str(adapter.unwrap_or("pty_native_adapter")).id()
}

pub fn plan_launch(
    adapter: Option<&str>,
    provider: &str,
    window_name: &str,
    project_path: &str,
    prompt: &str,
    autonomous: bool,
    model: Option<&str>,
) -> Result<RuntimeLaunchPlan> {
    let adapter = RuntimeAdapter::from_str(adapter.unwrap_or("pty_native_adapter"));
    let provider = RuntimeProvider::from_str(provider);
    let binary = resolve_provider_binary(provider).ok_or_else(|| {
        anyhow::anyhow!(
            "{} binary not found on PATH or standard install locations",
            provider.label()
        )
    })?;
    let normalized_model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let command = build_provider_command_with_binary(
        provider,
        &binary,
        prompt,
        autonomous,
        normalized_model.as_deref(),
    );

    Ok(RuntimeLaunchPlan {
        adapter: adapter.id().to_string(),
        provider: provider.id().to_string(),
        provider_label: provider.label().to_string(),
        binary,
        model: normalized_model,
        autonomous,
        project_path: project_path.to_string(),
        window_name: window_name.to_string(),
        command,
        bootstrap_mode: "dx_runtime_bootstrap".to_string(),
        bootstrap_files: DX_BOOTSTRAP_FILES
            .iter()
            .map(|item| item.to_string())
            .collect(),
    })
}

pub fn plan_tmux_launch(
    provider: &str,
    window_name: &str,
    project_path: &str,
    prompt: &str,
    autonomous: bool,
    model: Option<&str>,
) -> Result<RuntimeLaunchPlan> {
    plan_launch(
        Some("tmux_migration_adapter"),
        provider,
        window_name,
        project_path,
        prompt,
        autonomous,
        model,
    )
}

fn build_provider_command_with_binary(
    provider: RuntimeProvider,
    binary: &str,
    prompt: &str,
    autonomous: bool,
    model: Option<&str>,
) -> String {
    let escaped_prompt = shell_escape(&decorate_prompt_with_bootstrap(provider, prompt));
    let model_arg = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(shell_escape);

    match provider {
        RuntimeProvider::Claude => {
            let perms_flag = if autonomous {
                " --dangerously-skip-permissions"
            } else {
                ""
            };
            let model_flag = model_arg
                .as_deref()
                .map(|value| format!(" --model {}", value))
                .unwrap_or_default();
            format!(
                "{}{}{} -p {}",
                binary, perms_flag, model_flag, escaped_prompt
            )
        }
        RuntimeProvider::Codex => {
            let auto_flag = if autonomous { " --full-auto" } else { "" };
            let model_flag = model_arg
                .as_deref()
                .map(|value| format!(" -m {}", value))
                .unwrap_or_default();
            format!("{}{}{} {}", binary, auto_flag, model_flag, escaped_prompt)
        }
        RuntimeProvider::Gemini => {
            let auto_flag = if autonomous { " --yolo" } else { "" };
            let model_flag = model_arg
                .as_deref()
                .map(|value| format!(" -m {}", value))
                .unwrap_or_default();
            format!(
                "{}{}{} --prompt-interactive {}",
                binary, auto_flag, model_flag, escaped_prompt
            )
        }
        RuntimeProvider::OpenCode => {
            let model_flag = model_arg
                .as_deref()
                .map(|value| format!(" -m {}", value))
                .unwrap_or_default();
            format!("{}{} {}", binary, model_flag, escaped_prompt)
        }
    }
}

fn decorate_prompt_with_bootstrap(provider: RuntimeProvider, prompt: &str) -> String {
    let provider_label = provider.label();
    let provider_runtime = match provider {
        RuntimeProvider::Claude => "Claude Code",
        RuntimeProvider::Codex => "Codex CLI",
        RuntimeProvider::Gemini => "Gemini CLI",
        RuntimeProvider::OpenCode => "OpenCode",
    };
    let mut sections = vec![
        "## DX Runtime Bootstrap".to_string(),
        format!("- Runtime provider: {} ({})", provider_label, provider_runtime),
        "- Before major work, consult the DX runtime guidance files if they exist.".to_string(),
        "- Canonical guidance files: $DX_SHARED_GUIDANCE_PATH, $DX_PROVIDER_GUIDANCE_PATH, $DX_AUTOMATION_GUIDE_PATH.".to_string(),
        "- Canonical bridge manifests: $DX_PROVIDER_BRIDGE_PATH, $DX_AUTOMATION_BRIDGE_PROJECT_PATH, $DX_AUTOMATION_BRIDGE_USER_PATH.".to_string(),
        "- Structured workflow catalogs: $DX_WORKFLOW_CATALOG_PROJECT_PATH, $DX_WORKFLOW_CATALOG_USER_PATH.".to_string(),
        "- Prefer DX-shared workflows and bridged commands/skills over inventing a new local workflow.".to_string(),
        "- If the task conflicts with the shared guidance or bridge contracts, surface the contradiction explicitly.".to_string(),
    ];

    match provider {
        RuntimeProvider::Claude => sections.push(
            "- Use the DX guidance and automation files as the first execution context before tool-heavy work.".to_string(),
        ),
        RuntimeProvider::Codex => sections.push(
            "- Treat the DX guidance files as the startup contract for this Codex lane and preserve them as read-only context.".to_string(),
        ),
        RuntimeProvider::Gemini => sections.push(
            "- Start from the DX guidance files and automation bridge inventory before drafting or executing workflow steps.".to_string(),
        ),
        RuntimeProvider::OpenCode => sections.push(
            "- Follow the DX runtime bootstrap files before acting so this lane stays interoperable with the other providers.".to_string(),
        ),
    }

    let trimmed_prompt = prompt.trim();
    if trimmed_prompt.is_empty() {
        sections.join("\n")
    } else {
        format!("{}\n\n{}", sections.join("\n"), trimmed_prompt)
    }
}

fn resolve_provider_binary(provider: RuntimeProvider) -> Option<String> {
    let candidates: &[&str] = match provider {
        RuntimeProvider::Claude => &[
            "/opt/homebrew/bin/claude",
            "/usr/local/bin/claude",
            "claude",
        ],
        RuntimeProvider::Codex => &[
            "/Users/pran/.nvm/versions/node/v22.22.0/bin/codex",
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "codex",
        ],
        RuntimeProvider::Gemini => &[
            "/opt/homebrew/bin/gemini",
            "/usr/local/bin/gemini",
            "gemini",
        ],
        RuntimeProvider::OpenCode => &[
            "/opt/homebrew/bin/opencode",
            "/usr/local/bin/opencode",
            "opencode",
        ],
    };

    for candidate in candidates {
        if candidate.contains('/') {
            if std::path::Path::new(candidate).exists() {
                return Some((*candidate).to_string());
            }
        } else if let Ok(output) = Command::new("which").arg(candidate).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runtime_provider_names() {
        assert_eq!(RuntimeProvider::from_str("claude"), RuntimeProvider::Claude);
        assert_eq!(RuntimeProvider::from_str("codex"), RuntimeProvider::Codex);
        assert_eq!(RuntimeProvider::from_str("openai"), RuntimeProvider::Codex);
        assert_eq!(RuntimeProvider::from_str("gemini"), RuntimeProvider::Gemini);
        assert_eq!(
            RuntimeProvider::from_str("opencode"),
            RuntimeProvider::OpenCode
        );
        assert_eq!(
            RuntimeProvider::from_str("something-else"),
            RuntimeProvider::Claude
        );
    }

    #[test]
    fn normalizes_runtime_adapters() {
        assert_eq!(
            RuntimeAdapter::from_str("pty_native_adapter"),
            RuntimeAdapter::PtyNative
        );
        assert_eq!(RuntimeAdapter::from_str("pty"), RuntimeAdapter::PtyNative);
        assert_eq!(
            RuntimeAdapter::from_str("tmux_migration_adapter"),
            RuntimeAdapter::TmuxMigration
        );
    }

    #[test]
    fn builds_claude_command_with_model_and_permissions() {
        let plan = RuntimeLaunchPlan {
            adapter: "tmux_migration_adapter".to_string(),
            provider: "claude".to_string(),
            provider_label: "Claude Code".to_string(),
            binary: "/bin/claude".to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            autonomous: true,
            project_path: "/tmp/demo".to_string(),
            window_name: "dx-claude".to_string(),
            command: build_provider_command_with_binary(
                RuntimeProvider::Claude,
                "/bin/claude",
                "ship it",
                true,
                Some("claude-sonnet-4-6"),
            ),
            bootstrap_mode: "dx_runtime_bootstrap".to_string(),
            bootstrap_files: DX_BOOTSTRAP_FILES
                .iter()
                .map(|item| item.to_string())
                .collect(),
        };
        assert!(plan.command.contains("/bin/claude"));
        assert!(plan.command.contains("--dangerously-skip-permissions"));
        assert!(plan.command.contains("--model 'claude-sonnet-4-6'"));
        assert!(plan.command.contains("DX_SHARED_GUIDANCE_PATH"));
        assert!(plan.command.contains("ship it"));
    }

    #[test]
    fn builds_codex_command_with_full_auto() {
        let cmd = build_provider_command_with_binary(
            RuntimeProvider::Codex,
            "/bin/codex",
            "review this diff",
            true,
            Some("gpt-5.4"),
        );
        assert!(cmd.contains("/bin/codex"));
        assert!(cmd.contains("--full-auto"));
        assert!(cmd.contains("-m 'gpt-5.4'"));
        assert!(cmd.contains("DX_AUTOMATION_GUIDE_PATH"));
        assert!(cmd.contains("review this diff"));
    }

    #[test]
    fn builds_gemini_interactive_command() {
        let cmd = build_provider_command_with_binary(
            RuntimeProvider::Gemini,
            "/bin/gemini",
            "design three options",
            false,
            Some("gemini-2.5-pro"),
        );
        assert!(cmd.contains("/bin/gemini"));
        assert!(cmd.contains("--prompt-interactive"));
        assert!(cmd.contains("-m 'gemini-2.5-pro'"));
        assert!(cmd.contains("DX_PROVIDER_BRIDGE_PATH"));
        assert!(cmd.contains("design three options"));
        assert!(!cmd.contains("--yolo"));
    }

    #[test]
    fn launch_plan_can_target_pty_adapter() {
        let plan = plan_launch(
            Some("pty_native_adapter"),
            "codex",
            "dx-codex-1",
            "/tmp/demo",
            "review this diff",
            true,
            Some("gpt-5.4"),
        );
        if let Ok(plan) = plan {
            assert_eq!(plan.adapter, "pty_native_adapter");
            assert_eq!(plan.provider, "codex");
            assert_eq!(plan.bootstrap_mode, "dx_runtime_bootstrap");
            assert!(plan
                .bootstrap_files
                .iter()
                .any(|item| item == "DX_AUTOMATION_GUIDE_PATH"));
        }
    }

    #[test]
    fn provider_inventory_prefers_pty_and_exposes_supported_adapters() {
        let providers = provider_inventory();
        let claude = providers
            .iter()
            .find(|item| item.provider == "claude")
            .expect("claude provider inventory entry");
        assert_eq!(claude.preferred_adapter, "pty_native_adapter");
        assert!(claude
            .supported_adapters
            .iter()
            .any(|item| item == "pty_native_adapter"));
        assert!(claude
            .supported_adapters
            .iter()
            .any(|item| item == "tmux_migration_adapter"));
    }
}
