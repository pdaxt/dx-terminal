use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::scanner;

// ========== Finding Types ==========

#[derive(Debug, Clone)]
struct Finding {
    severity: &'static str, // "critical", "high", "medium", "low", "info"
    category: &'static str,
    file: String,
    line: usize,
    message: String,
}

impl Finding {
    fn to_json(&self) -> Value {
        json!({
            "severity": self.severity,
            "category": self.category,
            "file": self.file,
            "line": self.line,
            "message": self.message,
        })
    }
}

// ========== Public API ==========

/// Audit code for fragmentation, dead code, loose ends
pub fn audit_code(project_path: &str) -> Value {
    let path = resolve_path(project_path);
    if !path.exists() {
        return json!({"error": format!("Path not found: {}", path.display())});
    }

    let mut findings = Vec::new();
    findings.extend(find_loose_ends(&path));
    findings.extend(find_dead_code_markers(&path));
    findings.extend(find_fragmented_files(&path));
    findings.extend(find_empty_impls(&path));

    let summary = summarize(&findings);
    json!({
        "project": project_path,
        "audit_type": "code",
        "findings": findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        "summary": summary,
    })
}

/// Audit for security vulnerabilities
pub fn audit_security(project_path: &str) -> Value {
    let path = resolve_path(project_path);
    if !path.exists() {
        return json!({"error": format!("Path not found: {}", path.display())});
    }

    let mut findings = Vec::new();
    findings.extend(find_hardcoded_secrets(&path));
    findings.extend(find_unsafe_code(&path));
    findings.extend(find_command_injection(&path));
    findings.extend(find_path_traversal(&path));

    // Try cargo audit if Cargo.lock exists
    if path.join("Cargo.lock").exists() {
        findings.extend(run_cargo_audit(&path));
    }

    let summary = summarize(&findings);
    json!({
        "project": project_path,
        "audit_type": "security",
        "findings": findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        "summary": summary,
    })
}

/// Verify code intent — check declared modules, test coverage, stub functions
pub fn audit_intent(project_path: &str, description: &str) -> Value {
    let path = resolve_path(project_path);
    if !path.exists() {
        return json!({"error": format!("Path not found: {}", path.display())});
    }

    let mut findings = Vec::new();
    findings.extend(find_stub_functions(&path));
    findings.extend(find_untested_modules(&path));
    findings.extend(check_module_declarations(&path));

    // Check README vs code
    if !description.is_empty() {
        findings.extend(check_readme_claims(&path, description));
    }

    let summary = summarize(&findings);
    json!({
        "project": project_path,
        "audit_type": "intent",
        "description": description,
        "findings": findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        "summary": summary,
    })
}

/// Audit dependencies for health issues
pub fn audit_deps(project_path: &str) -> Value {
    let path = resolve_path(project_path);
    if !path.exists() {
        return json!({"error": format!("Path not found: {}", path.display())});
    }

    let mut findings = Vec::new();
    findings.extend(analyze_cargo_deps(&path));
    findings.extend(analyze_node_deps(&path));
    findings.extend(find_duplicate_deps(&path));

    let summary = summarize(&findings);
    json!({
        "project": project_path,
        "audit_type": "deps",
        "findings": findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        "summary": summary,
    })
}

/// Full audit — runs all checks, stores result, returns aggregate
pub fn audit_full(project_path: &str) -> Value {
    let code = audit_code(project_path);
    let security = audit_security(project_path);
    let intent = audit_intent(project_path, "");
    let deps = audit_deps(project_path);

    let all_findings: Vec<Value> = [&code, &security, &intent, &deps]
        .iter()
        .flat_map(|r| {
            r.get("findings")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .collect();

    let critical = all_findings
        .iter()
        .filter(|f| f.get("severity").and_then(|s| s.as_str()) == Some("critical"))
        .count();
    let high = all_findings
        .iter()
        .filter(|f| f.get("severity").and_then(|s| s.as_str()) == Some("high"))
        .count();
    let medium = all_findings
        .iter()
        .filter(|f| f.get("severity").and_then(|s| s.as_str()) == Some("medium"))
        .count();
    let low = all_findings
        .iter()
        .filter(|f| f.get("severity").and_then(|s| s.as_str()) == Some("low"))
        .count();
    let info = all_findings
        .iter()
        .filter(|f| f.get("severity").and_then(|s| s.as_str()) == Some("info"))
        .count();

    let grade = if critical > 0 {
        "F"
    } else if high > 2 {
        "D"
    } else if high > 0 || medium > 5 {
        "C"
    } else if medium > 0 || low > 5 {
        "B"
    } else {
        "A"
    };

    let result = json!({
        "project": project_path,
        "audit_type": "full",
        "grade": grade,
        "total_findings": all_findings.len(),
        "by_severity": {
            "critical": critical,
            "high": high,
            "medium": medium,
            "low": low,
            "info": info,
        },
        "code": code,
        "security": security,
        "intent": intent,
        "deps": deps,
    });

    store_audit(project_path, &result);
    result
}

// ========== Code Audit Helpers ==========

fn find_loose_ends(root: &Path) -> Vec<Finding> {
    let patterns = [
        (r"//.*\bTODO\b", "loose_end", "medium", "TODO found"),
        (r"//.*\bFIXME\b", "loose_end", "high", "FIXME found"),
        (r"//.*\bHACK\b", "loose_end", "medium", "HACK found"),
        (r"//.*\bXXX\b", "loose_end", "medium", "XXX marker found"),
        (r"//.*\bTEMP\b", "loose_end", "low", "TEMP marker found"),
        (
            r"\bunimplemented!\b",
            "incomplete",
            "high",
            "unimplemented!() macro — code path not finished",
        ),
        (
            r"\btodo!\b",
            "incomplete",
            "high",
            "todo!() macro — code path not finished",
        ),
        (
            r#"panic!\("not implemented"\)"#,
            "incomplete",
            "high",
            "panic with 'not implemented'",
        ),
    ];
    scan_source_files(root, &patterns)
}

fn find_dead_code_markers(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r"#\[allow\(dead_code\)\]",
            "dead_code",
            "low",
            "Explicitly suppressed dead_code warning",
        ),
        (
            r"#\[allow\(unused\)\]",
            "dead_code",
            "low",
            "Explicitly suppressed unused warning",
        ),
        (
            r"//.*removed|//.*deprecated|//.*legacy",
            "dead_code",
            "info",
            "Comment suggesting removed/deprecated code",
        ),
    ];
    scan_source_files(root, &patterns)
}

fn find_fragmented_files(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entry in walk_source_files(root) {
        if let Ok(content) = fs::read_to_string(&entry) {
            let real_lines = content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty()
                        && !t.starts_with("//")
                        && !t.starts_with("/*")
                        && !t.starts_with('*')
                })
                .count();
            if real_lines < 5 && real_lines > 0 {
                let rel = entry.strip_prefix(root).unwrap_or(&entry);
                findings.push(Finding {
                    severity: "info",
                    category: "fragmentation",
                    file: rel.display().to_string(),
                    line: 0,
                    message: format!(
                        "File has only {} lines of code — consider merging",
                        real_lines
                    ),
                });
            }
        }
    }
    findings
}

fn find_empty_impls(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r"\{\s*\}",
            "incomplete",
            "info",
            "Empty block — possible stub",
        ),
        (
            r"_ =>\s*\{\s*\}",
            "incomplete",
            "low",
            "Catch-all match arm with empty body",
        ),
    ];
    scan_source_files(root, &patterns)
}

// ========== Security Audit Helpers ==========

fn find_hardcoded_secrets(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r#"(?i)(api[_-]?key|secret|password|token|auth_token)\s*[:=]\s*["'][^"']{8,}"#,
            "secret",
            "critical",
            "Possible hardcoded secret",
        ),
        (
            r"AKIA[A-Z0-9]{16}",
            "secret",
            "critical",
            "AWS access key ID pattern",
        ),
        (
            r"sk-[a-zA-Z0-9]{20,}",
            "secret",
            "critical",
            "API key pattern (OpenAI/Anthropic)",
        ),
        (
            r"ghp_[a-zA-Z0-9]{36}",
            "secret",
            "critical",
            "GitHub personal access token",
        ),
        (
            r"xox[bpors]-[a-zA-Z0-9-]+",
            "secret",
            "critical",
            "Slack token pattern",
        ),
    ];
    // Filter: skip test files, comments, and .lock files
    scan_source_files(root, &patterns)
}

fn find_unsafe_code(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r"\bunsafe\s*\{",
            "unsafe",
            "medium",
            "Unsafe block — requires manual review",
        ),
        (r"\*mut\s+\w+", "unsafe", "medium", "Raw mutable pointer"),
        (r"\*const\s+\w+", "unsafe", "low", "Raw const pointer"),
    ];
    scan_source_files(root, &patterns)
}

fn find_command_injection(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r"Command::new\(.*format!",
            "injection",
            "high",
            "Command::new with format! — possible injection vector",
        ),
        (
            r"\.arg\(.*format!",
            "injection",
            "medium",
            "Command arg with format! — check input sanitization",
        ),
        (
            r#"shell\s*=\s*true|/bin/sh|/bin/bash"#,
            "injection",
            "high",
            "Shell execution — review for injection",
        ),
    ];
    scan_source_files(root, &patterns)
}

fn find_path_traversal(root: &Path) -> Vec<Finding> {
    let patterns = [
        (
            r"\.join\(.*req\.",
            "path_traversal",
            "medium",
            "Path join with request data — check for traversal",
        ),
        (
            r#"\.\./|\.\.\\|%2e%2e"#,
            "path_traversal",
            "high",
            "Path traversal pattern in source",
        ),
    ];
    scan_source_files(root, &patterns)
}

fn run_cargo_audit(root: &Path) -> Vec<Finding> {
    let output = std::process::Command::new("cargo")
        .arg("audit")
        .arg("--json")
        .current_dir(root)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(v) = serde_json::from_str::<Value>(&stdout) {
                let mut findings = Vec::new();
                if let Some(vulns) = v
                    .get("vulnerabilities")
                    .and_then(|v| v.get("list"))
                    .and_then(|l| l.as_array())
                {
                    for vuln in vulns {
                        let empty = json!({});
                        let advisory = vuln.get("advisory").unwrap_or(&empty);
                        let pkg = vuln
                            .get("package")
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("?");
                        let id = advisory.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                        let title = advisory
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("?");
                        findings.push(Finding {
                            severity: "high",
                            category: "cve",
                            file: format!("Cargo.lock ({})", pkg),
                            line: 0,
                            message: format!("{}: {}", id, title),
                        });
                    }
                }
                return findings;
            }
            Vec::new()
        }
        Err(_) => {
            vec![Finding {
                severity: "info",
                category: "tooling",
                file: "Cargo.lock".into(),
                line: 0,
                message: "cargo-audit not installed — run `cargo install cargo-audit`".into(),
            }]
        }
    }
}

// ========== Intent Verification Helpers ==========

fn find_stub_functions(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entry in walk_source_files(root) {
        if let Ok(content) = fs::read_to_string(&entry) {
            let rel = entry
                .strip_prefix(root)
                .unwrap_or(&entry)
                .display()
                .to_string();
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                // Functions that just return default/empty/Ok(())
                if (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn "))
                    && !trimmed.contains("test")
                {
                    // Check if next non-empty line is just a closing brace or trivial return
                    let remaining: Vec<&str> = content
                        .lines()
                        .skip(i + 1)
                        .take(3)
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .collect();
                    if remaining.len() <= 2 {
                        let body = remaining.join(" ");
                        if body == "}"
                            || body == "Ok(()) }"
                            || body == "Default::default() }"
                            || body == "String::new() }"
                            || body == "Vec::new() }"
                        {
                            findings.push(Finding {
                                severity: "medium",
                                category: "stub",
                                file: rel.clone(),
                                line: i + 1,
                                message: format!(
                                    "Stub function: {}",
                                    trimmed.chars().take(60).collect::<String>()
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    findings
}

fn find_untested_modules(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let test_dir = root.join("tests");
    let has_test_dir = test_dir.exists();

    // For Rust: check if src/X.rs has corresponding tests
    let src_dir = root.join("src");
    if src_dir.exists() {
        for entry in walk_source_files(&src_dir) {
            if let Some(name) = entry.file_stem().and_then(|s| s.to_str()) {
                if name == "mod" || name == "lib" || name == "main" {
                    continue;
                }
                // Check for inline tests
                let content = fs::read_to_string(&entry).unwrap_or_default();
                let has_inline_tests =
                    content.contains("#[cfg(test)]") || content.contains("#[test]");
                // Check for integration test file
                let has_integration =
                    has_test_dir && test_dir.join(format!("{}.rs", name)).exists();

                if !has_inline_tests && !has_integration {
                    let rel = entry.strip_prefix(root).unwrap_or(&entry);
                    findings.push(Finding {
                        severity: "low",
                        category: "test_coverage",
                        file: rel.display().to_string(),
                        line: 0,
                        message: format!("Module '{}' has no tests (inline or integration)", name),
                    });
                }
            }
        }
    }
    findings
}

fn check_module_declarations(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    // Check lib.rs/main.rs for declared modules
    for entry_name in &["src/lib.rs", "src/main.rs"] {
        let entry = root.join(entry_name);
        if !entry.exists() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&entry) {
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if let Some(mod_name) = trimmed
                    .strip_prefix("pub mod ")
                    .or_else(|| trimmed.strip_prefix("mod "))
                {
                    let mod_name = mod_name.trim_end_matches(';');
                    let mod_file = root.join("src").join(format!("{}.rs", mod_name));
                    let mod_dir = root.join("src").join(mod_name).join("mod.rs");
                    if !mod_file.exists() && !mod_dir.exists() {
                        findings.push(Finding {
                            severity: "high",
                            category: "missing_module",
                            file: entry_name.to_string(),
                            line: i + 1,
                            message: format!(
                                "Declared module '{}' has no corresponding file",
                                mod_name
                            ),
                        });
                    }
                }
            }
        }
    }
    findings
}

fn check_readme_claims(root: &Path, description: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let readme = root.join("README.md");
    if readme.exists() {
        if let Ok(content) = fs::read_to_string(&readme) {
            if content.trim().len() < 50 {
                findings.push(Finding {
                    severity: "low",
                    category: "docs",
                    file: "README.md".into(),
                    line: 0,
                    message: "README is nearly empty".into(),
                });
            }
        }
    } else {
        findings.push(Finding {
            severity: "low",
            category: "docs",
            file: "README.md".into(),
            line: 0,
            message: "No README.md found".into(),
        });
    }

    // Check if description keywords map to actual source files
    let keywords: Vec<&str> = description
        .split_whitespace()
        .filter(|w| w.len() > 4)
        .collect();
    let src = root.join("src");
    if src.exists() {
        let source_content = walk_source_files(&src)
            .filter_map(|f| fs::read_to_string(&f).ok())
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        for kw in keywords {
            let kw_lower = kw.to_lowercase();
            if !source_content.contains(&kw_lower) {
                findings.push(Finding {
                    severity: "info",
                    category: "intent_mismatch",
                    file: "".into(),
                    line: 0,
                    message: format!("Description keyword '{}' not found in source code", kw),
                });
            }
        }
    }

    findings
}

// ========== Dependency Helpers ==========

fn analyze_cargo_deps(root: &Path) -> Vec<Finding> {
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Vec::new();
    }

    let mut findings = Vec::new();
    if let Ok(content) = fs::read_to_string(&cargo_toml) {
        // Count dependencies
        let dep_count = content
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with('#')
                    && !t.starts_with('[')
                    && t.contains('=')
                    && !t.starts_with("name")
                    && !t.starts_with("version")
                    && !t.starts_with("edition")
                    && !t.starts_with("authors")
            })
            .count();

        if dep_count > 30 {
            findings.push(Finding {
                severity: "medium",
                category: "deps",
                file: "Cargo.toml".into(),
                line: 0,
                message: format!(
                    "{} dependencies — consider reducing to minimize attack surface",
                    dep_count
                ),
            });
        }

        // Check for wildcard versions
        for (i, line) in content.lines().enumerate() {
            if line.contains("= \"*\"") {
                findings.push(Finding {
                    severity: "high",
                    category: "deps",
                    file: "Cargo.toml".into(),
                    line: i + 1,
                    message: format!("Wildcard dependency version: {}", line.trim()),
                });
            }
        }
    }
    findings
}

fn analyze_node_deps(root: &Path) -> Vec<Finding> {
    let pkg_json = root.join("package.json");
    if !pkg_json.exists() {
        return Vec::new();
    }

    let mut findings = Vec::new();
    if let Ok(content) = fs::read_to_string(&pkg_json) {
        if let Ok(v) = serde_json::from_str::<Value>(&content) {
            let deps = v
                .get("dependencies")
                .and_then(|d| d.as_object())
                .map(|d| d.len())
                .unwrap_or(0);
            let dev_deps = v
                .get("devDependencies")
                .and_then(|d| d.as_object())
                .map(|d| d.len())
                .unwrap_or(0);
            if deps + dev_deps > 50 {
                findings.push(Finding {
                    severity: "medium",
                    category: "deps",
                    file: "package.json".into(),
                    line: 0,
                    message: format!(
                        "{} total dependencies — review for unused packages",
                        deps + dev_deps
                    ),
                });
            }
        }
    }
    findings
}

fn find_duplicate_deps(root: &Path) -> Vec<Finding> {
    // Try cargo tree --duplicates
    if !root.join("Cargo.lock").exists() {
        return Vec::new();
    }

    let output = std::process::Command::new("cargo")
        .args(["tree", "--duplicates", "--depth", "1"])
        .current_dir(root)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let dupes: Vec<&str> = stdout
                .lines()
                .filter(|l| !l.starts_with(' ') && !l.is_empty())
                .collect();
            if dupes.len() > 1 {
                vec![Finding {
                    severity: "low",
                    category: "deps",
                    file: "Cargo.lock".into(),
                    line: 0,
                    message: format!(
                        "{} duplicate dependencies: {}",
                        dupes.len(),
                        dupes.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                    ),
                }]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

// ========== Storage ==========

fn store_audit(project: &str, result: &Value) {
    // Extract just the project name for storage (avoid absolute paths)
    let name = Path::new(project)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project.replace('/', "_"));
    let dir = config::dx_root().join("audits").join(&name);
    let _ = fs::create_dir_all(&dir);
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let path = dir.join(format!("{}.json", ts));
    let _ = fs::write(
        &path,
        serde_json::to_string_pretty(result).unwrap_or_default(),
    );

    // Also write "latest.json" symlink-like file
    let latest = dir.join("latest.json");
    let _ = fs::write(
        &latest,
        serde_json::to_string_pretty(result).unwrap_or_default(),
    );
}

/// Load the latest audit result for a project by name.
pub fn load_latest_audit(project_name: &str) -> Option<Value> {
    let dir = config::dx_root().join("audits").join(project_name);
    let latest = dir.join("latest.json");
    if !latest.exists() {
        return None;
    }
    let content = fs::read_to_string(&latest).ok()?;
    serde_json::from_str(&content).ok()
}

/// List all projects that have stored audit results.
pub fn list_audited_projects() -> Vec<String> {
    let audits_dir = config::dx_root().join("audits");
    if !audits_dir.exists() {
        return Vec::new();
    }
    let mut projects = Vec::new();
    if let Ok(entries) = fs::read_dir(&audits_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && entry.path().join("latest.json").exists() {
                if let Some(name) = entry.file_name().to_str() {
                    projects.push(name.to_string());
                }
            }
        }
    }
    projects.sort();
    projects
}

// ========== Core Scanning Engine ==========

fn scan_source_files(
    root: &Path,
    patterns: &[(&str, &'static str, &'static str, &'static str)],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let compiled: Vec<(Regex, &'static str, &'static str, &'static str)> = patterns
        .iter()
        .filter_map(|(pat, cat, sev, msg)| Regex::new(pat).ok().map(|re| (re, *cat, *sev, *msg)))
        .collect();

    for entry in walk_source_files(root) {
        // Skip test files for secret scanning
        let rel = entry.strip_prefix(root).unwrap_or(&entry);
        let rel_str = rel.display().to_string();
        if rel_str.contains("target/") || rel_str.contains("node_modules/") {
            continue;
        }
        // Skip audit module itself to avoid self-matching regex patterns
        if rel_str.contains("audit.rs") || rel_str.ends_with("audit/mod.rs") {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&entry) {
            for (i, line) in content.lines().enumerate() {
                // Skip comment-only lines for some checks
                let trimmed = line.trim();
                if trimmed.starts_with("//")
                    && !trimmed.contains("TODO")
                    && !trimmed.contains("FIXME")
                    && !trimmed.contains("HACK")
                    && !trimmed.contains("XXX")
                {
                    continue;
                }
                // Skip regex pattern definitions (avoid self-matching)
                if trimmed.contains("r#\"") || trimmed.contains("r\"") && trimmed.contains("Regex")
                {
                    continue;
                }
                // Skip compile-time includes (not runtime path traversal)
                if trimmed.contains("include_str!") || trimmed.contains("include_bytes!") {
                    continue;
                }
                // Skip string literals / descriptions mentioning keywords
                if trimmed.contains("description")
                    && (trimmed.contains("\"") || trimmed.contains("'"))
                {
                    continue;
                }
                for (re, cat, sev, msg) in &compiled {
                    if re.is_match(line) {
                        findings.push(Finding {
                            severity: sev,
                            category: cat,
                            file: rel_str.clone(),
                            line: i + 1,
                            message: format!(
                                "{}: {}",
                                msg,
                                trimmed.chars().take(80).collect::<String>()
                            ),
                        });
                    }
                }
            }
        }
    }
    findings
}

fn walk_source_files(root: &Path) -> Box<dyn Iterator<Item = PathBuf>> {
    let extensions = ["rs", "ts", "tsx", "js", "jsx", "py", "go", "java"];
    let root = root.to_path_buf();
    Box::new(
        walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                // Skip build/vendor directories
                !matches!(
                    name.as_ref(),
                    "target" | "node_modules" | ".git" | "dist" | "build" | "__pycache__"
                )
            })
            .filter_map(|e| e.ok())
            .filter(move |e| {
                e.file_type().is_file()
                    && e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| extensions.contains(&ext))
                        .unwrap_or(false)
            })
            .map(|e| e.into_path()),
    )
}

fn resolve_path(project: &str) -> PathBuf {
    // If it's already an absolute path, use it
    let p = PathBuf::from(project);
    if p.is_absolute() && p.exists() {
        return p;
    }
    // Try scanner registry
    let reg = scanner::load_registry();
    if let Some(proj) = reg
        .projects
        .iter()
        .find(|pp| pp.name.to_lowercase() == project.to_lowercase())
    {
        return PathBuf::from(&proj.path);
    }
    // Fallback: ~/Projects/{name}
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join("Projects").join(project)
}

fn summarize(findings: &[Finding]) -> Value {
    let critical = findings.iter().filter(|f| f.severity == "critical").count();
    let high = findings.iter().filter(|f| f.severity == "high").count();
    let medium = findings.iter().filter(|f| f.severity == "medium").count();
    let low = findings.iter().filter(|f| f.severity == "low").count();
    let info = findings.iter().filter(|f| f.severity == "info").count();

    json!({
        "total": findings.len(),
        "critical": critical,
        "high": high,
        "medium": medium,
        "low": low,
        "info": info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_full_on_dx_terminal() {
        let result = audit_full(env!("CARGO_MANIFEST_DIR"));
        assert!(result.get("grade").is_some(), "Should return a grade");
        assert!(result.get("total_findings").is_some());
        // Print to stdout for manual review
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    fn test_audit_code_on_dx_terminal() {
        let result = audit_code(env!("CARGO_MANIFEST_DIR"));
        assert!(result.get("findings").is_some());
        let findings = result["findings"].as_array().unwrap();
        println!("Code audit: {} findings", findings.len());
        for f in findings.iter().take(10) {
            println!(
                "  [{:>8}] {} — {}:{}",
                f["severity"].as_str().unwrap_or("?"),
                f["message"].as_str().unwrap_or("?"),
                f["file"].as_str().unwrap_or("?"),
                f["line"]
            );
        }
    }

    #[test]
    fn test_audit_security_on_dx_terminal() {
        let result = audit_security(env!("CARGO_MANIFEST_DIR"));
        assert!(result.get("findings").is_some());
        let findings = result["findings"].as_array().unwrap();
        println!("Security audit: {} findings", findings.len());
        for f in findings.iter().take(10) {
            println!(
                "  [{:>8}] {} — {}:{}",
                f["severity"].as_str().unwrap_or("?"),
                f["message"].as_str().unwrap_or("?"),
                f["file"].as_str().unwrap_or("?"),
                f["line"]
            );
        }
    }

    #[test]
    fn test_resolve_path_absolute() {
        let p = resolve_path(env!("CARGO_MANIFEST_DIR"));
        assert!(p.exists());
    }
}
