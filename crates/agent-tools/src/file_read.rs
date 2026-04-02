use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct ReadInput {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadOutput {
    pub path: String,
    pub content: String,
    pub num_lines: usize,
    pub start_line: usize,
    pub total_lines: usize,
}

pub fn read_file(input: ReadInput, cwd: &Path) -> Result<ReadOutput> {
    let file_path = if Path::new(&input.path).is_absolute() {
        input.path.clone()
    } else {
        cwd.join(&input.path).to_string_lossy().to_string()
    };

    let content = fs::read_to_string(&file_path)?;
    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();

    let offset = input.offset.unwrap_or(0);
    let limit = input.limit.unwrap_or(2000);

    let selected: Vec<String> = all_lines
        .iter()
        .enumerate()
        .skip(offset)
        .take(limit)
        .map(|(i, line)| format!("{:>6}\t{line}", i + 1))
        .collect();

    Ok(ReadOutput {
        path: file_path,
        content: selected.join("\n"),
        num_lines: selected.len(),
        start_line: offset + 1,
        total_lines,
    })
}
