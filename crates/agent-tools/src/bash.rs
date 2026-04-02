use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct BashInput {
    pub command: String,
    pub timeout: Option<u64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub fn execute_bash(input: BashInput, cwd: &Path) -> Result<BashOutput> {
    let timeout_ms = input.timeout.unwrap_or(120_000);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let child = Command::new("sh")
            .arg("-lc")
            .arg(&input.command)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        match timeout(Duration::from_millis(timeout_ms), child.wait_with_output()).await {
            Ok(Ok(output)) => Ok(BashOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
                timed_out: false,
            }),
            Ok(Err(e)) => Err(dx_agent_core::DxosError::Io(e)),
            Err(_) => Ok(BashOutput {
                stdout: String::new(),
                stderr: format!("command timed out after {timeout_ms}ms"),
                exit_code: None,
                timed_out: true,
            }),
        }
    })
}
