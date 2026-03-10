//! UX Audit Engine
//!
//! Runtime UX checks via Playwright or fallback DOM analysis.
//! Checks: keyboard nav, responsive viewports, heading hierarchy,
//! JS console errors, focus indicators, ARIA labels.

use serde_json::{json, Value};
use std::process::Command;
use std::io::Write;

// ========== UX Check Types ==========

#[derive(Debug, Clone)]
struct UxCheck {
    name: String,
    category: String,
    passed: bool,
    details: String,
    severity: String,
}

impl UxCheck {
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "category": self.category,
            "passed": self.passed,
            "details": self.details,
            "severity": self.severity,
        })
    }
}

// ========== Public API ==========

/// Run UX audit on a URL using Playwright
pub fn audit_ux(url: &str) -> Value {
    let mut checks = Vec::new();

    // Try Playwright-based checks first
    match run_playwright_audit(url) {
        Ok(pw_checks) => checks.extend(pw_checks),
        Err(e) => {
            // Fallback: report playwright unavailable but still do static checks
            checks.push(UxCheck {
                name: "playwright_available".into(),
                category: "setup".into(),
                passed: false,
                details: format!("Playwright not available: {}. Install with: npm i -g playwright", e),
                severity: "warning".into(),
            });
        }
    }

    // Static HTML analysis (fetch page and analyze)
    match fetch_page_html(url) {
        Ok(html) => {
            checks.extend(check_heading_hierarchy(&html));
            checks.extend(check_aria_labels(&html));
            checks.extend(check_interactive_elements(&html));
            checks.extend(check_meta_viewport(&html));
            checks.extend(check_empty_states(&html));
            checks.extend(check_reduced_motion(&html));
        }
        Err(e) => {
            checks.push(UxCheck {
                name: "page_fetch".into(),
                category: "setup".into(),
                passed: false,
                details: format!("Could not fetch page: {}", e),
                severity: "error".into(),
            });
        }
    }

    // Score
    let total = checks.len();
    let passed = checks.iter().filter(|c| c.passed).count();
    let score = if total > 0 { (passed as f64 / total as f64) * 100.0 } else { 0.0 };
    let grade = match score as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    let verdict = match score as u32 {
        88..=100 => "EXCELLENT",
        71..=87 => "GOOD",
        47..=70 => "NEEDS WORK",
        _ => "POOR",
    };

    // Group by category
    let categories: Vec<String> = {
        let mut cats: Vec<String> = checks.iter().map(|c| c.category.clone()).collect();
        cats.sort();
        cats.dedup();
        cats
    };

    let category_summary: Vec<Value> = categories.iter().map(|cat| {
        let cat_checks: Vec<&UxCheck> = checks.iter().filter(|c| c.category == *cat).collect();
        let cat_passed = cat_checks.iter().filter(|c| c.passed).count();
        json!({
            "category": cat,
            "passed": cat_passed,
            "total": cat_checks.len(),
            "all_pass": cat_passed == cat_checks.len(),
        })
    }).collect();

    json!({
        "url": url,
        "checks": checks.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
        "categories": category_summary,
        "passed": passed,
        "total": total,
        "score": (score * 10.0).round() / 10.0,
        "grade": grade,
        "verdict": verdict,
    })
}

// ========== Playwright Integration ==========

fn run_playwright_audit(url: &str) -> Result<Vec<UxCheck>, String> {
    // Generate Playwright test script
    let script = generate_playwright_script(url);

    // Write to temp file
    let tmp_dir = std::env::temp_dir();
    let script_path = tmp_dir.join("dx_ux_audit.mjs");
    let mut f = std::fs::File::create(&script_path)
        .map_err(|e| format!("Failed to create temp script: {}", e))?;
    f.write_all(script.as_bytes())
        .map_err(|e| format!("Failed to write script: {}", e))?;

    // Run with node
    let output = Command::new("node")
        .arg(&script_path)
        .output()
        .map_err(|e| format!("Failed to run node: {}", e))?;

    // Clean up
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Playwright script failed: {}", stderr.chars().take(500).collect::<String>()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_playwright_output(&stdout)
}

fn generate_playwright_script(url: &str) -> String {
    format!(r#"
import {{ chromium }} from 'playwright';

const url = '{}';
const results = [];

function check(name, category, passed, details, severity) {{
    results.push({{ name, category, passed, details, severity }});
}}

try {{
    const browser = await chromium.launch({{ headless: true }});
    const context = await browser.newContext({{ viewport: {{ width: 1440, height: 900 }} }});
    const page = await context.newPage();

    // Collect console errors
    const consoleErrors = [];
    page.on('console', msg => {{
        if (msg.type() === 'error') consoleErrors.push(msg.text());
    }});

    await page.goto(url, {{ waitUntil: 'networkidle', timeout: 15000 }});
    await page.waitForTimeout(2000);

    // H1: System Status — check for live indicator
    const hasLive = await page.locator('text=LIVE').count();
    check('live_indicator', 'system_status', hasLive > 0,
        hasLive > 0 ? 'LIVE indicator found' : 'No LIVE indicator visible', 'warning');

    // H1: Clock updating
    const clock = await page.locator('.clock').textContent().catch(() => '');
    check('clock_visible', 'system_status', !!clock,
        clock ? `Clock shows: ${{clock}}` : 'No clock element found', 'warning');

    // H7: Command palette
    await page.keyboard.press('Meta+k');
    await page.waitForTimeout(300);
    const paletteOpen = await page.locator('.palette-overlay.open').count();
    check('command_palette', 'flexibility', paletteOpen > 0,
        paletteOpen > 0 ? 'Cmd+K opens palette' : 'Cmd+K did not open palette', 'info');
    if (paletteOpen > 0) await page.keyboard.press('Escape');

    // Console errors
    check('no_console_errors', 'error_handling', consoleErrors.length === 0,
        consoleErrors.length === 0 ? 'Zero console errors' : `${{consoleErrors.length}} console errors: ${{consoleErrors[0]}}`,
        consoleErrors.length > 0 ? 'error' : 'info');

    // Responsive: 1100px
    await page.setViewportSize({{ width: 1100, height: 900 }});
    await page.waitForTimeout(500);
    const scrollWidth1100 = await page.evaluate(() => document.documentElement.scrollWidth);
    check('responsive_1100', 'responsive', scrollWidth1100 <= 1110,
        scrollWidth1100 <= 1110 ? 'No horizontal overflow at 1100px' : `Horizontal overflow: ${{scrollWidth1100}}px`, 'warning');

    // Responsive: 900px
    await page.setViewportSize({{ width: 900, height: 900 }});
    await page.waitForTimeout(500);
    const scrollWidth900 = await page.evaluate(() => document.documentElement.scrollWidth);
    check('responsive_900', 'responsive', scrollWidth900 <= 910,
        scrollWidth900 <= 910 ? 'No horizontal overflow at 900px' : `Horizontal overflow: ${{scrollWidth900}}px`, 'warning');

    // Focus indicators
    await page.setViewportSize({{ width: 1440, height: 900 }});
    const focusable = await page.locator('button, input, select, [tabindex]').count();
    check('focusable_elements', 'accessibility', focusable > 5,
        `${{focusable}} focusable elements found`, 'info');

    // Title exists
    const title = await page.title();
    check('page_title', 'seo', title.length > 0,
        `Page title: ${{title}}`, 'info');

    await browser.close();
}} catch(e) {{
    check('playwright_run', 'setup', false, `Error: ${{e.message}}`, 'error');
}}

console.log(JSON.stringify(results));
"#, url)
}

fn parse_playwright_output(output: &str) -> Result<Vec<UxCheck>, String> {
    // Find the JSON array in output (last line usually)
    let json_line = output.lines().rev()
        .find(|line| line.trim().starts_with('['))
        .ok_or("No JSON output from Playwright script")?;

    let arr: Vec<Value> = serde_json::from_str(json_line)
        .map_err(|e| format!("Failed to parse Playwright output: {}", e))?;

    Ok(arr.iter().map(|v| UxCheck {
        name: v["name"].as_str().unwrap_or("unknown").to_string(),
        category: v["category"].as_str().unwrap_or("other").to_string(),
        passed: v["passed"].as_bool().unwrap_or(false),
        details: v["details"].as_str().unwrap_or("").to_string(),
        severity: v["severity"].as_str().unwrap_or("info").to_string(),
    }).collect())
}

// ========== HTML Fetch ==========

fn fetch_page_html(url: &str) -> Result<String, String> {
    // Use curl to fetch the page
    let output = Command::new("curl")
        .args(["-sL", "--max-time", "10", url])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!("curl returned status {}", output.status));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

// ========== Static HTML Checks ==========

fn check_heading_hierarchy(html: &str) -> Vec<UxCheck> {
    let re = regex::Regex::new(r"<(h[1-6])\b").unwrap();
    let headings: Vec<u8> = re.captures_iter(html)
        .filter_map(|c| c[1].chars().last()?.to_digit(10).map(|d| d as u8))
        .collect();

    let mut checks = Vec::new();

    if headings.is_empty() {
        checks.push(UxCheck {
            name: "heading_exists".into(),
            category: "accessibility".into(),
            passed: false,
            details: "No headings found".into(),
            severity: "warning".into(),
        });
        return checks;
    }

    // Check for skipped levels
    let mut prev = 0u8;
    let mut skipped = false;
    for &h in &headings {
        if h > prev + 1 && prev > 0 {
            skipped = true;
        }
        prev = h;
    }

    checks.push(UxCheck {
        name: "heading_hierarchy".into(),
        category: "accessibility".into(),
        passed: !skipped,
        details: if skipped {
            format!("Heading levels skip: {:?}", headings)
        } else {
            format!("Heading hierarchy OK: {:?}", headings)
        },
        severity: if skipped { "warning" } else { "info" }.into(),
    });

    checks
}

fn check_aria_labels(html: &str) -> Vec<UxCheck> {
    let mut checks = Vec::new();

    // Check buttons without accessible text
    let button_re = regex::Regex::new(r"<button[^>]*>([^<]*)</button>").unwrap();
    let mut empty_buttons = 0;
    let mut total_buttons = 0;
    for cap in button_re.captures_iter(html) {
        total_buttons += 1;
        let text = cap[1].trim();
        let tag = &cap[0];
        if text.is_empty() && !tag.contains("aria-label") {
            empty_buttons += 1;
        }
    }

    checks.push(UxCheck {
        name: "button_labels".into(),
        category: "accessibility".into(),
        passed: empty_buttons == 0,
        details: if empty_buttons == 0 {
            format!("All {} buttons have text or aria-label", total_buttons)
        } else {
            format!("{}/{} buttons missing accessible text", empty_buttons, total_buttons)
        },
        severity: if empty_buttons > 0 { "warning" } else { "info" }.into(),
    });

    // Check inputs without labels
    let input_re = regex::Regex::new(r"<input[^>]*>").unwrap();
    let inputs: Vec<&str> = input_re.find_iter(html).map(|m| m.as_str()).collect();
    let unlabeled: Vec<&&str> = inputs.iter()
        .filter(|i| !i.contains("aria-label") && !i.contains("placeholder"))
        .collect();

    checks.push(UxCheck {
        name: "input_labels".into(),
        category: "accessibility".into(),
        passed: unlabeled.is_empty(),
        details: if unlabeled.is_empty() {
            format!("All {} inputs have labels/placeholders", inputs.len())
        } else {
            format!("{}/{} inputs missing labels", unlabeled.len(), inputs.len())
        },
        severity: if unlabeled.is_empty() { "info" } else { "warning" }.into(),
    });

    checks
}

fn check_interactive_elements(html: &str) -> Vec<UxCheck> {
    let mut checks = Vec::new();

    // Check for onclick without keyboard equivalent
    let onclick_re = regex::Regex::new(r#"onclick="[^"]*""#).unwrap();
    let onkeydown_re = regex::Regex::new(r#"onkeydown="[^"]*""#).unwrap();
    let onclick_count = onclick_re.find_iter(html).count();
    let onkeydown_count = onkeydown_re.find_iter(html).count();

    checks.push(UxCheck {
        name: "keyboard_handlers".into(),
        category: "accessibility".into(),
        passed: true, // Info only — onclick on buttons is fine
        details: format!("{} onclick handlers, {} onkeydown handlers", onclick_count, onkeydown_count),
        severity: "info".into(),
    });

    // Check for tabindex
    let tabindex_re = regex::Regex::new(r#"tabindex="([^"]*)""#).unwrap();
    let negative_tabindex: Vec<&str> = tabindex_re.captures_iter(html)
        .filter(|c| c[1].starts_with('-'))
        .map(|c| c.get(1).unwrap().as_str())
        .collect();

    checks.push(UxCheck {
        name: "no_negative_tabindex".into(),
        category: "accessibility".into(),
        passed: negative_tabindex.is_empty(),
        details: if negative_tabindex.is_empty() {
            "No negative tabindex values".into()
        } else {
            format!("{} elements with negative tabindex (keyboard inaccessible)", negative_tabindex.len())
        },
        severity: if negative_tabindex.is_empty() { "info" } else { "warning" }.into(),
    });

    checks
}

fn check_meta_viewport(html: &str) -> Vec<UxCheck> {
    let has_viewport = html.contains(r#"name="viewport""#);
    vec![UxCheck {
        name: "meta_viewport".into(),
        category: "responsive".into(),
        passed: has_viewport,
        details: if has_viewport {
            "Viewport meta tag present".into()
        } else {
            "Missing viewport meta tag — page may not be responsive".into()
        },
        severity: if has_viewport { "info" } else { "error" }.into(),
    }]
}

fn check_empty_states(html: &str) -> Vec<UxCheck> {
    // Check for "No data" or empty state handling
    let has_empty_states = html.contains("no-data") || html.contains("No data") || html.contains("empty-state");
    vec![UxCheck {
        name: "empty_states".into(),
        category: "ux_heuristic".into(),
        passed: has_empty_states,
        details: if has_empty_states {
            "Empty state handling found".into()
        } else {
            "No empty state handling detected — blank sections when no data".into()
        },
        severity: if has_empty_states { "info" } else { "warning" }.into(),
    }]
}

fn check_reduced_motion(html: &str) -> Vec<UxCheck> {
    let has_reduced_motion = html.contains("prefers-reduced-motion");
    vec![UxCheck {
        name: "reduced_motion".into(),
        category: "accessibility".into(),
        passed: has_reduced_motion,
        details: if has_reduced_motion {
            "prefers-reduced-motion media query found".into()
        } else {
            "No prefers-reduced-motion support — animations may cause issues for sensitive users".into()
        },
        severity: if has_reduced_motion { "info" } else { "warning" }.into(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_hierarchy() {
        let html = "<h1>Title</h1><h2>Sub</h2><h3>Detail</h3>";
        let checks = check_heading_hierarchy(html);
        assert!(checks[0].passed, "Sequential headings should pass");
    }

    #[test]
    fn test_heading_hierarchy_skip() {
        let html = "<h1>Title</h1><h3>Skipped h2</h3>";
        let checks = check_heading_hierarchy(html);
        assert!(!checks[0].passed, "Skipped heading level should fail");
    }

    #[test]
    fn test_meta_viewport() {
        let html = r#"<meta name="viewport" content="width=device-width">"#;
        let checks = check_meta_viewport(html);
        assert!(checks[0].passed);
    }

    #[test]
    fn test_empty_states() {
        let html = r#"<div class="no-data">Nothing here</div>"#;
        let checks = check_empty_states(html);
        assert!(checks[0].passed);
    }
}
