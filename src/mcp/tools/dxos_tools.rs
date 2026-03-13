use crate::dxos;

pub fn resolve_project_path(project: Option<&str>) -> String {
    project.unwrap_or(".").to_string()
}

pub fn control_plane(project: Option<&str>) -> String {
    let project_path = resolve_project_path(project);
    dxos::control_plane_snapshot(&project_path, None).to_string()
}

pub fn debate_list(project: Option<&str>) -> String {
    dxos::debate_list(&resolve_project_path(project), None)
}

pub fn debate_start(
    project: Option<&str>,
    title: &str,
    objective: &str,
    stage: Option<&str>,
    feature_id: Option<&str>,
    participants: Vec<String>,
    requested_by: Option<&str>,
) -> String {
    dxos::debate_start(
        &resolve_project_path(project),
        None,
        title,
        objective,
        stage,
        feature_id,
        participants,
        requested_by,
    )
}

pub fn debate_proposal(
    project: Option<&str>,
    debate_id: &str,
    author: &str,
    model: Option<&str>,
    summary: &str,
    rationale: &str,
    evidence: Vec<String>,
) -> String {
    dxos::debate_add_proposal(
        &resolve_project_path(project),
        None,
        debate_id,
        author,
        model,
        summary,
        rationale,
        evidence,
    )
}

pub fn debate_contradiction(
    project: Option<&str>,
    debate_id: &str,
    proposal_id: &str,
    author: &str,
    model: Option<&str>,
    rationale: &str,
) -> String {
    dxos::debate_add_contradiction(
        &resolve_project_path(project),
        None,
        debate_id,
        proposal_id,
        author,
        model,
        rationale,
    )
}

pub fn debate_vote(
    project: Option<&str>,
    debate_id: &str,
    proposal_id: &str,
    voter: &str,
    model: Option<&str>,
    stance: &str,
    rationale: &str,
) -> String {
    dxos::debate_cast_vote(
        &resolve_project_path(project),
        None,
        debate_id,
        proposal_id,
        voter,
        model,
        stance,
        rationale,
    )
}

pub fn debate_finalize(
    project: Option<&str>,
    debate_id: &str,
    chosen_proposal_id: &str,
    decided_by: &str,
    summary: &str,
    rationale: &str,
) -> String {
    dxos::debate_finalize(
        &resolve_project_path(project),
        None,
        debate_id,
        chosen_proposal_id,
        decided_by,
        summary,
        rationale,
    )
}
