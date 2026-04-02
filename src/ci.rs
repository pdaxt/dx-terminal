//! Local CI gate: runs cargo check + test + clippy before push.
//!
//! Blocks the push if any step fails. Designed to prevent agents from
//! burning remote CI minutes on broken code.

use std::fmt;
use std::process::Command;
use std::time::{Duration, Instant};

/// Individual CI step result.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: &'static str,
    pub passed: bool,
    pub duration: Duration,
    pub stderr: String,
}

/// Overall CI gate result.
#[derive(Debug, Clone)]
pub struct CiResult {
    pub steps: Vec<StepResult>,
}

impl CiResult {
    /// True if every step passed.
    pub fn passed(&self) -> bool {
        self.steps.iter().all(|s| s.passed)
    }

    /// Total wall-clock time across all steps.
    pub fn total_duration(&self) -> Duration {
        self.steps.iter().map(|s| s.duration).sum()
    }
}

impl fmt::Display for CiResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "dx local CI gate")?;
        writeln!(f, "{}", "-".repeat(40))?;
        for step in &self.steps {
            let icon = if step.passed { "PASS" } else { "FAIL" };
            writeln!(
                f,
                "  {} {} ({:.1}s)",
                icon,
                step.name,
                step.duration.as_secs_f64()
            )?;
            // Show last 10 lines of stderr on failure to help debugging
            if !step.passed && !step.stderr.is_empty() {
                for line in step
                    .stderr
                    .lines()
                    .rev()
                    .take(10)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        writeln!(f, "    | {trimmed}")?;
                    }
                }
            }
        }
        writeln!(f, "{}", "-".repeat(40))?;
        let status = if self.passed() {
            "CI passed"
        } else {
            "CI FAILED -- push blocked"
        };
        writeln!(
            f,
            "  {} ({:.1}s total)",
            status,
            self.total_duration().as_secs_f64()
        )?;
        Ok(())
    }
}

/// Which checks to include in the CI gate.
#[derive(Debug, Clone)]
pub struct CiConfig {
    pub check: bool,
    pub test: bool,
    pub clippy: bool,
    /// Working directory (defaults to current dir if None).
    pub working_dir: Option<String>,
    /// Stop on first failure.
    pub fail_fast: bool,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            check: true,
            test: true,
            clippy: true,
            working_dir: None,
            fail_fast: true,
        }
    }
}

/// Run a single cargo command and return the result.
fn run_cargo_step(name: &'static str, args: &[&str], working_dir: Option<&str>) -> StepResult {
    let start = Instant::now();

    let mut cmd = Command::new("cargo");
    cmd.args(args);

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    let output = cmd.output();
    let duration = start.elapsed();

    match output {
        Ok(o) => StepResult {
            name,
            passed: o.status.success(),
            duration,
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        },
        Err(e) => StepResult {
            name,
            passed: false,
            duration,
            stderr: format!("Failed to execute cargo: {e}"),
        },
    }
}

/// Run the full CI gate. Returns the result (caller decides whether to block).
pub fn run(config: &CiConfig) -> CiResult {
    let dir = config.working_dir.as_deref();
    let mut steps = Vec::new();

    if config.check {
        let result = run_cargo_step("cargo check", &["check", "--workspace"], dir);
        let failed = !result.passed;
        steps.push(result);
        if failed && config.fail_fast {
            return CiResult { steps };
        }
    }

    if config.test {
        let result = run_cargo_step("cargo test", &["test", "--workspace"], dir);
        let failed = !result.passed;
        steps.push(result);
        if failed && config.fail_fast {
            return CiResult { steps };
        }
    }

    if config.clippy {
        let result = run_cargo_step(
            "cargo clippy",
            &[
                "clippy",
                "--workspace",
                "--",
                "-D",
                "clippy::correctness",
                "-W",
                "clippy::suspicious",
            ],
            dir,
        );
        let failed = !result.passed;
        steps.push(result);
        if failed && config.fail_fast {
            return CiResult { steps };
        }
    }

    CiResult { steps }
}

/// Convenience: run all checks with default config, return exit code (0 = pass).
pub fn gate() -> i32 {
    let result = run(&CiConfig::default());
    print!("{result}");
    if result.passed() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_all_steps() {
        let cfg = CiConfig::default();
        assert!(cfg.check);
        assert!(cfg.test);
        assert!(cfg.clippy);
        assert!(cfg.fail_fast);
    }

    #[test]
    fn ci_result_passed_all_true() {
        let result = CiResult {
            steps: vec![
                StepResult {
                    name: "a",
                    passed: true,
                    duration: Duration::from_secs(1),
                    stderr: String::new(),
                },
                StepResult {
                    name: "b",
                    passed: true,
                    duration: Duration::from_secs(2),
                    stderr: String::new(),
                },
            ],
        };
        assert!(result.passed());
        assert_eq!(result.total_duration(), Duration::from_secs(3));
    }

    #[test]
    fn ci_result_fails_if_any_step_fails() {
        let result = CiResult {
            steps: vec![
                StepResult {
                    name: "a",
                    passed: true,
                    duration: Duration::from_secs(1),
                    stderr: String::new(),
                },
                StepResult {
                    name: "b",
                    passed: false,
                    duration: Duration::from_secs(2),
                    stderr: "error".into(),
                },
            ],
        };
        assert!(!result.passed());
    }

    #[test]
    fn ci_result_display_format() {
        let result = CiResult {
            steps: vec![
                StepResult {
                    name: "cargo check",
                    passed: true,
                    duration: Duration::from_millis(1500),
                    stderr: String::new(),
                },
                StepResult {
                    name: "cargo test",
                    passed: false,
                    duration: Duration::from_millis(3200),
                    stderr: "test failed".into(),
                },
            ],
        };
        let display = format!("{result}");
        assert!(display.contains("PASS"));
        assert!(display.contains("FAIL"));
        assert!(display.contains("cargo check"));
        assert!(display.contains("cargo test"));
        assert!(display.contains("CI FAILED"));
    }

    #[test]
    fn ci_result_display_passed() {
        let result = CiResult {
            steps: vec![StepResult {
                name: "cargo check",
                passed: true,
                duration: Duration::from_millis(500),
                stderr: String::new(),
            }],
        };
        let display = format!("{result}");
        assert!(display.contains("CI passed"));
    }

    #[test]
    fn empty_steps_is_pass() {
        let result = CiResult { steps: vec![] };
        assert!(result.passed());
        assert_eq!(result.total_duration(), Duration::ZERO);
    }

    #[test]
    fn fail_fast_stops_after_first_failure() {
        // Use a nonexistent working dir to force failure
        let config = CiConfig {
            check: true,
            test: true,
            clippy: true,
            working_dir: Some("/nonexistent/path/that/does/not/exist".into()),
            fail_fast: true,
        };
        let result = run(&config);
        // With fail_fast, should stop after check fails
        assert_eq!(result.steps.len(), 1);
        assert!(!result.steps[0].passed);
    }

    #[test]
    fn no_fail_fast_runs_all_steps() {
        let config = CiConfig {
            check: true,
            test: true,
            clippy: true,
            working_dir: Some("/nonexistent/path/that/does/not/exist".into()),
            fail_fast: false,
        };
        let result = run(&config);
        // Without fail_fast, all 3 steps should run
        assert_eq!(result.steps.len(), 3);
        assert!(result.steps.iter().all(|s| !s.passed));
    }

    #[test]
    fn selective_steps() {
        let config = CiConfig {
            check: true,
            test: false,
            clippy: false,
            working_dir: Some("/nonexistent".into()),
            fail_fast: false,
        };
        let result = run(&config);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].name, "cargo check");
    }
}
