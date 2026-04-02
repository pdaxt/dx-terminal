use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

impl PermissionMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::FullAccess => "full-access",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allow,
    Deny { reason: String },
}

pub trait PermissionPrompter {
    fn decide(&mut self, tool_name: &str, input: &str) -> PermissionOutcome;
}

#[derive(Debug, Clone)]
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_requirements: BTreeMap<String, PermissionMode>,
}

impl PermissionPolicy {
    #[must_use]
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            active_mode: mode,
            tool_requirements: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_tool(mut self, name: impl Into<String>, required: PermissionMode) -> Self {
        self.tool_requirements.insert(name.into(), required);
        self
    }

    #[must_use]
    pub fn authorize(
        &self,
        tool_name: &str,
        input: &str,
        prompter: Option<&mut dyn PermissionPrompter>,
    ) -> PermissionOutcome {
        let required = self
            .tool_requirements
            .get(tool_name)
            .copied()
            .unwrap_or(PermissionMode::FullAccess);

        if self.active_mode >= required {
            return PermissionOutcome::Allow;
        }

        // If we're in workspace-write and tool needs full-access, prompt
        if self.active_mode == PermissionMode::WorkspaceWrite
            && required == PermissionMode::FullAccess
        {
            if let Some(prompter) = prompter {
                return prompter.decide(tool_name, input);
            }
        }

        PermissionOutcome::Deny {
            reason: format!(
                "{tool_name} requires {} (current: {})",
                required.as_str(),
                self.active_mode.as_str()
            ),
        }
    }
}
