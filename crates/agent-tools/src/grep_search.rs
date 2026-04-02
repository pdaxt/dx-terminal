use std::path::Path;

use ignore::WalkBuilder;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct GrepInput {
    pub pattern: String,
    pub path: Option<String>,
    pub glob: Option<String>,
    #[serde(rename = "case_insensitive")]
    pub case_insensitive: Option<bool>,
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrepMatch {
    pub file: String,
    pub line: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<GrepMatch>,
    pub count: usize,
    pub truncated: bool,
}

pub fn grep_content(input: GrepInput, cwd: &Path) -> Result<GrepOutput> {
    let search_dir = input.path.as_deref().map(Path::new).unwrap_or(cwd);

    let case_insensitive = input.case_insensitive.unwrap_or(false);
    let max_results = input.max_results.unwrap_or(500);

    let re = RegexBuilder::new(&input.pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|e| dx_agent_core::DxosError::Tool {
            tool: "grep".to_string(),
            message: format!("invalid regex: {e}"),
        })?;

    let glob_pattern = input.glob.as_ref().and_then(|g| glob::Pattern::new(g).ok());

    let mut matches = Vec::new();
    let mut truncated = false;

    for entry in WalkBuilder::new(search_dir)
        .hidden(false)
        .git_ignore(true)
        .build()
        .flatten()
    {
        if matches.len() >= max_results {
            truncated = true;
            break;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Apply glob filter
        if let Some(ref pattern) = glob_pattern {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !pattern.matches(name) {
                    continue;
                }
            }
        }

        // Read and search
        if let Ok(content) = std::fs::read_to_string(path) {
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(GrepMatch {
                        file: path.to_string_lossy().to_string(),
                        line: line_num + 1,
                        content: line.to_string(),
                    });
                    if matches.len() >= max_results {
                        truncated = true;
                        break;
                    }
                }
            }
        }
    }

    let count = matches.len();
    Ok(GrepOutput {
        matches,
        count,
        truncated,
    })
}
