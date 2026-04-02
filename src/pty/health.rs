use super::output;
use super::PtyManager;

/// Minimum output lines required to consider a non-running agent as "done" vs "crashed"
const MIN_OUTPUT_LINES_FOR_DONE: usize = 5;

/// Health check result for a single pane
#[derive(Debug, Clone)]
pub struct PaneHealth {
    pub running: bool,
    pub has_output: bool,
    pub done: bool,
    pub error: Option<String>,
    pub done_marker: Option<String>,
    pub exit_code: Option<i32>,
}

/// Run health check on a specific pane
pub fn check_pane(pty_mgr: &PtyManager, pane: u8, markers: &[String]) -> PaneHealth {
    let agent = pty_mgr.agents.get(&pane);

    match agent {
        None => PaneHealth {
            running: false,
            has_output: false,
            done: false,
            error: None,
            done_marker: None,
            exit_code: None,
        },
        Some(handle) => {
            let running = handle.is_running();
            let exit_code = handle.exit_code();
            let last_out = handle.last_output(30);
            let has_output = !last_out.trim().is_empty();
            let line_count = handle.line_count();
            let done_marker = output::check_completion(&last_out, markers);
            let shell = output::check_shell_prompt(&last_out);

            // Process not running: distinguish "completed" from "crashed"
            // A process that exits with < MIN_OUTPUT_LINES is considered crashed
            let crashed = !running
                && line_count < MIN_OUTPUT_LINES_FOR_DONE
                && done_marker.is_none()
                && !shell;

            // Check output errors first, then crash detection, then non-zero exit code
            let error = if let Some(e) = output::check_errors(&last_out) {
                Some(e)
            } else if crashed {
                Some(format!(
                    "agent crashed (only {} output lines, exit={:?})",
                    line_count, exit_code
                ))
            } else if !running && exit_code.is_some_and(|c| c != 0 && c != -1) {
                Some(format!("process exited with code {}", exit_code.unwrap()))
            } else {
                None
            };

            // done = explicit marker OR shell prompt OR (not running AND enough output to be real)
            let done_from_exit = !running && !crashed;

            PaneHealth {
                running,
                has_output,
                done: done_marker.is_some() || shell || done_from_exit,
                error,
                done_marker,
                exit_code,
            }
        }
    }
}
