use std::collections::{HashMap, VecDeque};
use std::io::BufRead;
use chrono::Utc;
use regex::Regex;
use rusqlite::{Connection, params};
use serde_json::{json, Value};

use crate::config;

// === COMMON HELPERS ===

fn now_iso() -> String { Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string() }

fn make_id(name: &str, entity_type: &str) -> String {
    let input = format!("{}:{}", entity_type, name);
    format!("{:012x}", crate::collab::crc32(&input) as u64)
}

fn gen_id(prefix: &str) -> String {
    let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    format!("{}-{:08x}", prefix, ts & 0xFFFF_FFFF)
}

// ═══════════════════════════════════════════════════════════════════════
// KGRAPH — Knowledge Graph (8 tools)
// DB: ~/.claude/experience/kgraph.db
// ═══════════════════════════════════════════════════════════════════════

const KGRAPH_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, type TEXT NOT NULL,
    properties TEXT DEFAULT '{}',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation TEXT NOT NULL, weight REAL DEFAULT 1.0,
    properties TEXT DEFAULT '{}', created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_id, target_id, relation)
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
CREATE TABLE IF NOT EXISTS observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    edge_id INTEGER REFERENCES edges(id) ON DELETE CASCADE,
    session_id TEXT DEFAULT '', observation TEXT NOT NULL,
    impact REAL DEFAULT 0.1, created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_observations_edge ON observations(edge_id);
"#;

fn kgraph_db() -> Result<Connection, String> {
    let dir = config::home_dir().join(".claude").join("experience");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("kgraph.db");
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").map_err(|e| e.to_string())?;
    conn.execute_batch(KGRAPH_SCHEMA).map_err(|e| e.to_string())?;
    Ok(conn)
}

fn resolve_entity(conn: &Connection, reference: &str) -> Option<String> {
    conn.query_row("SELECT id FROM entities WHERE id = ?1", params![reference], |r| r.get(0)).ok()
        .or_else(|| conn.query_row("SELECT id FROM entities WHERE name = ?1 COLLATE NOCASE", params![reference], |r| r.get(0)).ok())
}

pub fn kgraph_add_entity(name: &str, entity_type: &str, properties: &str, id: &str) -> Value {
    let valid_types = ["project","file","tool","pattern","error","person","concept","mcp","library","platform","config","service","database"];
    if !valid_types.contains(&entity_type) {
        return json!({"error": format!("Invalid type: {}. Valid: {:?}", entity_type, valid_types)});
    }
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let entity_id = if id.is_empty() { make_id(name, entity_type) } else { id.to_string() };
    let now = now_iso();

    let exists: bool = conn.query_row("SELECT COUNT(*) FROM entities WHERE id = ?1", params![entity_id], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if exists {
        let _ = conn.execute("UPDATE entities SET properties = ?1, updated_at = ?2 WHERE id = ?3", params![properties, now, entity_id]);
    } else {
        let _ = conn.execute("INSERT INTO entities (id, name, type, properties, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![entity_id, name, entity_type, properties, now, now]);
    }
    json!({"id": entity_id, "name": name, "type": entity_type, "action": if exists {"updated"} else {"created"}})
}

pub fn kgraph_add_edge(source: &str, target: &str, relation: &str, weight: f64, properties: &str) -> Value {
    let valid = ["uses","depends_on","causes","fixes","part_of","related_to","conflicts_with","replaced_by","about","solved_by","creates","configures","tests","deploys","documents"];
    if !valid.contains(&relation) {
        return json!({"error": format!("Invalid relation: {}. Valid: {:?}", relation, valid)});
    }
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let src = match resolve_entity(&conn, source) { Some(s) => s, None => return json!({"error": format!("Entity not found: {}", source)}) };
    let tgt = match resolve_entity(&conn, target) { Some(t) => t, None => return json!({"error": format!("Entity not found: {}", target)}) };
    let w = weight.clamp(0.0, 10.0);

    match conn.execute("INSERT INTO edges (source_id, target_id, relation, weight, properties, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![src, tgt, relation, w, properties, now_iso()]) {
        Ok(_) => {},
        Err(_) => { let _ = conn.execute("UPDATE edges SET weight=?1, properties=?2 WHERE source_id=?3 AND target_id=?4 AND relation=?5",
            params![w, properties, src, tgt, relation]); },
    }
    json!({"source": src, "target": tgt, "relation": relation, "weight": w})
}

pub fn kgraph_observe(source: &str, target: &str, relation: &str, observation: &str, impact: f64, session_id: &str) -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let impact = impact.clamp(-1.0, 1.0);

    // Auto-create entities
    let src = resolve_entity(&conn, source).unwrap_or_else(|| {
        let id = make_id(source, "concept");
        let _ = conn.execute("INSERT OR IGNORE INTO entities (id, name, type) VALUES (?1,?2,'concept')", params![id, source]);
        id
    });
    let tgt = resolve_entity(&conn, target).unwrap_or_else(|| {
        let id = make_id(target, "concept");
        let _ = conn.execute("INSERT OR IGNORE INTO entities (id, name, type) VALUES (?1,?2,'concept')", params![id, target]);
        id
    });

    // Ensure edge
    let edge: Option<(i64, f64)> = conn.query_row(
        "SELECT id, weight FROM edges WHERE source_id=?1 AND target_id=?2 AND relation=?3",
        params![src, tgt, relation], |r| Ok((r.get(0)?, r.get(1)?))).ok();
    let (edge_id, old_weight) = match edge {
        Some(e) => e,
        None => {
            let _ = conn.execute("INSERT INTO edges (source_id, target_id, relation, weight) VALUES (?1,?2,?3,1.0)", params![src, tgt, relation]);
            conn.query_row("SELECT id, weight FROM edges WHERE source_id=?1 AND target_id=?2 AND relation=?3",
                params![src, tgt, relation], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((0, 1.0))
        }
    };
    let _ = conn.execute("INSERT INTO observations (edge_id, session_id, observation, impact) VALUES (?1,?2,?3,?4)",
        params![edge_id, session_id, observation, impact]);
    let new_weight = (old_weight + impact).max(0.0);
    let _ = conn.execute("UPDATE edges SET weight=?1 WHERE id=?2", params![new_weight, edge_id]);
    json!({"source": source, "target": target, "relation": relation, "old_weight": old_weight, "new_weight": new_weight})
}

pub fn kgraph_query_neighbors(entity: &str, relation: &str, direction: &str, depth: u32, limit: u32) -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let start_id = match resolve_entity(&conn, entity) { Some(s) => s, None => return json!({"error": format!("Entity not found: {}", entity)}) };
    let depth = depth.min(4);

    // BFS traversal
    let mut visited: HashMap<String, u32> = HashMap::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    visited.insert(start_id.clone(), 0);
    queue.push_back((start_id, 0));

    while let Some((node, d)) = queue.pop_front() {
        if d >= depth { continue; }
        let sql = match direction {
            "outgoing" => "SELECT target_id as next FROM edges WHERE source_id=?1",
            "incoming" => "SELECT source_id as next FROM edges WHERE target_id=?1",
            _ => "SELECT CASE WHEN source_id=?1 THEN target_id ELSE source_id END as next FROM edges WHERE source_id=?1 OR target_id=?1",
        };
        let sql = if !relation.is_empty() { format!("{} AND relation=?2", sql) } else { sql.to_string() };
        let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(_) => continue };
        let rows: Vec<String> = if !relation.is_empty() {
            stmt.query_map(params![node, relation], |r| r.get(0)).unwrap().flatten().collect()
        } else {
            stmt.query_map(params![node], |r| r.get(0)).unwrap().flatten().collect()
        };
        for next in rows {
            if visited.len() as u32 >= limit { break; }
            if !visited.contains_key(&next) {
                visited.insert(next.clone(), d + 1);
                queue.push_back((next, d + 1));
            }
        }
    }

    let ids: Vec<&str> = visited.keys().map(|s| s.as_str()).collect();
    if ids.is_empty() {
        return json!({"root": entity, "depth": depth, "node_count": 0, "edge_count": 0, "nodes": [], "edges": []});
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT * FROM entities WHERE id IN ({})", placeholders);
    let mut stmt = conn.prepare(&sql).unwrap();
    let nodes: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(&ids), |r| {
        Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?,
            "properties": r.get::<_, String>(3).unwrap_or_default()}))
    }).unwrap().flatten().collect();

    let esql = format!("SELECT * FROM edges WHERE source_id IN ({0}) AND target_id IN ({0})", placeholders);
    let mut params_double: Vec<&str> = Vec::new();
    params_double.extend_from_slice(&ids);
    params_double.extend_from_slice(&ids);
    let mut stmt = conn.prepare(&esql).unwrap();
    let edges: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(&params_double), |r| {
        Ok(json!({"id": r.get::<_, i64>(0)?, "source_id": r.get::<_, String>(1)?, "target_id": r.get::<_, String>(2)?,
            "relation": r.get::<_, String>(3)?, "weight": r.get::<_, f64>(4)?}))
    }).unwrap().flatten().collect();

    json!({"root": entity, "depth": depth, "node_count": nodes.len(), "edge_count": edges.len(), "nodes": nodes, "edges": edges})
}

pub fn kgraph_query_path(source: &str, target: &str, max_depth: u32) -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let src = match resolve_entity(&conn, source) { Some(s) => s, None => return json!({"error": format!("Entity not found: {}", source)}) };
    let tgt = match resolve_entity(&conn, target) { Some(t) => t, None => return json!({"error": format!("Entity not found: {}", target)}) };

    let mut visited: HashMap<String, Option<(String, Value)>> = HashMap::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    visited.insert(src.clone(), None);
    queue.push_back((src.clone(), 0));

    while let Some((current, d)) = queue.pop_front() {
        if d >= max_depth { continue; }
        let mut stmt = conn.prepare("SELECT id, source_id, target_id, relation, weight FROM edges WHERE source_id=?1 OR target_id=?1").unwrap();
        let edges: Vec<(i64, String, String, String, f64)> = stmt.query_map(params![current], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        }).unwrap().flatten().collect();

        for (eid, sid, tid, rel, w) in edges {
            let neighbor = if sid == current { &tid } else { &sid };
            if !visited.contains_key(neighbor.as_str()) {
                let edge_info = json!({"id": eid, "source_id": sid, "target_id": tid, "relation": rel, "weight": w});
                visited.insert(neighbor.clone(), Some((current.clone(), edge_info)));
                if *neighbor == tgt {
                    // Reconstruct path
                    let mut path_nodes = VecDeque::new();
                    let mut path_edges = VecDeque::new();
                    let mut node = tgt.clone();
                    while let Some(Some((prev, edge))) = visited.get(&node) {
                        let nrow: Option<Value> = conn.query_row("SELECT name, type FROM entities WHERE id=?1",
                            params![node], |r| Ok(json!({"id": &node, "name": r.get::<_, String>(0)?, "type": r.get::<_, String>(1)?}))).ok();
                        if let Some(n) = nrow { path_nodes.push_front(n); }
                        path_edges.push_front(edge.clone());
                        node = prev.clone();
                    }
                    let nrow: Option<Value> = conn.query_row("SELECT name, type FROM entities WHERE id=?1",
                        params![src], |r| Ok(json!({"id": &src, "name": r.get::<_, String>(0)?, "type": r.get::<_, String>(1)?}))).ok();
                    if let Some(n) = nrow { path_nodes.push_front(n); }
                    return json!({"found": true, "hops": path_edges.len(), "path_nodes": path_nodes, "path_edges": path_edges});
                }
                queue.push_back((neighbor.clone(), d + 1));
            }
        }
    }
    json!({"found": false, "source": source, "target": target, "max_depth": max_depth})
}

pub fn kgraph_search(query: &str, entity_type: &str, limit: u32) -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let like = format!("%{}%", query);
    let sql = if entity_type.is_empty() {
        "SELECT * FROM entities WHERE name LIKE ?1 OR properties LIKE ?1 ORDER BY updated_at DESC LIMIT ?2"
    } else {
        "SELECT * FROM entities WHERE (name LIKE ?1 OR properties LIKE ?1) AND type = ?3 ORDER BY updated_at DESC LIMIT ?2"
    };
    let mut stmt = conn.prepare(sql).unwrap();
    let results: Vec<Value> = if entity_type.is_empty() {
        stmt.query_map(params![like, limit], |r| {
            Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?, "properties": r.get::<_, String>(3).unwrap_or_default()}))
        }).unwrap().flatten().collect()
    } else {
        stmt.query_map(params![like, limit, entity_type], |r| {
            Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?, "properties": r.get::<_, String>(3).unwrap_or_default()}))
        }).unwrap().flatten().collect()
    };
    let count = results.len();
    json!({"query": query, "count": count, "results": results})
}

pub fn kgraph_delete(entity_id: &str, edge_source: &str, edge_target: &str, edge_relation: &str) -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    if !entity_id.is_empty() {
        let eid = match resolve_entity(&conn, entity_id) { Some(e) => e, None => return json!({"error": format!("Entity not found: {}", entity_id)}) };
        let _ = conn.execute("DELETE FROM entities WHERE id=?1", params![eid]);
        return json!({"deleted_entity": eid});
    }
    if !edge_source.is_empty() && !edge_target.is_empty() {
        let src = match resolve_entity(&conn, edge_source) { Some(s) => s, None => return json!({"error": "Source not found"}) };
        let tgt = match resolve_entity(&conn, edge_target) { Some(t) => t, None => return json!({"error": "Target not found"}) };
        if !edge_relation.is_empty() {
            let _ = conn.execute("DELETE FROM edges WHERE source_id=?1 AND target_id=?2 AND relation=?3", params![src, tgt, edge_relation]);
        } else {
            let _ = conn.execute("DELETE FROM edges WHERE source_id=?1 AND target_id=?2", params![src, tgt]);
        }
        return json!({"deleted_edge": format!("{} -> {}", edge_source, edge_target)});
    }
    json!({"error": "Provide entity_id or (edge_source + edge_target)"})
}

pub fn kgraph_stats() -> Value {
    let conn = match kgraph_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let entity_count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0)).unwrap_or(0);
    let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
    let obs_count: i64 = conn.query_row("SELECT COUNT(*) FROM observations", [], |r| r.get(0)).unwrap_or(0);

    let mut by_type = serde_json::Map::new();
    let mut stmt = conn.prepare("SELECT type, COUNT(*) FROM entities GROUP BY type ORDER BY COUNT(*) DESC").unwrap();
    let _ = stmt.query_map([], |r| {
        let t: String = r.get(0)?; let c: i64 = r.get(1)?;
        by_type.insert(t, json!(c)); Ok(())
    }).unwrap().for_each(|_| {});

    let mut by_relation = serde_json::Map::new();
    let mut stmt = conn.prepare("SELECT relation, COUNT(*) FROM edges GROUP BY relation ORDER BY COUNT(*) DESC").unwrap();
    let _ = stmt.query_map([], |r| {
        let t: String = r.get(0)?; let c: i64 = r.get(1)?;
        by_relation.insert(t, json!(c)); Ok(())
    }).unwrap().for_each(|_| {});

    json!({"entities": entity_count, "edges": edge_count, "observations": obs_count, "by_type": by_type, "by_relation": by_relation})
}

// ═══════════════════════════════════════════════════════════════════════
// SESSION REPLAY — Searchable index over Claude Code sessions (7 tools)
// DB: ~/.claude/experience/replay_index.db
// ═══════════════════════════════════════════════════════════════════════

const REPLAY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    file_path TEXT PRIMARY KEY, session_id TEXT, project_path TEXT NOT NULL,
    started_at TEXT, ended_at TEXT, message_count INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0, error_count INTEGER DEFAULT 0,
    model TEXT, title TEXT, git_branch TEXT,
    file_size INTEGER DEFAULT 0, file_mtime REAL DEFAULT 0, indexed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY, file_path TEXT NOT NULL, role TEXT NOT NULL,
    timestamp TEXT, content_preview TEXT, tool_name TEXT,
    is_error INTEGER DEFAULT 0, line_number INTEGER DEFAULT 0,
    FOREIGN KEY (file_path) REFERENCES sessions(file_path) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_messages_file ON messages(file_path);
CREATE INDEX IF NOT EXISTS idx_messages_tool ON messages(tool_name);
CREATE INDEX IF NOT EXISTS idx_messages_error ON messages(is_error);
"#;

fn replay_db() -> Result<Connection, String> {
    let dir = config::home_dir().join(".claude").join("experience");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("replay_index.db");
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").map_err(|e| e.to_string())?;
    conn.execute_batch(REPLAY_SCHEMA).map_err(|e| e.to_string())?;
    Ok(conn)
}

fn decode_project_path(dir_name: &str) -> String {
    if !dir_name.starts_with('-') { return dir_name.to_string(); }
    format!("/{}", &dir_name[1..].replace('-', "/"))
}

fn discover_sessions(project_filter: &str) -> Vec<(String, String, u64, f64)> {
    let sessions_dir = config::home_dir().join(".claude").join("projects");
    if !sessions_dir.exists() { return vec![]; }
    let mut results = vec![];
    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() { continue; }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let project_path = decode_project_path(&dir_name);
            if !project_filter.is_empty() && !project_path.contains(project_filter) { continue; }
            if let Ok(files) = std::fs::read_dir(entry.path()) {
                for f in files.flatten() {
                    if f.path().extension().map_or(true, |e| e != "jsonl") { continue; }
                    if let Ok(meta) = f.metadata() {
                        let mtime = meta.modified().ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs_f64()).unwrap_or(0.0);
                        results.push((f.path().to_string_lossy().to_string(), project_path.clone(), meta.len(), mtime));
                    }
                }
            }
        }
    }
    results
}

fn parse_session_jsonl(path: &str, max_messages: usize) -> Value {
    let file = match std::fs::File::open(path) { Ok(f) => f, Err(_) => return json!({"turns": []}) };
    let reader = std::io::BufReader::new(file);
    let mut meta = json!({"session_id": "", "project_path": "", "model": "", "started_at": "", "ended_at": "", "title": "", "git_branch": ""});
    let mut turns: Vec<Value> = vec![];
    let mut assistant_msgs: HashMap<String, Value> = HashMap::new();
    let mut last_ts = String::new();
    let mut line_num = 0u32;

    for line in reader.lines() {
        line_num += 1;
        let line = match line { Ok(l) => l, Err(_) => continue };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        let entry: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let entry_type = entry["type"].as_str().unwrap_or("");
        let ts = entry["timestamp"].as_str().unwrap_or("").to_string();
        if !ts.is_empty() { last_ts = ts.clone(); }
        if meta["session_id"].as_str() == Some("") { if let Some(sid) = entry["sessionId"].as_str() { meta["session_id"] = json!(sid); } }
        if meta["project_path"].as_str() == Some("") { if let Some(cwd) = entry["cwd"].as_str() { meta["project_path"] = json!(cwd); } }
        if meta["started_at"].as_str() == Some("") && !ts.is_empty() { meta["started_at"] = json!(&ts); }
        if let Some(br) = entry["gitBranch"].as_str() { meta["git_branch"] = json!(br); }
        if entry_type == "summary" { meta["title"] = json!(entry["summary"].as_str().unwrap_or("")); continue; }
        if entry_type == "system" || entry_type == "progress" { continue; }

        if entry_type == "assistant" {
            let msg = &entry["message"];
            let msg_id = msg["id"].as_str().unwrap_or("").to_string();
            if let Some(model) = msg["model"].as_str() { meta["model"] = json!(model); }
            let am = assistant_msgs.entry(msg_id.clone()).or_insert_with(|| json!({"role":"assistant","text":[],"tool_calls":[],"timestamp":&ts,"line_number":line_num}));
            if let Some(content) = msg["content"].as_array() {
                for block in content {
                    match block["type"].as_str().unwrap_or("") {
                        "text" => { if let Some(arr) = am["text"].as_array_mut() { arr.push(json!(block["text"].as_str().unwrap_or(""))); } }
                        "tool_use" => { if let Some(arr) = am["tool_calls"].as_array_mut() {
                            let inp = &block["input"];
                            let mut summary = serde_json::Map::new();
                            if let Some(obj) = inp.as_object() {
                                for key in &["command","query","pattern","file_path","content","prompt"] {
                                    if let Some(v) = obj.get(*key) {
                                        let s = v.as_str().unwrap_or("").chars().take(500).collect::<String>();
                                        summary.insert(key.to_string(), json!(s));
                                    }
                                }
                            }
                            arr.push(json!({"id": block["id"], "tool": block["name"], "input": summary}));
                        }}
                        _ => {}
                    }
                }
            }
            if msg["stop_reason"].is_string() {
                if let Some(am) = assistant_msgs.remove(&msg_id) { turns.push(am); }
            }
        } else if entry_type == "user" {
            if entry["isMeta"].as_bool().unwrap_or(false) { continue; }
            let content = &entry["message"]["content"];
            if let Some(arr) = content.as_array() {
                for item in arr {
                    if item["type"].as_str() == Some("tool_result") {
                        let preview: String = item["content"].as_str().unwrap_or("").chars().take(500).collect();
                        let is_error = item["is_error"].as_bool().unwrap_or(false);
                        turns.push(json!({"role":"tool_result","is_error":is_error,"content":preview,"timestamp":&ts,"line_number":line_num}));
                    }
                }
            } else if let Some(text) = content.as_str() {
                if !text.trim().is_empty() { turns.push(json!({"role":"user","text":text,"timestamp":&ts,"line_number":line_num})); }
            }
        }
        if max_messages > 0 && turns.len() >= max_messages { break; }
    }
    for (_, am) in assistant_msgs { turns.push(am); }
    meta["ended_at"] = json!(last_ts);
    let tc_count: usize = turns.iter().filter(|t| t["role"].as_str() == Some("assistant"))
        .map(|t| t["tool_calls"].as_array().map_or(0, |a| a.len())).sum();
    let err_count = turns.iter().filter(|t| t["is_error"].as_bool().unwrap_or(false)).count();
    json!({"file_path": path, "session_id": meta["session_id"], "project_path": meta["project_path"],
        "model": meta["model"], "started_at": meta["started_at"], "ended_at": meta["ended_at"],
        "title": meta["title"], "git_branch": meta["git_branch"],
        "turn_count": turns.len(), "tool_call_count": tc_count, "error_count": err_count, "turns": turns})
}

pub fn replay_index(force: bool, project: &str) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let files = discover_sessions(project);
    let mut stats = json!({"total_files": files.len(), "new_indexed": 0, "updated": 0, "skipped": 0, "errors": 0});

    for (fpath, proj_path, fsize, fmtime) in &files {
        if *fsize < 50 { stats["skipped"] = json!(stats["skipped"].as_u64().unwrap_or(0) + 1); continue; }
        if !force {
            if let Ok(row) = conn.query_row("SELECT file_size, file_mtime FROM sessions WHERE file_path=?1",
                params![fpath], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))) {
                if row.0 as u64 == *fsize && (row.1 - fmtime).abs() < 0.01 {
                    stats["skipped"] = json!(stats["skipped"].as_u64().unwrap_or(0) + 1); continue;
                }
            }
        }
        let session = parse_session_jsonl(fpath, 500);
        if session["turns"].as_array().map_or(true, |a| a.is_empty()) {
            stats["skipped"] = json!(stats["skipped"].as_u64().unwrap_or(0) + 1); continue;
        }
        let session_id = session["session_id"].as_str().unwrap_or("").to_string();
        let sid = if session_id.is_empty() { std::path::Path::new(fpath).file_stem().unwrap().to_string_lossy().to_string() } else { session_id };
        let now = now_iso();

        let existing: bool = conn.query_row("SELECT COUNT(*) FROM sessions WHERE file_path=?1", params![fpath], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
        if existing {
            let _ = conn.execute("DELETE FROM messages WHERE file_path=?1", params![fpath]);
            let _ = conn.execute("DELETE FROM sessions WHERE file_path=?1", params![fpath]);
            stats["updated"] = json!(stats["updated"].as_u64().unwrap_or(0) + 1);
        } else {
            stats["new_indexed"] = json!(stats["new_indexed"].as_u64().unwrap_or(0) + 1);
        }

        let _ = conn.execute(
            "INSERT INTO sessions (file_path,session_id,project_path,started_at,ended_at,message_count,tool_call_count,error_count,model,title,git_branch,file_size,file_mtime,indexed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![fpath, sid, session["project_path"].as_str().unwrap_or(proj_path),
                session["started_at"].as_str().unwrap_or(""), session["ended_at"].as_str().unwrap_or(""),
                session["turn_count"].as_i64().unwrap_or(0), session["tool_call_count"].as_i64().unwrap_or(0),
                session["error_count"].as_i64().unwrap_or(0), session["model"].as_str().unwrap_or(""),
                session["title"].as_str().unwrap_or(""), session["git_branch"].as_str().unwrap_or(""),
                *fsize as i64, *fmtime, now]);

        if let Some(turns) = session["turns"].as_array() {
            for turn in turns {
                let role = turn["role"].as_str().unwrap_or("");
                let ts = turn["timestamp"].as_str().unwrap_or("");
                let ln = turn["line_number"].as_i64().unwrap_or(0);
                if role == "assistant" {
                    if let Some(tcs) = turn["tool_calls"].as_array() {
                        for tc in tcs {
                            let tool_name = tc["tool"].as_str().unwrap_or("");
                            let preview: String = format!("tool:{}", tool_name).chars().take(500).collect();
                            let mid = gen_id("msg");
                            let _ = conn.execute("INSERT INTO messages (id,file_path,role,timestamp,content_preview,tool_name,is_error,line_number) VALUES (?1,?2,?3,?4,?5,?6,0,?7)",
                                params![mid, fpath, role, ts, preview, tool_name, ln]);
                        }
                    }
                } else if role == "tool_result" {
                    let preview: String = turn["content"].as_str().unwrap_or("").chars().take(500).collect();
                    let is_err = if turn["is_error"].as_bool().unwrap_or(false) { 1 } else { 0 };
                    let mid = gen_id("msg");
                    let _ = conn.execute("INSERT INTO messages (id,file_path,role,timestamp,content_preview,tool_name,is_error,line_number) VALUES (?1,?2,?3,?4,?5,NULL,?6,?7)",
                        params![mid, fpath, role, ts, preview, is_err, ln]);
                } else if role == "user" {
                    let preview: String = turn["text"].as_str().unwrap_or("").chars().take(500).collect();
                    let mid = gen_id("msg");
                    let _ = conn.execute("INSERT INTO messages (id,file_path,role,timestamp,content_preview,tool_name,is_error,line_number) VALUES (?1,?2,?3,?4,?5,NULL,0,?6)",
                        params![mid, fpath, role, ts, preview, ln]);
                }
            }
        }
    }
    let _ = conn.execute_batch(""); // ensure commit
    stats
}

fn days_ago(days: u32) -> String {
    let dt = Utc::now() - chrono::Duration::days(days as i64);
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn replay_search(query: &str, project: &str, tool: &str, limit: u32, days: u32) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let like = format!("%{}%", query);
    let mut sql = "SELECT m.content_preview, m.role, m.tool_name, m.is_error, m.timestamp, m.line_number, s.session_id, s.project_path, s.title, s.file_path FROM messages m JOIN sessions s ON s.file_path = m.file_path WHERE m.content_preview LIKE ?1".to_string();
    let mut param_count = 1;
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(like)];
    if !tool.is_empty() { param_count += 1; sql += &format!(" AND m.tool_name = ?{}", param_count); params_vec.push(Box::new(tool.to_string())); }
    if !project.is_empty() { param_count += 1; sql += &format!(" AND s.project_path LIKE ?{}", param_count); params_vec.push(Box::new(format!("%{}%", project))); }
    if days > 0 { param_count += 1; sql += &format!(" AND m.timestamp >= ?{}", param_count); params_vec.push(Box::new(days_ago(days))); }
    param_count += 1; sql += &format!(" ORDER BY m.timestamp DESC LIMIT ?{}", param_count);
    params_vec.push(Box::new(limit));

    let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(e) => return json!({"error": e.to_string()}) };
    let results: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |r| {
        Ok(json!({"content": r.get::<_, String>(0)?, "role": r.get::<_, String>(1)?, "tool": r.get::<_, Option<String>>(2)?,
            "is_error": r.get::<_, bool>(3)?, "timestamp": r.get::<_, String>(4)?, "line_number": r.get::<_, i64>(5)?,
            "session_id": r.get::<_, String>(6)?, "project": r.get::<_, String>(7)?, "session_title": r.get::<_, String>(8)?}))
    }).unwrap().flatten().collect();
    json!(results)
}

pub fn replay_session(session_id: &str, include_tools: bool, include_errors: bool, max_messages: u32) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let file_path: Option<String> = conn.query_row("SELECT file_path FROM sessions WHERE file_path=?1 OR session_id=?1",
        params![session_id], |r| r.get(0)).ok();
    let fp = match file_path { Some(p) => p, None => return json!({"error": format!("Session not found: {}", session_id)}) };
    if !std::path::Path::new(&fp).exists() { return json!({"error": format!("Session file not found: {}", fp)}); }
    let mut session = parse_session_jsonl(&fp, max_messages as usize);
    if let Some(turns) = session["turns"].as_array_mut() {
        if !include_tools { turns.retain(|t| t["role"].as_str() != Some("tool_result")); }
        if !include_errors { turns.retain(|t| !t["is_error"].as_bool().unwrap_or(false)); }
    }
    session
}

pub fn replay_list_sessions(project: &str, days: u32, limit: u32) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let since = days_ago(days);
    let mut sql = "SELECT file_path, session_id, project_path, started_at, ended_at, message_count, tool_call_count, error_count, model, title FROM sessions WHERE started_at >= ?1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(since)];
    if !project.is_empty() { sql += " AND project_path LIKE ?2"; params_vec.push(Box::new(format!("%{}%", project))); }
    sql += &format!(" ORDER BY started_at DESC LIMIT ?{}", params_vec.len() + 1);
    params_vec.push(Box::new(limit));

    let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(e) => return json!({"error": e.to_string()}) };
    let results: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |r| {
        Ok(json!({"file_path": r.get::<_, String>(0)?, "session_id": r.get::<_, String>(1)?, "project_path": r.get::<_, String>(2)?,
            "started_at": r.get::<_, String>(3)?, "ended_at": r.get::<_, String>(4)?, "messages": r.get::<_, i64>(5)?,
            "tool_calls": r.get::<_, i64>(6)?, "errors": r.get::<_, i64>(7)?, "model": r.get::<_, String>(8)?, "title": r.get::<_, String>(9)?}))
    }).unwrap().flatten().collect();
    json!(results)
}

pub fn replay_tool_history(tool_name: &str, limit: u32, days: u32) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let mut sql = "SELECT m.content_preview, m.is_error, m.timestamp, m.line_number, s.session_id, s.project_path, s.title FROM messages m JOIN sessions s ON s.file_path=m.file_path WHERE m.tool_name=?1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(tool_name.to_string())];
    if days > 0 { sql += " AND m.timestamp >= ?2"; params_vec.push(Box::new(days_ago(days))); }
    sql += &format!(" ORDER BY m.timestamp DESC LIMIT ?{}", params_vec.len() + 1);
    params_vec.push(Box::new(limit));

    let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(e) => return json!({"error": e.to_string()}) };
    let results: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |r| {
        Ok(json!({"content": r.get::<_, String>(0)?, "is_error": r.get::<_, bool>(1)?, "timestamp": r.get::<_, String>(2)?,
            "line_number": r.get::<_, i64>(3)?, "session_id": r.get::<_, String>(4)?, "project": r.get::<_, String>(5)?, "title": r.get::<_, String>(6)?}))
    }).unwrap().flatten().collect();
    json!(results)
}

pub fn replay_errors(project: &str, days: u32, limit: u32) -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let since = days_ago(days);
    let mut sql = "SELECT m.content_preview, m.tool_name, m.timestamp, m.line_number, s.session_id, s.project_path, s.title FROM messages m JOIN sessions s ON s.file_path=m.file_path WHERE m.is_error=1 AND m.timestamp >= ?1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(since)];
    if !project.is_empty() { sql += " AND s.project_path LIKE ?2"; params_vec.push(Box::new(format!("%{}%", project))); }
    sql += &format!(" ORDER BY m.timestamp DESC LIMIT ?{}", params_vec.len() + 1);
    params_vec.push(Box::new(limit));

    let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(e) => return json!({"error": e.to_string()}) };
    let results: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |r| {
        Ok(json!({"error_content": r.get::<_, String>(0)?, "tool": r.get::<_, Option<String>>(1)?, "timestamp": r.get::<_, String>(2)?,
            "line_number": r.get::<_, i64>(3)?, "session_id": r.get::<_, String>(4)?, "project": r.get::<_, String>(5)?, "title": r.get::<_, String>(6)?}))
    }).unwrap().flatten().collect();
    json!(results)
}

pub fn replay_status() -> Value {
    let conn = match replay_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let session_count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap_or(0);
    let message_count: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0)).unwrap_or(0);
    let error_count: i64 = conn.query_row("SELECT COUNT(*) FROM messages WHERE is_error=1", [], |r| r.get(0)).unwrap_or(0);
    let latest: String = conn.query_row("SELECT MAX(indexed_at) FROM sessions", [], |r| r.get(0)).unwrap_or_default();
    let all_files = discover_sessions("");
    json!({"status": "ok", "sessions": session_count, "messages": message_count, "errors": error_count,
        "unindexed_files": (all_files.len() as i64 - session_count).max(0), "last_indexed": latest})
}

// ═══════════════════════════════════════════════════════════════════════
// TRUTHGUARD — Immutable fact registry with contradiction detection (8 tools)
// DB: ~/.truthguard/truthguard.db
// ═══════════════════════════════════════════════════════════════════════

const TG_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS facts (
    id TEXT PRIMARY KEY, category TEXT NOT NULL, key TEXT NOT NULL,
    value TEXT NOT NULL, confidence REAL NOT NULL DEFAULT 1.0,
    source TEXT NOT NULL DEFAULT 'manual', aliases TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
    UNIQUE(category, key)
);
CREATE INDEX IF NOT EXISTS idx_facts_category ON facts(category);
CREATE INDEX IF NOT EXISTS idx_facts_key ON facts(key);
CREATE TABLE IF NOT EXISTS fact_checks (
    id TEXT PRIMARY KEY, claim_text TEXT NOT NULL, fact_id TEXT,
    verdict TEXT NOT NULL, matched_key TEXT, matched_value TEXT,
    claim_value TEXT, confidence REAL NOT NULL DEFAULT 0.0, checked_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT, fact_id TEXT NOT NULL,
    action TEXT NOT NULL, old_value TEXT, new_value TEXT,
    actor TEXT NOT NULL DEFAULT 'system', timestamp TEXT NOT NULL
);
"#;

fn tg_db() -> Result<Connection, String> {
    let dir = config::home_dir().join(".truthguard");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("truthguard.db");
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL;").map_err(|e| e.to_string())?;
    conn.execute_batch(TG_SCHEMA).map_err(|e| e.to_string())?;
    Ok(conn)
}

const VALID_CATEGORIES: &[&str] = &["identity", "project", "business", "technical", "preference"];

pub fn fact_add(category: &str, key: &str, value: &str, confidence: f64, source: &str, aliases: &[String], tags: &[String]) -> Value {
    if !VALID_CATEGORIES.contains(&category) {
        return json!({"error": format!("Invalid category '{}'. Must be one of: {:?}", category, VALID_CATEGORIES)});
    }
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let exists: bool = conn.query_row("SELECT COUNT(*) FROM facts WHERE category=?1 AND key=?2",
        params![category, key], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if exists { return json!({"error": format!("Fact '{}:{}' already exists. Use fact_update.", category, key)}); }

    let fact_id = gen_id("fact");
    let now = now_iso();
    let aliases_json = serde_json::to_string(aliases).unwrap_or_else(|_| "[]".into());
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".into());
    let source = if source.is_empty() { "manual" } else { source };
    let _ = conn.execute("INSERT INTO facts (id,category,key,value,confidence,source,aliases,tags,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![fact_id, category, key, value, confidence, source, aliases_json, tags_json, now, now]);
    let _ = conn.execute("INSERT INTO audit_log (fact_id,action,new_value,actor,timestamp) VALUES (?1,'created',?2,?3,?4)",
        params![fact_id, value, source, now]);
    json!({"id": fact_id, "category": category, "key": key, "value": value, "confidence": confidence, "status": "created"})
}

pub fn fact_get(fact_id: &str, key: &str, category: &str) -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let row: Option<(String, String, String, String, f64, String, String, String)> = if !fact_id.is_empty() {
        conn.query_row("SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE id=?1",
            params![fact_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).ok()
    } else if !key.is_empty() && !category.is_empty() {
        conn.query_row("SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE category=?1 AND key=?2",
            params![category, key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).ok()
    } else if !key.is_empty() {
        conn.query_row("SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE key=?1",
            params![key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).ok()
    } else { None };
    match row {
        Some((id, cat, k, v, conf, src, al, tg)) => {
            let aliases: Value = serde_json::from_str(&al).unwrap_or(json!([]));
            let tags: Value = serde_json::from_str(&tg).unwrap_or(json!([]));
            json!({"id": id, "category": cat, "key": k, "value": v, "confidence": conf, "source": src, "aliases": aliases, "tags": tags})
        }
        None => json!({"error": "Fact not found"})
    }
}

pub fn fact_search(query: &str, category: &str, min_confidence: f64, limit: u32) -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let like = format!("%{}%", query);
    let sql = if !query.is_empty() && !category.is_empty() {
        "SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE (key LIKE ?1 OR value LIKE ?1 OR aliases LIKE ?1) AND category=?2 AND confidence>=?3 ORDER BY confidence DESC LIMIT ?4"
    } else if !query.is_empty() {
        "SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE (key LIKE ?1 OR value LIKE ?1 OR aliases LIKE ?1) AND confidence>=?3 ORDER BY confidence DESC LIMIT ?4"
    } else if !category.is_empty() {
        "SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE category=?2 AND confidence>=?3 ORDER BY confidence DESC LIMIT ?4"
    } else {
        "SELECT id,category,key,value,confidence,source,aliases,tags FROM facts WHERE confidence>=?3 ORDER BY confidence DESC LIMIT ?4"
    };
    let mut stmt = conn.prepare(sql).unwrap();
    let results: Vec<Value> = stmt.query_map(params![like, category, min_confidence, limit], |r| {
        Ok(json!({"id": r.get::<_, String>(0)?, "category": r.get::<_, String>(1)?, "key": r.get::<_, String>(2)?,
            "value": r.get::<_, String>(3)?, "confidence": r.get::<_, f64>(4)?, "source": r.get::<_, String>(5)?}))
    }).unwrap().flatten().collect();
    let count = results.len();
    json!({"facts": results, "count": count})
}

pub fn fact_check(claim: &str) -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let entities = extract_entities(claim);
    let mut results = vec![];

    for entity in &entities {
        let ev = entity["value"].as_str().unwrap_or("").to_lowercase();
        if ev.len() < 2 { continue; }
        let like = format!("%{}%", ev);
        let mut stmt = conn.prepare("SELECT id,category,key,value,confidence FROM facts WHERE LOWER(value)=?1 OR LOWER(key)=?1 OR value LIKE ?2 OR aliases LIKE ?2").unwrap();
        let facts: Vec<(String, String, String, String, f64)> = stmt.query_map(params![ev, like], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        }).unwrap().flatten().collect();

        for (fid, _cat, fkey, fval, fconf) in &facts {
            let fval_lower = fval.to_lowercase();
            let verdict = if ev == fval_lower { "match" }
                else if claim.to_lowercase().contains(&fval_lower) { "match" }
                else { "partial_match" };
            let check_id = gen_id("chk");
            let _ = conn.execute("INSERT INTO fact_checks (id,claim_text,fact_id,verdict,matched_key,matched_value,claim_value,confidence,checked_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![check_id, &claim[..claim.len().min(500)], fid, verdict, fkey, fval, ev, fconf, now_iso()]);
            results.push(json!({"check_id": check_id, "entity": ev, "matched_fact": {"key": fkey, "value": fval, "confidence": fconf}, "verdict": verdict}));
        }
    }
    let contradictions = results.iter().filter(|r| r["verdict"].as_str() == Some("contradiction")).count();
    let matches = results.iter().filter(|r| r["verdict"].as_str() == Some("match")).count();
    json!({"claim": &claim[..claim.len().min(200)], "entities_found": entities.len(), "facts_matched": results.len(),
        "contradictions": contradictions, "matches": matches, "pass": contradictions == 0, "results": results})
}

pub fn fact_check_response(response_text: &str) -> Value {
    let re = Regex::new(r"(?<=[.!?])\s+|\n+").unwrap();
    let sentences: Vec<&str> = re.split(response_text).filter(|s| s.len() > 10).take(100).collect();
    let mut flagged = vec![];
    let mut total_contradictions = 0u32;
    let mut total_matches = 0u32;

    for sentence in &sentences {
        let result = fact_check(sentence);
        let c = result["contradictions"].as_u64().unwrap_or(0) as u32;
        let m = result["matches"].as_u64().unwrap_or(0) as u32;
        total_contradictions += c;
        total_matches += m;
        if c > 0 { flagged.push(json!({"sentence": &sentence[..sentence.len().min(120)], "contradictions": c, "details": result["results"]})); }
    }
    json!({"pass": total_contradictions == 0, "sentences_checked": sentences.len(), "total_contradictions": total_contradictions,
        "total_matches": total_matches, "flagged_sentences": flagged})
}

pub fn fact_update(fact_id: &str, category: &str, key: &str, value: &str, confidence: f64, aliases: &[String], source: &str, tags: &[String]) -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let row: Option<String> = if !fact_id.is_empty() {
        conn.query_row("SELECT id FROM facts WHERE id=?1", params![fact_id], |r| r.get(0)).ok()
    } else if !category.is_empty() && !key.is_empty() {
        conn.query_row("SELECT id FROM facts WHERE category=?1 AND key=?2", params![category, key], |r| r.get(0)).ok()
    } else if !key.is_empty() {
        conn.query_row("SELECT id FROM facts WHERE key=?1", params![key], |r| r.get(0)).ok()
    } else { None };
    let fid = match row { Some(id) => id, None => return json!({"error": "Fact not found"}) };

    let mut updates = vec![];
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    if !value.is_empty() { updates.push("value=?"); params_vec.push(Box::new(value.to_string())); }
    if confidence >= 0.0 { updates.push("confidence=?"); params_vec.push(Box::new(confidence)); }
    if !aliases.is_empty() { updates.push("aliases=?"); params_vec.push(Box::new(serde_json::to_string(aliases).unwrap_or_else(|_| "[]".into()))); }
    if !source.is_empty() { updates.push("source=?"); params_vec.push(Box::new(source.to_string())); }
    if !tags.is_empty() { updates.push("tags=?"); params_vec.push(Box::new(serde_json::to_string(tags).unwrap_or_else(|_| "[]".into()))); }
    if updates.is_empty() { return json!({"error": "No fields to update"}); }
    updates.push("updated_at=?"); params_vec.push(Box::new(now_iso()));
    params_vec.push(Box::new(fid.clone()));

    let placeholders: Vec<String> = updates.iter().enumerate().map(|(i, u)| u.replace('?', &format!("?{}", i + 1))).collect();
    let sql = format!("UPDATE facts SET {} WHERE id=?{}", placeholders.join(","), params_vec.len());
    let _ = conn.execute(&sql, rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())));
    json!({"status": "updated", "fact_id": fid})
}

pub fn fact_delete(fact_id: &str, reason: &str) -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let exists: bool = conn.query_row("SELECT COUNT(*) FROM facts WHERE id=?1", params![fact_id], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !exists { return json!({"error": format!("Fact not found: {}", fact_id)}); }
    let _ = conn.execute("INSERT INTO audit_log (fact_id,action,new_value,actor,timestamp) VALUES (?1,'deleted',?2,'manual',?3)",
        params![fact_id, reason, now_iso()]);
    let _ = conn.execute("DELETE FROM facts WHERE id=?1", params![fact_id]);
    json!({"status": "deleted", "fact_id": fact_id})
}

pub fn truthguard_status() -> Value {
    let conn = match tg_db() { Ok(c) => c, Err(e) => return json!({"error": e}) };
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM facts", [], |r| r.get(0)).unwrap_or(0);
    let mut by_category = serde_json::Map::new();
    for cat in VALID_CATEGORIES {
        let c: i64 = conn.query_row("SELECT COUNT(*) FROM facts WHERE category=?1", params![cat], |r| r.get(0)).unwrap_or(0);
        by_category.insert(cat.to_string(), json!(c));
    }
    let total_checks: i64 = conn.query_row("SELECT COUNT(*) FROM fact_checks", [], |r| r.get(0)).unwrap_or(0);
    let contradictions: i64 = conn.query_row("SELECT COUNT(*) FROM fact_checks WHERE verdict='contradiction'", [], |r| r.get(0)).unwrap_or(0);
    json!({"status": "healthy", "total_facts": total, "facts_by_category": by_category, "total_checks": total_checks, "total_contradictions_found": contradictions})
}

// === ENTITY EXTRACTION (for fact_check) ===

fn extract_entities(text: &str) -> Vec<Value> {
    let mut entities = vec![];
    let mut seen = std::collections::HashSet::new();

    let patterns: Vec<(&str, &str)> = vec![
        (r"[\w.+-]+@[\w-]+\.[\w.]+", "email"),
        (r"[~/][\w./-]{3,}", "path"),
        (r"https?://[^\s,)]+", "url"),
        (r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+)+\b", "name"),
        (r#""([^"]{2,})""#, "quoted"),
        (r"\bv?\d+\.\d+(?:\.\d+)?\b", "version"),
    ];
    for (pat, etype) in &patterns {
        if let Ok(re) = Regex::new(pat) {
            for cap in re.find_iter(text) {
                let val = cap.as_str().to_string();
                if !seen.contains(&val) {
                    seen.insert(val.clone());
                    entities.push(json!({"type": etype, "value": val}));
                }
            }
        }
    }
    entities
}
