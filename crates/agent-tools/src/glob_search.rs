use std::path::Path;

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct GlobInput {
    pub pattern: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobOutput {
    pub matches: Vec<String>,
    pub count: usize,
}

pub fn glob_files(input: GlobInput, cwd: &Path) -> Result<GlobOutput> {
    let search_dir = input.path.as_deref().map(Path::new).unwrap_or(cwd);

    let glob_pattern =
        glob::Pattern::new(&input.pattern).map_err(|e| dx_agent_core::DxosError::Tool {
            tool: "glob".to_string(),
            message: format!("invalid pattern: {e}"),
        })?;

    let mut matches = Vec::new();

    for entry in WalkBuilder::new(search_dir)
        .hidden(false)
        .git_ignore(true)
        .build()
        .flatten()
    {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if glob_pattern.matches(name) {
                matches.push(path.to_string_lossy().to_string());
            }
        }
        // Also try matching the relative path
        if let Ok(rel) = path.strip_prefix(search_dir) {
            if glob_pattern.matches_path(rel)
                && !matches.contains(&path.to_string_lossy().to_string())
            {
                matches.push(path.to_string_lossy().to_string());
            }
        }
    }

    matches.sort();
    let count = matches.len();

    Ok(GlobOutput { matches, count })
}
