//! Knowledge tools: knowledge graph, session replay, TruthGuard fact registry.
//!
//! Thin wrappers over crate::knowledge so all layers route through one place.

use super::super::types::*;

// ── Knowledge Graph ──

/// Add an entity to the knowledge graph
pub fn kgraph_add_entity(req: &KgraphAddEntityRequest) -> String {
    crate::knowledge::kgraph_add_entity(
        &req.name,
        &req.entity_type,
        &req.properties.clone().unwrap_or_else(|| "{}".into()),
        &req.id.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Add a typed edge between entities
pub fn kgraph_add_edge(req: &KgraphAddEdgeRequest) -> String {
    crate::knowledge::kgraph_add_edge(
        &req.source,
        &req.target,
        &req.relation,
        req.weight.unwrap_or(1.0),
        &req.properties.clone().unwrap_or_else(|| "{}".into()),
    )
    .to_string()
}

/// Record an observation on an edge
pub fn kgraph_observe(req: &KgraphObserveRequest) -> String {
    crate::knowledge::kgraph_observe(
        &req.source,
        &req.target,
        &req.relation,
        &req.observation,
        req.impact.unwrap_or(0.1),
        &req.session_id.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Query neighbors via BFS
pub fn kgraph_query_neighbors(req: &KgraphQueryNeighborsRequest) -> String {
    crate::knowledge::kgraph_query_neighbors(
        &req.entity,
        &req.relation.clone().unwrap_or_default(),
        &req.direction.clone().unwrap_or_else(|| "both".into()),
        req.depth.unwrap_or(1),
        req.limit.unwrap_or(50),
    )
    .to_string()
}

/// Find shortest path between entities
pub fn kgraph_query_path(req: &KgraphQueryPathRequest) -> String {
    crate::knowledge::kgraph_query_path(&req.source, &req.target, req.max_depth.unwrap_or(4))
        .to_string()
}

/// Search entities by name or properties
pub fn kgraph_search(req: &KgraphSearchRequest) -> String {
    crate::knowledge::kgraph_search(
        &req.query,
        &req.entity_type.clone().unwrap_or_default(),
        req.limit.unwrap_or(20),
    )
    .to_string()
}

/// Delete an entity or edge
pub fn kgraph_delete(req: &KgraphDeleteRequest) -> String {
    crate::knowledge::kgraph_delete(
        &req.entity_id.clone().unwrap_or_default(),
        &req.edge_source.clone().unwrap_or_default(),
        &req.edge_target.clone().unwrap_or_default(),
        &req.edge_relation.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Knowledge graph statistics
pub fn kgraph_stats() -> String {
    crate::knowledge::kgraph_stats().to_string()
}

// ── Session Replay ──

/// Index session JSONL files
pub fn replay_index(req: &ReplayIndexRequest) -> String {
    crate::knowledge::replay_index(
        req.force.unwrap_or(false),
        &req.project.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Search across indexed sessions
pub fn replay_search(req: &ReplaySearchRequest) -> String {
    crate::knowledge::replay_search(
        &req.query,
        &req.project.clone().unwrap_or_default(),
        &req.tool.clone().unwrap_or_default(),
        req.limit.unwrap_or(20),
        req.days.unwrap_or(0),
    )
    .to_string()
}

/// Retrieve full session turns
pub fn replay_session(req: &ReplaySessionRequest) -> String {
    crate::knowledge::replay_session(
        &req.session_id,
        req.include_tools.unwrap_or(true),
        req.include_errors.unwrap_or(true),
        req.max_messages.unwrap_or(100),
    )
    .to_string()
}

/// List indexed sessions
pub fn replay_list_sessions(req: &ReplayListSessionsRequest) -> String {
    crate::knowledge::replay_list_sessions(
        &req.project.clone().unwrap_or_default(),
        req.days.unwrap_or(30),
        req.limit.unwrap_or(50),
    )
    .to_string()
}

/// Tool usage history
pub fn replay_tool_history(req: &ReplayToolHistoryRequest) -> String {
    crate::knowledge::replay_tool_history(
        &req.tool_name,
        req.limit.unwrap_or(20),
        req.days.unwrap_or(0),
    )
    .to_string()
}

/// Recent errors across sessions
pub fn replay_errors(req: &ReplayErrorsRequest) -> String {
    crate::knowledge::replay_errors(
        &req.project.clone().unwrap_or_default(),
        req.days.unwrap_or(7),
        req.limit.unwrap_or(50),
    )
    .to_string()
}

/// Session replay index status
pub fn replay_status() -> String {
    crate::knowledge::replay_status().to_string()
}

// ── TruthGuard ──

/// Add an immutable fact
pub fn fact_add(req: &FactAddRequest) -> String {
    crate::knowledge::fact_add(
        &req.category,
        &req.key,
        &req.value,
        req.confidence.unwrap_or(1.0),
        &req.source.clone().unwrap_or_default(),
        &req.aliases.clone().unwrap_or_default(),
        &req.tags.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Get a fact by ID or key
pub fn fact_get(req: &FactGetRequest) -> String {
    crate::knowledge::fact_get(
        &req.fact_id.clone().unwrap_or_default(),
        &req.key.clone().unwrap_or_default(),
        &req.category.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Search facts
pub fn fact_search(req: &FactSearchRequest) -> String {
    crate::knowledge::fact_search(
        &req.query.clone().unwrap_or_default(),
        &req.category.clone().unwrap_or_default(),
        req.min_confidence.unwrap_or(0.0),
        req.limit.unwrap_or(20),
    )
    .to_string()
}

/// Check a claim against known facts
pub fn fact_check(claim: &str) -> String {
    crate::knowledge::fact_check(claim).to_string()
}

/// Check an entire response for contradictions
pub fn fact_check_response(response_text: &str) -> String {
    crate::knowledge::fact_check_response(response_text).to_string()
}

/// Update a fact
pub fn fact_update(req: &FactUpdateRequest) -> String {
    crate::knowledge::fact_update(
        &req.fact_id.clone().unwrap_or_default(),
        &req.category.clone().unwrap_or_default(),
        &req.key.clone().unwrap_or_default(),
        &req.value.clone().unwrap_or_default(),
        req.confidence.unwrap_or(-1.0),
        &req.aliases.clone().unwrap_or_default(),
        &req.source.clone().unwrap_or_default(),
        &req.tags.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Delete a fact
pub fn fact_delete(fact_id: &str, reason: &str) -> String {
    crate::knowledge::fact_delete(fact_id, reason).to_string()
}

/// TruthGuard status
pub fn truthguard_status() -> String {
    crate::knowledge::truthguard_status().to_string()
}
