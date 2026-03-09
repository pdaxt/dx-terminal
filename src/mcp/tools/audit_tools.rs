//! Audit tools: code quality, security, intent verification, dependency health, full audit.
//!
//! Thin wrappers over crate::audit so all layers route through one place.

/// Audit code quality
pub fn audit_code(project: &str) -> String {
    crate::audit::audit_code(project).to_string()
}

/// Security audit
pub fn audit_security(project: &str) -> String {
    crate::audit::audit_security(project).to_string()
}

/// Intent verification
pub fn audit_intent(project: &str, description: &str) -> String {
    crate::audit::audit_intent(project, description).to_string()
}

/// Dependency health audit
pub fn audit_deps(project: &str) -> String {
    crate::audit::audit_deps(project).to_string()
}

/// Full audit (code + security + intent + deps)
pub fn audit_full(project: &str) -> String {
    crate::audit::audit_full(project).to_string()
}
