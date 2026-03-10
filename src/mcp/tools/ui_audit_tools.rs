//! UI/UX audit MCP tool wrappers

/// Run UI design system audit on a file
pub fn audit_ui(file: Option<&str>) -> String {
    match file {
        Some(path) => crate::ui_audit::audit_ui_file(path).to_string(),
        None => {
            // Audit the embedded dashboard
            let html = include_str!("../../../assets/dashboard.html");
            crate::ui_audit::audit_ui_html(html, "dashboard.html (embedded)").to_string()
        }
    }
}

/// Run UX audit on a URL
pub fn audit_ux(url: &str) -> String {
    crate::ux_audit::audit_ux(url).to_string()
}

/// Get design tokens from embedded dashboard
pub fn design_tokens() -> String {
    crate::design_tokens::design_tokens().to_string()
}

/// Check WCAG contrast ratio between two colors
pub fn contrast_check(fg: &str, bg: &str) -> String {
    crate::design_tokens::check_contrast(fg, bg).to_string()
}
