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

#[allow(clippy::too_many_arguments)]
pub fn session_upsert(
    project: Option<&str>,
    session_id: Option<&str>,
    role: &str,
    provider: Option<&str>,
    model: Option<&str>,
    autonomy_level: Option<&str>,
    objective: &str,
    expected_outputs: Vec<String>,
    allowed_capabilities: Vec<String>,
    allowed_repos: Vec<String>,
    allowed_paths: Vec<String>,
    workspace_path: Option<&str>,
    branch_name: Option<&str>,
    browser_port: Option<u16>,
    pane: Option<u8>,
    tmux_target: Option<&str>,
    feature_id: Option<&str>,
    stage: Option<&str>,
    supervisor_session_id: Option<&str>,
    escalation_policy: Option<&str>,
    status: Option<&str>,
) -> String {
    dxos::upsert_session_contract(
        &resolve_project_path(project),
        None,
        session_id,
        role,
        provider,
        model,
        autonomy_level,
        objective,
        expected_outputs,
        allowed_capabilities,
        allowed_repos,
        allowed_paths,
        workspace_path,
        branch_name,
        browser_port,
        pane,
        tmux_target,
        feature_id,
        stage,
        supervisor_session_id,
        escalation_policy,
        status,
    )
}

pub fn session_list(project: Option<&str>) -> String {
    dxos::session_list(&resolve_project_path(project), None)
}

pub fn session_status(
    project: Option<&str>,
    session_id: &str,
    status: &str,
    note: Option<&str>,
) -> String {
    dxos::update_session_status(
        &resolve_project_path(project),
        None,
        session_id,
        status,
        note,
    )
}

pub fn work_delegate(
    project: Option<&str>,
    supervisor_session_id: &str,
    worker_session_id: Option<&str>,
    title: &str,
    objective: &str,
    feature_id: Option<&str>,
    stage: Option<&str>,
    required_capabilities: Vec<String>,
    expected_outputs: Vec<String>,
) -> String {
    dxos::delegate_work_order(
        &resolve_project_path(project),
        None,
        supervisor_session_id,
        worker_session_id,
        title,
        objective,
        feature_id,
        stage,
        required_capabilities,
        expected_outputs,
    )
}

pub fn work_block(
    project: Option<&str>,
    work_order_id: &str,
    blocker: &str,
    requested_permission: Option<&str>,
) -> String {
    dxos::work_order_block(
        &resolve_project_path(project),
        None,
        work_order_id,
        blocker,
        requested_permission,
    )
}

pub fn work_resolve(
    project: Option<&str>,
    work_order_id: &str,
    resolution: Option<&str>,
) -> String {
    dxos::resolve_work_order(
        &resolve_project_path(project),
        None,
        work_order_id,
        resolution,
    )
}
