/// Check output for completion markers
pub fn check_completion(output: &str, markers: &[String]) -> Option<String> {
    for marker in markers {
        if output.contains(marker.as_str()) {
            return Some(marker.clone());
        }
    }
    None
}

/// Check output for shell prompt (Claude exited back to shell)
pub fn check_shell_prompt(output: &str) -> bool {
    let lines: Vec<&str> = output.trim().lines().collect();
    if let Some(last) = lines.last() {
        let trimmed = last.trim();
        return trimmed.ends_with('$')
            || trimmed.ends_with("$ ")
            || trimmed.contains("Claude exited")
            || trimmed.ends_with('%')
            || trimmed.ends_with("% ");
    }
    false
}

/// Check output for error patterns
pub fn check_errors(output: &str) -> Option<String> {
    let patterns = [
        "Error:", "FATAL:", "panic:", "Traceback",
        "rate limit", "hit your limit", "SIGTERM",
    ];
    for pat in &patterns {
        if output.contains(pat) {
            return Some(pat.to_string());
        }
    }
    None
}
