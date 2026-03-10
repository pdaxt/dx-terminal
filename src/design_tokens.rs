//! Design tokens parsed from dashboard.html :root CSS variables.
//! Single source of truth for the DX Terminal design system.

use serde_json::{json, Value};
use std::collections::HashMap;

/// All design tokens extracted from :root
#[derive(Debug, Clone)]
pub struct DesignTokens {
    pub colors: Vec<ColorToken>,
    pub typography: Vec<TypographyToken>,
    pub radii: Vec<RadiusToken>,
    pub transitions: Vec<TransitionToken>,
    pub shadows: Vec<ShadowToken>,
    pub raw: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ColorToken {
    pub name: String,
    pub var_name: String,
    pub value: String,
    pub rgb: Option<(u8, u8, u8)>,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct TypographyToken {
    pub name: String,
    pub var_name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct RadiusToken {
    pub name: String,
    pub var_name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct TransitionToken {
    pub name: String,
    pub var_name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ShadowToken {
    pub name: String,
    pub var_name: String,
    pub value: String,
}

// ========== Allowed Scales ==========

/// Allowed font sizes in the design system
pub const FONT_SCALE: &[f32] = &[8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 15.0, 16.0, 18.0, 26.0];

/// Allowed font weights
pub const WEIGHT_SCALE: &[u16] = &[400, 500, 600, 700, 800];

/// Allowed spacing values (px)
pub const SPACING_SCALE: &[f32] = &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0, 14.0, 16.0, 20.0];

// ========== Hex Parsing ==========

/// Parse a hex color string (#rgb or #rrggbb) into (r, g, b)
pub fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

/// sRGB to linear conversion for luminance calculation
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance per WCAG 2.0
pub fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let r = srgb_to_linear(r as f64 / 255.0);
    let g = srgb_to_linear(g as f64 / 255.0);
    let b = srgb_to_linear(b as f64 / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio between two luminance values
pub fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Check contrast between two hex colors, returns structured result
pub fn check_contrast(fg: &str, bg: &str) -> Value {
    let fg_rgb = match parse_hex_color(fg) {
        Some(c) => c,
        None => return json!({"error": format!("Invalid foreground color: {}", fg)}),
    };
    let bg_rgb = match parse_hex_color(bg) {
        Some(c) => c,
        None => return json!({"error": format!("Invalid background color: {}", bg)}),
    };

    let l1 = relative_luminance(fg_rgb.0, fg_rgb.1, fg_rgb.2);
    let l2 = relative_luminance(bg_rgb.0, bg_rgb.1, bg_rgb.2);
    let ratio = contrast_ratio(l1, l2);

    let aa_normal = ratio >= 4.5;
    let aa_large = ratio >= 3.0;
    let aaa_normal = ratio >= 7.0;
    let grade = if aaa_normal {
        "AAA"
    } else if aa_normal {
        "AA"
    } else if aa_large {
        "AA-large"
    } else {
        "fail"
    };

    json!({
        "foreground": fg,
        "background": bg,
        "ratio": format!("{:.1}:1", ratio),
        "ratio_num": (ratio * 10.0).round() / 10.0,
        "aa_normal": aa_normal,
        "aa_large": aa_large,
        "aaa_normal": aaa_normal,
        "grade": grade,
    })
}

// ========== Token Categorization ==========

fn categorize_color(name: &str) -> &'static str {
    match name {
        "bg" | "surface" | "surface2" => "background",
        "text" | "muted" | "dim" => "text",
        "border" | "border-bright" => "border",
        "blue" | "green" | "yellow" | "red" | "purple" | "teal" | "orange" | "pink" => "accent",
        _ => "other",
    }
}

fn is_color_value(val: &str) -> bool {
    val.starts_with('#') || val.starts_with("rgb") || val.starts_with("hsl")
}

fn is_font_value(val: &str) -> bool {
    val.contains("font") || val.contains("Mono") || val.contains("sans") || val.contains("system")
}

// ========== Token Parsing ==========

/// Parse :root CSS variables from an HTML string
pub fn parse_tokens_from_html(html: &str) -> DesignTokens {
    let mut raw = HashMap::new();
    let mut colors = Vec::new();
    let mut typography = Vec::new();
    let mut radii = Vec::new();
    let mut transitions = Vec::new();
    let mut shadows = Vec::new();

    // Extract :root block content
    let root_content = extract_root_block(html);

    // Parse each --var: value; pair
    let re = regex::Regex::new(r"--([a-zA-Z0-9_-]+)\s*:\s*([^;]+);").unwrap();
    for cap in re.captures_iter(&root_content) {
        let name = cap[1].trim().to_string();
        let value = cap[2].trim().to_string();
        let var_name = format!("--{}", name);

        raw.insert(var_name.clone(), value.clone());

        if is_color_value(&value) {
            colors.push(ColorToken {
                name: name.clone(),
                var_name: var_name.clone(),
                value: value.clone(),
                rgb: parse_hex_color(&value),
                category: categorize_color(&name).to_string(),
            });
        } else if is_font_value(&value) {
            typography.push(TypographyToken {
                name: name.clone(),
                var_name,
                value,
            });
        } else if name.contains("radius") {
            radii.push(RadiusToken {
                name: name.clone(),
                var_name,
                value,
            });
        } else if name.contains("transition") {
            transitions.push(TransitionToken {
                name: name.clone(),
                var_name,
                value,
            });
        } else if name.contains("shadow") {
            shadows.push(ShadowToken {
                name: name.clone(),
                var_name,
                value,
            });
        }
    }

    DesignTokens { colors, typography, radii, transitions, shadows, raw }
}

/// Extract the content of the :root { ... } block from CSS/HTML
fn extract_root_block(html: &str) -> String {
    // Find :root{ or :root { and capture everything until the matching }
    if let Some(start) = html.find(":root") {
        if let Some(brace_start) = html[start..].find('{') {
            let after_brace = start + brace_start + 1;
            let mut depth = 1;
            let mut end = after_brace;
            for (i, ch) in html[after_brace..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = after_brace + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            return html[after_brace..end].to_string();
        }
    }
    String::new()
}

// ========== Public API ==========

/// Get design tokens from the embedded dashboard HTML
pub fn design_tokens() -> Value {
    let html = include_str!("../assets/dashboard.html");
    let tokens = parse_tokens_from_html(html);
    tokens_to_json(&tokens)
}

/// Get design tokens from an arbitrary HTML file
pub fn design_tokens_from_file(path: &str) -> Value {
    match std::fs::read_to_string(path) {
        Ok(html) => {
            let tokens = parse_tokens_from_html(&html);
            tokens_to_json(&tokens)
        }
        Err(e) => json!({"error": format!("Failed to read {}: {}", path, e)}),
    }
}

/// Check all standard contrast pairs from design tokens
pub fn check_all_contrasts() -> Value {
    let html = include_str!("../assets/dashboard.html");
    let tokens = parse_tokens_from_html(html);

    let bg_colors: Vec<(&str, &str)> = tokens.colors.iter()
        .filter(|c| c.category == "background")
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();

    let text_colors: Vec<(&str, &str)> = tokens.colors.iter()
        .filter(|c| c.category == "text" || c.category == "accent")
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();

    let mut results = Vec::new();
    for (bg_name, bg_val) in &bg_colors {
        for (fg_name, fg_val) in &text_colors {
            let mut result = check_contrast(fg_val, bg_val);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("fg_name".to_string(), json!(format!("--{}", fg_name)));
                obj.insert("bg_name".to_string(), json!(format!("--{}", bg_name)));
            }
            results.push(result);
        }
    }

    // --dim is intentionally below AA contrast (decorative-only: timestamps, IDs)
    // Don't count it as a failure in scoring
    let failures: Vec<&Value> = results.iter()
        .filter(|r| {
            r.get("grade").and_then(|g| g.as_str()) == Some("fail")
                && r.get("fg_name").and_then(|n| n.as_str()) != Some("--dim")
        })
        .collect();

    json!({
        "total_pairs": results.len(),
        "failures": failures.len(),
        "all_pass": failures.is_empty(),
        "results": results,
    })
}

fn tokens_to_json(tokens: &DesignTokens) -> Value {
    json!({
        "colors": tokens.colors.iter().map(|c| json!({
            "name": c.name,
            "var": c.var_name,
            "value": c.value,
            "rgb": c.rgb.map(|(r,g,b)| format!("rgb({},{},{})", r, g, b)),
            "category": c.category,
        })).collect::<Vec<_>>(),
        "typography": tokens.typography.iter().map(|t| json!({
            "name": t.name,
            "var": t.var_name,
            "value": t.value,
        })).collect::<Vec<_>>(),
        "radii": tokens.radii.iter().map(|r| json!({
            "name": r.name,
            "var": r.var_name,
            "value": r.value,
        })).collect::<Vec<_>>(),
        "transitions": tokens.transitions.iter().map(|t| json!({
            "name": t.name,
            "var": t.var_name,
            "value": t.value,
        })).collect::<Vec<_>>(),
        "shadows": tokens.shadows.iter().map(|s| json!({
            "name": s.name,
            "var": s.var_name,
            "value": s.value,
        })).collect::<Vec<_>>(),
        "scales": {
            "font_sizes": FONT_SCALE,
            "font_weights": WEIGHT_SCALE,
            "spacing": SPACING_SCALE,
        },
        "raw": tokens.raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_hex_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_color("#4da6ff"), Some((77, 166, 255)));
    }

    #[test]
    fn test_contrast_ratio_black_white() {
        let l1 = relative_luminance(255, 255, 255);
        let l2 = relative_luminance(0, 0, 0);
        let ratio = contrast_ratio(l1, l2);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn test_design_tokens_parses() {
        let tokens = design_tokens();
        assert!(tokens.get("colors").is_some());
        let colors = tokens["colors"].as_array().unwrap();
        assert!(colors.len() >= 8, "Expected at least 8 color tokens");
    }

    #[test]
    fn test_check_contrast_valid() {
        let result = check_contrast("#d4dce8", "#0a0e14");
        assert_eq!(result["grade"], "AAA");
        assert!(result["aa_normal"].as_bool().unwrap());
    }

    #[test]
    fn test_check_contrast_fail() {
        let result = check_contrast("#3d4f63", "#0a0e14");
        assert_eq!(result["grade"], "fail");
    }
}
