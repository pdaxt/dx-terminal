use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct WriteInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WriteOutput {
    pub path: String,
    pub bytes_written: usize,
}

pub fn write_file(input: WriteInput, cwd: &Path) -> Result<WriteOutput> {
    let file_path = if Path::new(&input.path).is_absolute() {
        input.path.clone()
    } else {
        cwd.join(&input.path).to_string_lossy().to_string()
    };

    // Create parent directories if needed
    if let Some(parent) = Path::new(&file_path).parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = input.content.len();
    fs::write(&file_path, &input.content)?;

    Ok(WriteOutput {
        path: file_path,
        bytes_written: bytes,
    })
}
