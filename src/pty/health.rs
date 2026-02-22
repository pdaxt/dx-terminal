use super::PtyManager;
use super::output;

/// Health check result for a single pane
#[derive(Debug, Clone)]
pub struct PaneHealth {
    pub running: bool,
    pub has_output: bool,
    pub done: bool,
    pub error: Option<String>,
    pub done_marker: Option<String>,
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
        },
        Some(handle) => {
            let running = handle.is_running();
            let last_out = handle.last_output(30);
            let has_output = !last_out.trim().is_empty();
            let done_marker = output::check_completion(&last_out, markers);
            let shell = output::check_shell_prompt(&last_out);
            let error = output::check_errors(&last_out);

            PaneHealth {
                running,
                has_output,
                done: done_marker.is_some() || shell || !running,
                error,
                done_marker,
            }
        }
    }
}
