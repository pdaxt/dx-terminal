use serde_json::json;

use crate::mcp::types::{ClaimIssueRequest, ListClaimsRequest, ReleaseClaimRequest};

pub fn claim_issue(req: ClaimIssueRequest) -> String {
    match crate::claims::try_claim(&req.repo, req.issue, &req.agent_id) {
        Ok(true) => json!({
            "status": "claimed",
            "repo": req.repo,
            "issue": req.issue,
            "agent_id": req.agent_id,
        })
        .to_string(),
        Ok(false) => {
            let holder = crate::claims::is_claimed(&req.repo, req.issue)
                .ok()
                .flatten()
                .map(|c| c.agent_id)
                .unwrap_or_else(|| "unknown".to_string());
            json!({
                "status": "denied",
                "repo": req.repo,
                "issue": req.issue,
                "held_by": holder,
                "reason": "issue is already claimed by another agent",
            })
            .to_string()
        }
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub fn release_claim(req: ReleaseClaimRequest) -> String {
    let status = req.status.as_deref().unwrap_or("released");
    match crate::claims::release(&req.repo, req.issue, status) {
        Ok(true) => json!({
            "status": "released",
            "repo": req.repo,
            "issue": req.issue,
        })
        .to_string(),
        Ok(false) => json!({
            "status": "not_found",
            "repo": req.repo,
            "issue": req.issue,
            "reason": "no active claim found for this issue",
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub fn list_claims(req: ListClaimsRequest) -> String {
    let active_only = req.active_only.unwrap_or(true);
    match crate::claims::list(req.repo.as_deref(), active_only) {
        Ok(claims) => json!({
            "status": "ok",
            "count": claims.len(),
            "claims": claims,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}
