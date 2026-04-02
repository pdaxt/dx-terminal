use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionLevel {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub permission: PermissionLevel,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    specs: Vec<ToolSpec>,
}

impl ToolRegistry {
    /// Build the default lean tool set — 9 native tools.
    #[must_use]
    pub fn default_cli() -> Self {
        Self {
            specs: builtin_tools(),
        }
    }

    #[must_use]
    pub fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    /// Render tool definitions for the system prompt (Anthropic API format).
    #[must_use]
    pub fn to_api_definitions(&self) -> Vec<Value> {
        self.specs
            .iter()
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "description": spec.description,
                    "input_schema": spec.input_schema,
                })
            })
            .collect()
    }
}

fn builtin_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "bash",
            description: "Execute a shell command. Returns stdout, stderr, and exit code.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The shell command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in milliseconds (default: 120000)" },
                    "description": { "type": "string", "description": "What this command does" }
                },
                "required": ["command"]
            }),
            permission: PermissionLevel::FullAccess,
        },
        ToolSpec {
            name: "read_file",
            description: "Read a file. Returns numbered lines. Supports offset/limit for large files.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "offset": { "type": "integer", "description": "Start line (0-indexed)" },
                    "limit": { "type": "integer", "description": "Max lines to return (default: 2000)" }
                },
                "required": ["path"]
            }),
            permission: PermissionLevel::ReadOnly,
        },
        ToolSpec {
            name: "write_file",
            description: "Write content to a file. Creates parent directories if needed.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
            permission: PermissionLevel::WorkspaceWrite,
        },
        ToolSpec {
            name: "edit_file",
            description: "Replace exact string in a file. old_string must be unique unless replace_all is true.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string" },
                    "new_string": { "type": "string" },
                    "replace_all": { "type": "boolean", "default": false }
                },
                "required": ["path", "old_string", "new_string"]
            }),
            permission: PermissionLevel::WorkspaceWrite,
        },
        ToolSpec {
            name: "glob",
            description: "Find files matching a glob pattern. Respects .gitignore.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern (e.g. **/*.rs)" },
                    "path": { "type": "string", "description": "Directory to search in" }
                },
                "required": ["pattern"]
            }),
            permission: PermissionLevel::ReadOnly,
        },
        ToolSpec {
            name: "grep",
            description: "Search file contents with regex. Respects .gitignore.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern" },
                    "path": { "type": "string", "description": "Directory to search in" },
                    "glob": { "type": "string", "description": "File filter (e.g. *.rs)" },
                    "case_insensitive": { "type": "boolean" },
                    "max_results": { "type": "integer", "default": 500 }
                },
                "required": ["pattern"]
            }),
            permission: PermissionLevel::ReadOnly,
        },
        ToolSpec {
            name: "git",
            description: "Run a git command in the workspace.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string", "description": "Git subcommand and arguments (e.g. 'status', 'diff HEAD~1')" }
                },
                "required": ["args"]
            }),
            permission: PermissionLevel::WorkspaceWrite,
        },
        ToolSpec {
            name: "web_fetch",
            description: "Fetch content from a URL. Returns text content with HTML tags stripped.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to fetch" },
                    "max_length": { "type": "integer", "description": "Max characters to return (default: 50000)" }
                },
                "required": ["url"]
            }),
            permission: PermissionLevel::ReadOnly,
        },
        ToolSpec {
            name: "repo_map",
            description: "Generate a condensed map of definitions (functions, structs, classes, etc.) across source files in a directory. Uses tree-sitter for Rust and regex for other languages.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to scan (defaults to cwd)" },
                    "max_files": { "type": "integer", "description": "Max files to process (default: 500)" }
                },
                "required": []
            }),
            permission: PermissionLevel::ReadOnly,
        },
    ]
}
