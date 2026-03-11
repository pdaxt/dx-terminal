//! UI Design System Audit Engine
//!
//! Static analysis of HTML/CSS files for design system violations.
//! Checks: raw colors, off-scale fonts, non-standard radius, missing transitions,
//! contrast failures, component pattern compliance.

use crate::design_tokens::{self, parse_hex_color, FONT_SCALE};
use regex::Regex;
use serde_json::{json, Value};

// ========== Violation Types ==========

#[derive(Debug, Clone)]
struct Violation {
    severity: &'static str,
    rule: &'static str,
    message: String,
    line: usize,
    snippet: String,
    suggestion: String,
}

impl Violation {
    fn to_json(&self) -> Value {
        json!({
            "severity": self.severity,
            "rule": self.rule,
            "message": self.message,
            "line": self.line,
            "snippet": self.snippet,
            "suggestion": self.suggestion,
        })
    }
}

// ========== Audit Engine ==========

/// Run full UI audit on an HTML file by path
pub fn audit_ui_file(path: &str) -> Value {
    match std::fs::read_to_string(path) {
        Ok(html) => audit_ui_html(&html, path),
        Err(e) => json!({"error": format!("Failed to read {}: {}", path, e)}),
    }
}

/// Run full UI audit on HTML string
pub fn audit_ui_html(html: &str, file_name: &str) -> Value {
    let tokens = design_tokens::parse_tokens_from_html(html);
    let root_block = find_root_range(html);

    let mut violations = Vec::new();

    // Rule checks
    violations.extend(check_raw_colors(html, &root_block));
    violations.extend(check_font_sizes(html, &root_block));
    violations.extend(check_font_families(html, &root_block));
    violations.extend(check_border_radius(html, &root_block));
    violations.extend(check_transitions(html, &root_block));
    violations.extend(check_light_theme_leaks(html));

    // Contrast check from tokens
    let contrast_results = design_tokens::check_all_contrasts();

    // Scoring — weight by severity
    let error_count = violations.iter().filter(|v| v.severity == "error").count();
    let warning_count = violations
        .iter()
        .filter(|v| v.severity == "warning")
        .count();
    let info_count = violations.iter().filter(|v| v.severity == "info").count();

    let rule_categories: Vec<&str> = vec![
        "raw-hex-color",
        "off-scale-font-size",
        "raw-font-family",
        "non-standard-radius",
        "hardcoded-transition",
        "light-theme-leak",
    ];

    let mut category_results = Vec::new();
    for cat in &rule_categories {
        let count = violations.iter().filter(|v| v.rule == *cat).count();
        category_results.push(json!({
            "check": cat,
            "passed": count == 0,
            "violations": count,
        }));
    }

    // Contrast pass
    let contrast_pass = contrast_results
        .get("all_pass")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let contrast_failures = contrast_results
        .get("failures")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Score: start at 100, deduct for violations
    // errors: -10 each, warnings: -3 each, info: -1 each, contrast fails: -5 each
    let deductions = (error_count as f64 * 10.0)
        + (warning_count as f64 * 3.0)
        + (info_count as f64 * 1.0)
        + (contrast_failures as f64 * 5.0);
    let score = (100.0 - deductions).max(0.0);

    let total_checks = rule_categories.len() + 1; // +1 for contrast
    let passed = category_results
        .iter()
        .filter(|c| c["passed"].as_bool().unwrap_or(false))
        .count()
        + if contrast_pass { 1 } else { 0 };
    let grade = match score as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    json!({
        "file": file_name,
        "violations": violations.iter().map(|v| v.to_json()).collect::<Vec<_>>(),
        "violation_count": violations.len(),
        "checks": category_results,
        "contrast": contrast_results,
        "score": (score * 10.0).round() / 10.0,
        "grade": grade,
        "passed": passed,
        "total": total_checks,
        "verdict": if score >= 80.0 { "COMPLIANT" } else { "NON-COMPLIANT" },
        "token_count": tokens.raw.len(),
    })
}

// ========== Rule Implementations ==========

/// Find the line range of :root { } block to exclude it from checks
fn find_root_range(html: &str) -> (usize, usize) {
    let lines: Vec<&str> = html.lines().collect();
    let mut start = 0;
    let mut end = 0;
    let mut in_root = false;
    let mut depth = 0;

    for (i, line) in lines.iter().enumerate() {
        if line.contains(":root") {
            start = i;
            in_root = true;
        }
        if in_root {
            depth += line.matches('{').count();
            depth -= line.matches('}').count();
            if depth == 0 {
                end = i;
                break;
            }
        }
    }
    (start, end)
}

/// Check for raw hex colors outside :root
fn check_raw_colors(html: &str, root_range: &(usize, usize)) -> Vec<Violation> {
    let mut violations = Vec::new();
    let re = Regex::new(r"#([0-9a-fA-F]{3,8})\b").unwrap();

    let lines: Vec<&str> = html.lines().collect();
    let in_style = find_style_range(&lines);

    for (i, line) in lines.iter().enumerate() {
        // Skip :root block
        if i >= root_range.0 && i <= root_range.1 {
            continue;
        }
        // Only check CSS lines (inside <style>)
        if i < in_style.0 || i > in_style.1 {
            continue;
        }

        // Skip SVG data URIs
        if line.contains("data:image") {
            continue;
        }

        for cap in re.captures_iter(line) {
            let hex = &cap[0];
            // Skip if it's inside an rgba() — the hex is the color, rgba is fine
            let pos = cap.get(0).unwrap().start();
            let prefix = &line[..pos];
            if prefix.ends_with("rgba(") || prefix.ends_with("rgb(") {
                continue;
            }

            // Skip common acceptable patterns: #000 for text-on-bright, #fff for icon fills
            let hex_lower = hex.to_lowercase();
            if hex_lower == "#000"
                || hex_lower == "#000000"
                || hex_lower == "#fff"
                || hex_lower == "#ffffff"
            {
                continue;
            }

            // Skip if inside color-mix(), mix-blend, or gradient context with a CSS var
            if line.contains("color-mix(") || line.contains("var(--") {
                continue;
            }

            // Suggest the matching token
            let suggestion = suggest_color_token(hex);

            violations.push(Violation {
                severity: "warning",
                rule: "raw-hex-color",
                message: format!("Raw hex color {} used instead of CSS variable", hex),
                line: i + 1,
                snippet: line.trim().to_string(),
                suggestion,
            });
        }
    }

    violations
}

/// Suggest the closest CSS variable for a hex color
fn suggest_color_token(hex: &str) -> String {
    let known = [
        ("#0a0e14", "var(--bg)"),
        ("#111820", "var(--surface)"),
        ("#161d27", "var(--surface2)"),
        ("#1e2a3a", "var(--border)"),
        ("#2d4a6a", "var(--border-bright)"),
        ("#d4dce8", "var(--text)"),
        ("#6b7d95", "var(--muted)"),
        ("#3d4f63", "var(--dim)"),
        ("#4da6ff", "var(--blue)"),
        ("#44d98c", "var(--green)"),
        ("#f0c040", "var(--yellow)"),
        ("#ff5c5c", "var(--red)"),
        ("#b07aff", "var(--purple)"),
        ("#3dd6d0", "var(--teal)"),
        ("#ff9f43", "var(--orange)"),
        ("#ff6b9d", "var(--pink)"),
    ];

    let hex_lower = hex.to_lowercase();
    for (val, var) in &known {
        if hex_lower == *val {
            return format!("Use {} instead", var);
        }
    }

    // Try to find closest by RGB distance
    if let Some(target) = parse_hex_color(hex) {
        let mut closest = ("var(--blue)", u32::MAX);
        for (val, var) in &known {
            if let Some(c) = parse_hex_color(val) {
                let dist = ((target.0 as i32 - c.0 as i32).pow(2)
                    + (target.1 as i32 - c.1 as i32).pow(2)
                    + (target.2 as i32 - c.2 as i32).pow(2)) as u32;
                if dist < closest.1 {
                    closest = (var, dist);
                }
            }
        }
        if closest.1 < 5000 {
            return format!("Consider {} (close match)", closest.0);
        }
    }

    "Define a new CSS variable in :root".to_string()
}

/// Check for off-scale font-size values
fn check_font_sizes(html: &str, root_range: &(usize, usize)) -> Vec<Violation> {
    let mut violations = Vec::new();
    let re = Regex::new(r"font-size\s*:\s*(\d+(?:\.\d+)?)\s*px").unwrap();

    let lines: Vec<&str> = html.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i >= root_range.0 && i <= root_range.1 {
            continue;
        }

        for cap in re.captures_iter(line) {
            let size: f32 = cap[1].parse().unwrap_or(0.0);
            if !FONT_SCALE.contains(&size) {
                let nearest = FONT_SCALE
                    .iter()
                    .min_by(|a, b| {
                        ((**a - size).abs())
                            .partial_cmp(&((**b - size).abs()))
                            .unwrap()
                    })
                    .unwrap_or(&10.0);
                violations.push(Violation {
                    severity: "warning",
                    rule: "off-scale-font-size",
                    message: format!("Font size {}px is not in the design scale", size),
                    line: i + 1,
                    snippet: line.trim().to_string(),
                    suggestion: format!("Use {}px instead (nearest in scale)", nearest),
                });
            }
        }
    }

    violations
}

/// Check for raw font-family declarations
fn check_font_families(html: &str, root_range: &(usize, usize)) -> Vec<Violation> {
    let mut violations = Vec::new();
    let re = Regex::new(r"font-family\s*:\s*([^;]+);").unwrap();

    let lines: Vec<&str> = html.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i >= root_range.0 && i <= root_range.1 {
            continue;
        }

        for cap in re.captures_iter(line) {
            let family = cap[1].trim();
            if !family.contains("var(--mono)") && !family.contains("var(--sans)") {
                let suggest = if family.contains("Mono")
                    || family.contains("monospace")
                    || family.contains("Consolas")
                {
                    "var(--mono)"
                } else {
                    "var(--sans)"
                };
                violations.push(Violation {
                    severity: "warning",
                    rule: "raw-font-family",
                    message: format!("Raw font-family declaration instead of CSS variable"),
                    line: i + 1,
                    snippet: line.trim().to_string(),
                    suggestion: format!("Use {} instead", suggest),
                });
            }
        }
    }

    violations
}

/// Check for non-standard border-radius values
fn check_border_radius(html: &str, root_range: &(usize, usize)) -> Vec<Violation> {
    let mut violations = Vec::new();
    let re = Regex::new(r"border-radius\s*:\s*([^;]+);").unwrap();

    let allowed_literals = [
        "50%", "20px", "4px", "3px", "2px", "0", "10px", "6px", "7px", "8px", "12px",
    ];

    let lines: Vec<&str> = html.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i >= root_range.0 && i <= root_range.1 {
            continue;
        }

        for cap in re.captures_iter(line) {
            let val = cap[1].trim();
            if val.contains("var(--radius") {
                continue;
            }
            // Check if it's a known allowed literal
            let parts: Vec<&str> = val.split_whitespace().collect();
            let all_allowed = parts.iter().all(|p| allowed_literals.contains(p));
            if !all_allowed {
                violations.push(Violation {
                    severity: "info",
                    rule: "non-standard-radius",
                    message: format!("Border radius '{}' doesn't use design token", val),
                    line: i + 1,
                    snippet: line.trim().to_string(),
                    suggestion: "Use var(--radius) for 10px or var(--radius-sm) for 6px"
                        .to_string(),
                });
            }
        }
    }

    violations
}

/// Check for hardcoded transitions not using var(--transition)
fn check_transitions(html: &str, root_range: &(usize, usize)) -> Vec<Violation> {
    let mut violations = Vec::new();
    let re = Regex::new(r"transition\s*:\s*([^;]+);").unwrap();

    let lines: Vec<&str> = html.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i >= root_range.0 && i <= root_range.1 {
            continue;
        }

        for cap in re.captures_iter(line) {
            let val = cap[1].trim();
            if !val.contains("var(--transition") {
                // Allow specific transition properties (width, height for animations)
                if val.contains("width") || val.contains("height") || val.contains("opacity") {
                    continue;
                }
                violations.push(Violation {
                    severity: "info",
                    rule: "hardcoded-transition",
                    message: "Transition doesn't use var(--transition)".to_string(),
                    line: i + 1,
                    snippet: line.trim().to_string(),
                    suggestion: "Use transition:var(--transition) for consistency".to_string(),
                });
            }
        }
    }

    violations
}

/// Check for light theme leaks (white/light backgrounds, dark text)
fn check_light_theme_leaks(html: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let light_patterns = [
        ("background:#fff", "Light background"),
        ("background:white", "Light background"),
        ("background:#ffffff", "Light background"),
        ("color:black", "Dark text on dark theme"),
        ("color:#333", "Dark text on dark theme"),
    ];
    // Note: color:#000 is intentional on bright-background badges (priority pills)
    // so we don't flag it as a leak

    let lines: Vec<&str> = html.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase().replace(' ', "");
        for (pattern, msg) in &light_patterns {
            if lower.contains(pattern) {
                violations.push(Violation {
                    severity: "error",
                    rule: "light-theme-leak",
                    message: msg.to_string(),
                    line: i + 1,
                    snippet: line.trim().to_string(),
                    suggestion:
                        "Use var(--surface) or var(--bg) for backgrounds, var(--text) for text"
                            .to_string(),
                });
            }
        }
    }

    violations
}

/// Find the <style> block line range
fn find_style_range(lines: &[&str]) -> (usize, usize) {
    let mut start = 0;
    let mut end = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.contains("<style") {
            start = i;
        }
        if line.contains("</style") {
            end = i;
            break;
        }
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_embedded_dashboard() {
        let html = include_str!("../assets/dashboard.html");
        let result = audit_ui_html(html, "dashboard.html");
        assert!(result.get("violations").is_some());
        assert!(result.get("score").is_some());
        assert!(result.get("grade").is_some());
        // Dashboard is the reference — should be mostly compliant
        let score = result["score"].as_f64().unwrap();
        eprintln!("UI Audit Score: {}", score);
        eprintln!("Violations: {}", result["violation_count"]);
        if let Some(violations) = result["violations"].as_array() {
            for v in violations.iter().take(10) {
                eprintln!(
                    "  {} [{}] line {}: {}",
                    v["severity"], v["rule"], v["line"], v["message"]
                );
            }
        }
        assert!(
            score >= 40.0,
            "Dashboard itself should score >= 40, got {}",
            score
        );
    }

    #[test]
    fn test_detects_raw_hex() {
        let css = r#"<style>
:root{--blue:#4da6ff;}
.bad{color:#ff0000}
</style>"#;
        let violations = check_raw_colors(css, &(1, 1));
        assert!(
            violations.iter().any(|v| v.rule == "raw-hex-color"),
            "Should detect raw hex outside :root"
        );
    }

    #[test]
    fn test_detects_off_scale_font() {
        let css = r#"<style>
:root{}
.bad{font-size:14px}
</style>"#;
        let violations = check_font_sizes(css, &(1, 1));
        assert!(
            violations.iter().any(|v| v.rule == "off-scale-font-size"),
            "14px is not in the scale"
        );
    }
}
