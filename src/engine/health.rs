use crate::scanner::{self, ProjectInfo};
use crate::quality;

/// Result of running tests for a project
pub struct TestResult {
    pub success: bool,
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub duration_ms: i64,
    pub output: String,
}

/// Run test command for a project, log result via quality::log_test
pub async fn run_tests(info: &ProjectInfo) -> Option<TestResult> {
    let test_cmd = info.test_cmd.as_ref()?;

    let start = std::time::Instant::now();
    let output = tokio::process::Command::new("sh")
        .args(["-c", test_cmd])
        .current_dir(&info.path)
        .output()
        .await
        .ok()?;

    let duration_ms = start.elapsed().as_millis() as i64;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);
    let success = output.status.success();

    let (total, passed, failed) = parse_test_output(&combined, &info.tech);

    // Log via quality system
    quality::log_test(
        "health-monitor", &info.name, Some(test_cmd), success,
        Some(total), Some(passed), Some(failed), Some(0),
        Some(duration_ms), Some(&combined),
    );

    Some(TestResult { success, total, passed, failed, duration_ms, output: combined })
}

/// Run build command for a project, log result via quality::log_build
#[allow(dead_code)]
pub async fn run_build(info: &ProjectInfo) -> Option<bool> {
    let build_cmd = info.build_cmd.as_ref()?;

    let start = std::time::Instant::now();
    let output = tokio::process::Command::new("sh")
        .args(["-c", build_cmd])
        .current_dir(&info.path)
        .output()
        .await
        .ok()?;

    let duration_ms = start.elapsed().as_millis() as i64;
    let combined = format!("{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr));
    let success = output.status.success();

    quality::log_build(
        "health-monitor", &info.name, Some(build_cmd), success,
        Some(duration_ms), Some(&combined),
    );

    Some(success)
}

/// Parse test output to extract pass/fail counts based on tech stack
fn parse_test_output(output: &str, tech: &[String]) -> (i64, i64, i64) {
    let re_rust = regex::Regex::new(r"test result: \w+\. (\d+) passed; (\d+) failed").ok();
    let re_jest = regex::Regex::new(r"Tests:\s+(\d+) passed,?\s*(\d+)? failed").ok();
    let re_pytest = regex::Regex::new(r"(\d+) passed(?:,\s*(\d+) failed)?").ok();

    if tech.contains(&"rust".to_string()) {
        if let Some(re) = re_rust {
            if let Some(caps) = re.captures(output) {
                let passed: i64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let failed: i64 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                return (passed + failed, passed, failed);
            }
        }
    }

    if tech.contains(&"node".to_string()) || tech.contains(&"typescript".to_string()) {
        if let Some(re) = re_jest {
            if let Some(caps) = re.captures(output) {
                let passed: i64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let failed: i64 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                return (passed + failed, passed, failed);
            }
        }
    }

    if tech.contains(&"python".to_string()) {
        if let Some(re) = re_pytest {
            if let Some(caps) = re.captures(output) {
                let passed: i64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let failed: i64 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                return (passed + failed, passed, failed);
            }
        }
    }

    // Fallback: just check exit code (success = 1 passed, 0 failed)
    (0, 0, 0)
}

/// Check if project needs health checking (changed since last test)
fn needs_check(info: &ProjectInfo) -> bool {
    // Must have a test command
    if info.test_cmd.is_none() { return false; }
    // Dirty repos always need checking
    if info.git_dirty { return true; }
    // Check if last commit is newer than last test run
    let gate = quality::quality_gate(&info.name);
    let last_test_ts = gate.get("tests")
        .and_then(|v| v.get("last_run"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if last_test_ts.is_empty() { return true; } // Never tested
    // Compare timestamps
    if let Some(commit_ts) = &info.last_commit_ts {
        commit_ts > &last_test_ts.to_string()
    } else {
        false
    }
}

/// Full health cycle: check projects that have changed
pub async fn health_cycle() {
    let reg = scanner::load_registry();
    let mut checked = 0;

    for proj in reg.projects.iter().filter(|p| needs_check(p)) {
        if checked >= 3 { break; } // max 3 per cycle to avoid CPU saturation

        // Skip projects with active agents (tests would interfere)
        let agents = crate::multi_agent::agent_list(Some(&proj.name));
        if agents.get("count").and_then(|v| v.as_i64()).unwrap_or(0) > 0 {
            continue;
        }

        tracing::info!("Health check: testing {}", proj.name);
        if let Some(result) = run_tests(proj).await {
            if !result.success {
                tracing::warn!("Health check: {} FAILED ({}/{} passed)", proj.name, result.passed, result.total);
            }
        }
        checked += 1;
    }
}
