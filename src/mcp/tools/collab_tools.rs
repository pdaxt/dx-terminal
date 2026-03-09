//! Collaboration tools: spaces, documents CRUD, proposals, comments, search.
//!
//! Thin wrappers over crate::collab so all layers route through one place.

use super::super::types::*;

// ── Spaces ──

/// List all collaboration spaces
pub fn space_list() -> String {
    crate::collab::space_list().to_string()
}

/// Create a new space
pub fn space_create(name: &str) -> String {
    crate::collab::space_create(name).to_string()
}

// ── Documents ──

/// List documents with optional filters
pub fn doc_list(req: &DocListRequest) -> String {
    crate::collab::doc_list(
        &req.space.clone().unwrap_or_default(),
        &req.status.clone().unwrap_or_default(),
    ).to_string()
}

/// Read a document with metadata
pub fn doc_read(req: &DocReadRequest) -> String {
    crate::collab::doc_read(&req.space, &req.name, req.include_meta.unwrap_or(true)).to_string()
}

/// Create a new document
pub fn doc_create(req: &DocCreateRequest) -> String {
    crate::collab::doc_create(
        &req.space, &req.name, &req.content.clone().unwrap_or_default(),
        &req.status.clone().unwrap_or_default(), &req.tags.clone().unwrap_or_default(),
    ).to_string()
}

/// Edit a document
pub fn doc_edit(req: &DocEditRequest) -> String {
    crate::collab::doc_edit(
        &req.space, &req.name, &req.content, &req.agent_id.clone().unwrap_or_default(),
    ).to_string()
}

/// Propose changes to a document
pub fn doc_propose(req: &DocProposeRequest) -> String {
    crate::collab::doc_propose(
        &req.space, &req.name, &req.content,
        &req.summary.clone().unwrap_or_default(), &req.agent_id.clone().unwrap_or_default(),
    ).to_string()
}

/// Approve a proposal
pub fn doc_approve(req: &DocApproveRequest) -> String {
    crate::collab::doc_approve(
        &req.space, &req.name, &req.proposal_id.clone().unwrap_or_else(|| "latest".into()),
    ).to_string()
}

/// Reject a proposal
pub fn doc_reject(req: &DocRejectRequest) -> String {
    crate::collab::doc_reject(
        &req.space, &req.name, &req.proposal_id, &req.reason.clone().unwrap_or_default(),
    ).to_string()
}

/// Lock a document
pub fn doc_lock(req: &DocLockRequest) -> String {
    crate::collab::doc_lock(
        &req.space, &req.name, &req.locked_by.clone().unwrap_or_default(),
    ).to_string()
}

/// Unlock a document
pub fn doc_unlock(space: &str, name: &str) -> String {
    crate::collab::doc_unlock(space, name).to_string()
}

/// Add a comment to a document
pub fn doc_comment(req: &DocCommentRequest) -> String {
    crate::collab::doc_comment(
        &req.space, &req.name, &req.text,
        &req.author.clone().unwrap_or_default(), req.line.unwrap_or(0),
    ).to_string()
}

/// Read all comments on a document
pub fn doc_comments(space: &str, name: &str) -> String {
    crate::collab::doc_comments(space, name).to_string()
}

/// Update document status
pub fn doc_status(space: &str, name: &str, status: &str) -> String {
    crate::collab::doc_status(space, name, status).to_string()
}

/// Search across documents
pub fn doc_search(query: &str, space: &str) -> String {
    crate::collab::doc_search(query, space).to_string()
}

/// Find directives in documents
pub fn doc_directives(space: &str) -> String {
    crate::collab::doc_directives(space).to_string()
}

/// Show document version history
pub fn doc_history(space: &str, name: &str, limit: u32) -> String {
    crate::collab::doc_history(space, name, limit).to_string()
}

/// Delete a document
pub fn doc_delete(space: &str, name: &str, confirm: bool) -> String {
    crate::collab::doc_delete(space, name, confirm).to_string()
}

/// Initialize collab workspace
pub fn collab_init() -> String {
    crate::collab::collab_init().to_string()
}
