use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use dx_agent_core::{DxosError, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct EditInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditOutput {
    pub path: String,
    pub replacements: usize,
}

pub fn edit_file(input: EditInput, cwd: &Path) -> Result<EditOutput> {
    let file_path = if Path::new(&input.path).is_absolute() {
        input.path.clone()
    } else {
        cwd.join(&input.path).to_string_lossy().to_string()
    };

    let content = fs::read_to_string(&file_path)?;

    if input.old_string == input.new_string {
        return Err(DxosError::Tool {
            tool: "edit".to_string(),
            message: "old_string and new_string are identical".to_string(),
        });
    }

    let (new_content, replacements) = if input.replace_all {
        let count = content.matches(&input.old_string).count();
        (content.replace(&input.old_string, &input.new_string), count)
    } else {
        let count = content.matches(&input.old_string).count();
        if count == 0 {
            return Err(DxosError::Tool {
                tool: "edit".to_string(),
                message: "old_string not found in file".to_string(),
            });
        }
        if count > 1 {
            return Err(DxosError::Tool {
                tool: "edit".to_string(),
                message: format!(
                    "old_string matches {count} locations — provide more context to make it unique, or use replace_all"
                ),
            });
        }
        (content.replacen(&input.old_string, &input.new_string, 1), 1)
    };

    fs::write(&file_path, new_content)?;

    Ok(EditOutput {
        path: file_path,
        replacements,
    })
}
