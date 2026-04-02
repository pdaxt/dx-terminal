mod bash;
mod edit;
mod file_read;
mod file_write;
mod glob_search;
mod grep_search;
mod registry;
pub mod repo_map;
#[cfg(test)]
mod tests;
mod web_fetch;

pub use bash::{execute_bash, BashInput, BashOutput};
pub use edit::{edit_file, EditInput, EditOutput};
pub use file_read::{read_file, ReadInput, ReadOutput};
pub use file_write::{write_file, WriteInput, WriteOutput};
pub use glob_search::{glob_files, GlobInput, GlobOutput};
pub use grep_search::{grep_content, GrepInput, GrepOutput};
pub use registry::{ToolRegistry, ToolSpec};
pub use repo_map::{repo_map, RepoMapInput, RepoMapOutput};
pub use web_fetch::{web_fetch, WebFetchInput, WebFetchOutput};

use dx_agent_core::{DxosError, Result};

/// Execute a tool by name with JSON input, returning JSON output.
pub fn execute_tool(name: &str, input: &str, cwd: &std::path::Path) -> Result<String> {
    match name {
        "bash" => {
            let input: BashInput = serde_json::from_str(input)?;
            let output = execute_bash(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "read_file" | "Read" => {
            let input: ReadInput = serde_json::from_str(input)?;
            let output = read_file(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "write_file" | "Write" => {
            let input: WriteInput = serde_json::from_str(input)?;
            let output = write_file(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "edit_file" | "Edit" => {
            let input: EditInput = serde_json::from_str(input)?;
            let output = edit_file(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "glob" | "Glob" => {
            let input: GlobInput = serde_json::from_str(input)?;
            let output = glob_files(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "grep" | "Grep" => {
            let input: GrepInput = serde_json::from_str(input)?;
            let output = grep_content(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        "web_fetch" | "WebFetch" => {
            let input: WebFetchInput = serde_json::from_str(input)?;
            let output = web_fetch(input)?;
            Ok(serde_json::to_string(&output)?)
        }
        "repo_map" | "RepoMap" => {
            let input: RepoMapInput = serde_json::from_str(input)?;
            let output = repo_map(input, cwd)?;
            Ok(serde_json::to_string(&output)?)
        }
        _ => Err(DxosError::Tool {
            tool: name.to_string(),
            message: format!("unknown tool: {name}"),
        }),
    }
}
