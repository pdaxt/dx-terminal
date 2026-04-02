use chrono::Local;
use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::config;

const LOCK_TIMEOUT_MINUTES: i64 = 30;

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn spaces_dir() -> PathBuf {
    let d = config::collab_root().join("spaces");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn meta_dir() -> PathBuf {
    let d = config::collab_root().join("meta");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn proposals_dir() -> PathBuf {
    let d = config::collab_root().join("proposals");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn doc_path(space: &str, name: &str) -> PathBuf {
    spaces_dir().join(space).join(format!("{}.md", name))
}

fn meta_path(space: &str, name: &str) -> PathBuf {
    meta_dir().join(space).join(format!("{}.json", name))
}

fn proposal_dir(space: &str, name: &str) -> PathBuf {
    proposals_dir().join(space).join(name)
}

fn load_meta(space: &str, name: &str) -> Value {
    let mp = meta_path(space, name);
    if mp.exists() {
        if let Ok(content) = std::fs::read_to_string(&mp) {
            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                return v;
            }
        }
    }
    json!({
        "space": space, "name": name, "status": "draft",
        "locked_by": null, "locked_at": null,
        "created_at": null, "updated_at": null,
        "comments": [], "tags": [],
    })
}

fn save_meta(space: &str, name: &str, meta: &Value) {
    let mp = meta_path(space, name);
    let _ = std::fs::create_dir_all(mp.parent().unwrap());
    let _ = std::fs::write(&mp, serde_json::to_string_pretty(meta).unwrap_or_default());
}

fn is_locked(meta: &mut Value, agent_id: &str) -> bool {
    let locked_by = meta["locked_by"].as_str().unwrap_or("").to_string();
    if locked_by.is_empty() {
        return false;
    }

    // Check expiry
    if let Some(locked_at) = meta["locked_at"].as_str() {
        if let Ok(lock_time) = chrono::NaiveDateTime::parse_from_str(locked_at, "%Y-%m-%dT%H:%M:%S")
        {
            let now = Local::now().naive_local();
            if now.signed_duration_since(lock_time).num_minutes() > LOCK_TIMEOUT_MINUTES {
                meta["locked_by"] = json!(null);
                meta["locked_at"] = json!(null);
                return false;
            }
        }
    }

    // Same agent holds lock → not locked for them
    if !agent_id.is_empty() && locked_by == agent_id {
        return false;
    }
    true
}

fn scan_directives(content: &str) -> Vec<Value> {
    let re = Regex::new(r"<!--\s*@claude:\s*(.*?)\s*-->").unwrap();
    let mut results = vec![];
    for cap in re.captures_iter(content) {
        let start = cap.get(0).unwrap().start();
        let line_num = content[..start].matches('\n').count() + 1;
        results.push(json!({
            "line": line_num,
            "directive": cap[1].trim(),
            "raw": &cap[0],
        }));
    }
    results
}

// === SPACE TOOLS ===

pub fn space_list() -> Value {
    let sd = spaces_dir();
    let mut spaces = vec![];
    if let Ok(entries) = std::fs::read_dir(&sd) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                let name = entry.file_name().to_string_lossy().to_string();
                let doc_count = std::fs::read_dir(entry.path())
                    .ok()
                    .map(|e| {
                        e.flatten()
                            .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                            .count()
                    })
                    .unwrap_or(0);
                spaces.push(json!({"name": name, "docs": doc_count}));
            }
        }
    }
    spaces.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    json!({"spaces": spaces, "root": config::collab_root().to_string_lossy()})
}

pub fn space_create(name: &str) -> Value {
    let name = name.to_lowercase().replace(' ', "-");
    let space_dir = spaces_dir().join(&name);
    let _ = std::fs::create_dir_all(&space_dir);
    let _ = std::fs::create_dir_all(meta_dir().join(&name));
    json!({"created": name, "path": space_dir.to_string_lossy()})
}

// === DOCUMENT TOOLS ===

pub fn doc_list(space: &str, status_filter: &str) -> Value {
    let sd = spaces_dir();
    let search_dirs: Vec<PathBuf> = if space.is_empty() {
        std::fs::read_dir(&sd)
            .ok()
            .map(|e| {
                e.flatten()
                    .filter(|f| f.path().is_dir())
                    .map(|f| f.path())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![sd.join(space)]
    };

    let mut docs = vec![];
    for space_dir in &search_dirs {
        if !space_dir.exists() {
            continue;
        }
        let sp = space_dir.file_name().unwrap().to_string_lossy().to_string();
        if let Ok(entries) = std::fs::read_dir(space_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "md") {
                    continue;
                }
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                let mut meta = load_meta(&sp, &name);
                is_locked(&mut meta, ""); // refresh lock status
                let effective_status = if meta["locked_by"].is_string() {
                    "locked"
                } else {
                    meta["status"].as_str().unwrap_or("draft")
                };
                if !status_filter.is_empty() && effective_status != status_filter {
                    continue;
                }

                let prop_dir = proposal_dir(&sp, &name);
                let proposals = std::fs::read_dir(&prop_dir)
                    .ok()
                    .map(|e| {
                        e.flatten()
                            .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                            .count()
                    })
                    .unwrap_or(0);

                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let directives = scan_directives(&content).len();

                docs.push(json!({
                    "space": sp, "name": name, "status": effective_status,
                    "locked_by": meta["locked_by"], "comments": meta["comments"].as_array().map_or(0, |c| c.len()),
                    "proposals": proposals, "directives": directives,
                    "updated": meta["updated_at"], "tags": meta["tags"],
                }));
            }
        }
    }
    let count = docs.len();
    json!({"docs": docs, "count": count})
}

pub fn doc_read(space: &str, name: &str, include_meta: bool) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let content = std::fs::read_to_string(&dp).unwrap_or_default();
    let mut result = json!({"space": space, "name": name, "content": content});
    if include_meta {
        let mut meta = load_meta(space, name);
        is_locked(&mut meta, "");
        result["meta"] = meta;
        result["directives"] = json!(scan_directives(&content));

        let pdir = proposal_dir(space, name);
        if pdir.exists() {
            let mut proposals = vec![];
            if let Ok(entries) = std::fs::read_dir(&pdir) {
                for entry in entries.flatten() {
                    if entry.path().extension().is_none_or(|e| e != "md") {
                        continue;
                    }
                    let id = entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let preview: String = std::fs::read_to_string(entry.path())
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect();
                    proposals.push(json!({"id": id, "preview": preview}));
                }
            }
            result["proposals"] = json!(proposals);
        }
    }
    result
}

pub fn doc_create(space: &str, name: &str, content: &str, status: &str, tags: &[String]) -> Value {
    let name = name.to_lowercase().replace(' ', "-");
    let space = space.to_lowercase().replace(' ', "-");
    let dp = doc_path(&space, &name);
    if dp.exists() {
        return json!({"error": format!("Doc already exists: {}/{}. Use doc_edit to modify.", space, name)});
    }
    let _ = std::fs::create_dir_all(dp.parent().unwrap());
    let _ = std::fs::write(&dp, content);

    let status = if status.is_empty() { "draft" } else { status };
    let mut meta = load_meta(&space, &name);
    meta["status"] = json!(status);
    meta["created_at"] = json!(now_iso());
    meta["updated_at"] = json!(now_iso());
    meta["tags"] = json!(tags);
    save_meta(&space, &name, &meta);

    json!({"created": format!("{}/{}", space, name), "path": dp.to_string_lossy(), "status": status})
}

pub fn doc_edit(space: &str, name: &str, content: &str, agent_id: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let mut meta = load_meta(space, name);
    if is_locked(&mut meta, agent_id) {
        return json!({
            "error": "LOCKED", "locked_by": meta["locked_by"], "locked_at": meta["locked_at"],
            "message": format!("Doc is locked by {}. Use doc_propose instead.", meta["locked_by"].as_str().unwrap_or("unknown")),
        });
    }
    let old_content = std::fs::read_to_string(&dp).unwrap_or_default();
    let old_hash = format!("{:08x}", crc32(&old_content));
    let _ = std::fs::write(&dp, content);
    meta["updated_at"] = json!(now_iso());
    save_meta(space, name, &meta);
    json!({"edited": format!("{}/{}", space, name), "previous_hash": old_hash, "status": meta["status"]})
}

pub fn crc32(s: &str) -> u32 {
    let mut hash: u32 = 0xFFFF_FFFF;
    for byte in s.bytes() {
        hash ^= byte as u32;
        for _ in 0..8 {
            hash = if hash & 1 != 0 {
                (hash >> 1) ^ 0xEDB8_8320
            } else {
                hash >> 1
            };
        }
    }
    !hash
}

pub fn doc_propose(space: &str, name: &str, content: &str, summary: &str, agent_id: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let pdir = proposal_dir(space, name);
    let _ = std::fs::create_dir_all(&pdir);

    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let proposal_id = if agent_id.is_empty() {
        ts.clone()
    } else {
        format!("{}_{}", ts, agent_id)
    };
    let prop_file = pdir.join(format!("{}.md", proposal_id));

    let header =
        format!(
        "<!-- PROPOSAL: {} -->\n<!-- Summary: {} -->\n<!-- Agent: {} -->\n<!-- Date: {} -->\n\n",
        proposal_id, summary, agent_id, now_iso()
    );
    let _ = std::fs::write(&prop_file, format!("{}{}", header, content));

    let mut meta = load_meta(space, name);
    meta["status"] = json!("review");
    meta["updated_at"] = json!(now_iso());
    save_meta(space, name, &meta);

    json!({"proposal_id": proposal_id, "doc": format!("{}/{}", space, name), "summary": summary, "status": "pending_review"})
}

pub fn doc_approve(space: &str, name: &str, proposal_id: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let pdir = proposal_dir(space, name);
    if !pdir.exists() {
        return json!({"error": format!("No proposals pending for {}/{}", space, name)});
    }

    let (prop_file, actual_id) = if proposal_id == "latest" || proposal_id.is_empty() {
        let mut files: Vec<_> = std::fs::read_dir(&pdir)
            .ok()
            .map(|e| {
                e.flatten()
                    .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                    .map(|f| f.path())
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        match files.last() {
            Some(f) => (
                f.clone(),
                f.file_stem().unwrap().to_string_lossy().to_string(),
            ),
            None => return json!({"error": "No proposals found"}),
        }
    } else {
        (
            pdir.join(format!("{}.md", proposal_id)),
            proposal_id.to_string(),
        )
    };

    if !prop_file.exists() {
        return json!({"error": format!("Proposal not found: {}", actual_id)});
    }

    // Strip header comments and merge
    let prop_content = std::fs::read_to_string(&prop_file).unwrap_or_default();
    let mut content_lines = vec![];
    let mut past_header = false;
    for line in prop_content.lines() {
        if past_header {
            content_lines.push(line);
        } else if !line.starts_with("<!-- ") {
            past_header = true;
            content_lines.push(line);
        }
    }
    let _ = std::fs::write(&dp, content_lines.join("\n"));
    let _ = std::fs::remove_file(&prop_file);

    let mut meta = load_meta(space, name);
    meta["status"] = json!("approved");
    meta["updated_at"] = json!(now_iso());
    let remaining = std::fs::read_dir(&pdir)
        .ok()
        .map(|e| {
            e.flatten()
                .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                .count()
        })
        .unwrap_or(0);
    if remaining > 0 {
        meta["status"] = json!("review");
    }
    save_meta(space, name, &meta);

    json!({"approved": actual_id, "doc": format!("{}/{}", space, name), "status": meta["status"], "remaining_proposals": remaining})
}

pub fn doc_reject(space: &str, name: &str, proposal_id: &str, reason: &str) -> Value {
    let pdir = proposal_dir(space, name);
    let prop_file = pdir.join(format!("{}.md", proposal_id));
    if !prop_file.exists() {
        return json!({"error": format!("Proposal not found: {}", proposal_id)});
    }
    let _ = std::fs::remove_file(&prop_file);

    let mut meta = load_meta(space, name);
    if let Some(comments) = meta["comments"].as_array_mut() {
        comments.push(json!({"author": "system", "text": format!("Proposal {} rejected. {}", proposal_id, reason), "timestamp": now_iso()}));
    }
    let remaining = std::fs::read_dir(&pdir)
        .ok()
        .map(|e| {
            e.flatten()
                .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
                .count()
        })
        .unwrap_or(0);
    if remaining == 0 && meta["status"].as_str() == Some("review") {
        meta["status"] = json!("draft");
    }
    save_meta(space, name, &meta);
    json!({"rejected": proposal_id, "reason": reason, "remaining_proposals": remaining})
}

// === LOCK TOOLS ===

pub fn doc_lock(space: &str, name: &str, locked_by: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let mut meta = load_meta(space, name);
    if is_locked(&mut meta, "") {
        return json!({"error": "Already locked", "locked_by": meta["locked_by"], "locked_at": meta["locked_at"]});
    }
    let locker = if locked_by.is_empty() {
        "human"
    } else {
        locked_by
    };
    meta["locked_by"] = json!(locker);
    meta["locked_at"] = json!(now_iso());
    save_meta(space, name, &meta);
    json!({"locked": format!("{}/{}", space, name), "locked_by": locker, "expires_in": format!("{} minutes", LOCK_TIMEOUT_MINUTES)})
}

pub fn doc_unlock(space: &str, name: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let mut meta = load_meta(space, name);
    let prev = meta["locked_by"].as_str().unwrap_or("").to_string();
    meta["locked_by"] = json!(null);
    meta["locked_at"] = json!(null);
    save_meta(space, name, &meta);
    json!({"unlocked": format!("{}/{}", space, name), "was_locked_by": prev})
}

// === COMMENT TOOLS ===

pub fn doc_comment(space: &str, name: &str, text: &str, author: &str, line: u32) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let mut meta = load_meta(space, name);
    let comment_id = meta["comments"].as_array().map_or(1, |c| c.len() + 1);
    let author = if author.is_empty() { "claude" } else { author };
    let comment = json!({"id": comment_id, "author": author, "text": text, "line": line, "timestamp": now_iso()});
    if let Some(comments) = meta["comments"].as_array_mut() {
        comments.push(comment.clone());
    }
    save_meta(space, name, &meta);
    json!({"comment_added": comment})
}

pub fn doc_comments(space: &str, name: &str) -> Value {
    let meta = load_meta(space, name);
    let comments = meta["comments"].as_array().cloned().unwrap_or_default();
    let count = comments.len();
    json!({"doc": format!("{}/{}", space, name), "comments": comments, "count": count})
}

// === STATUS & SEARCH ===

pub fn doc_status(space: &str, name: &str, status: &str) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let valid = ["draft", "review", "approved", "archived"];
    if !valid.contains(&status) {
        return json!({"error": format!("Invalid status. Must be one of: {:?}", valid)});
    }
    let mut meta = load_meta(space, name);
    let old_status = meta["status"].as_str().unwrap_or("draft").to_string();
    meta["status"] = json!(status);
    meta["updated_at"] = json!(now_iso());
    save_meta(space, name, &meta);
    json!({"doc": format!("{}/{}", space, name), "old_status": old_status, "new_status": status})
}

pub fn doc_search(query: &str, space: &str) -> Value {
    let sd = spaces_dir();
    let search_dirs: Vec<PathBuf> = if space.is_empty() {
        std::fs::read_dir(&sd)
            .ok()
            .map(|e| {
                e.flatten()
                    .filter(|f| f.path().is_dir())
                    .map(|f| f.path())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![sd.join(space)]
    };

    let query_lower = query.to_lowercase();
    let mut results = vec![];
    for space_dir in &search_dirs {
        if !space_dir.exists() {
            continue;
        }
        let sp = space_dir.file_name().unwrap().to_string_lossy().to_string();
        if let Ok(entries) = std::fs::read_dir(space_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_none_or(|e| e != "md") {
                    continue;
                }
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                if !content.to_lowercase().contains(&query_lower) {
                    continue;
                }
                let matches: Vec<Value> = content
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.to_lowercase().contains(&query_lower))
                    .take(5)
                    .map(|(i, line)| json!({"line": i + 1, "text": &line[..line.len().min(100)]}))
                    .collect();
                let total = content
                    .lines()
                    .filter(|l| l.to_lowercase().contains(&query_lower))
                    .count();
                let name = entry
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                results.push(
                    json!({"space": sp, "name": name, "matches": matches, "total_matches": total}),
                );
            }
        }
    }
    let count = results.len();
    json!({"query": query, "results": results, "count": count})
}

pub fn doc_directives(space: &str) -> Value {
    let sd = spaces_dir();
    let search_dirs: Vec<PathBuf> = if space.is_empty() {
        std::fs::read_dir(&sd)
            .ok()
            .map(|e| {
                e.flatten()
                    .filter(|f| f.path().is_dir())
                    .map(|f| f.path())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![sd.join(space)]
    };

    let mut all = vec![];
    for space_dir in &search_dirs {
        if !space_dir.exists() {
            continue;
        }
        let sp = space_dir.file_name().unwrap().to_string_lossy().to_string();
        if let Ok(entries) = std::fs::read_dir(space_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_none_or(|e| e != "md") {
                    continue;
                }
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let directives = scan_directives(&content);
                if !directives.is_empty() {
                    let doc = entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    for mut d in directives {
                        d["space"] = json!(&sp);
                        d["doc"] = json!(&doc);
                        all.push(d);
                    }
                }
            }
        }
    }
    let count = all.len();
    json!({"directives": all, "count": count})
}

pub fn doc_history(space: &str, name: &str, limit: u32) -> Value {
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let output = std::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", limit),
            "--pretty=format:%H|%ai|%s",
            "--follow",
            &format!("spaces/{}/{}.md", space, name),
        ])
        .current_dir(config::collab_root())
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let history: Vec<Value> = stdout.lines().filter(|l| l.contains('|')).map(|line| {
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                json!({
                    "hash": &parts.first().unwrap_or(&"")[..8.min(parts.first().unwrap_or(&"").len())],
                    "date": parts.get(1).unwrap_or(&"").trim(),
                    "message": parts.get(2).unwrap_or(&"").trim(),
                })
            }).collect();
            json!({"doc": format!("{}/{}", space, name), "history": history})
        }
        _ => {
            json!({"doc": format!("{}/{}", space, name), "history": [], "note": "Not a git repo or no history"})
        }
    }
}

pub fn doc_delete(space: &str, name: &str, confirm: bool) -> Value {
    if !confirm {
        return json!({"error": "Set confirm=true to delete."});
    }
    let dp = doc_path(space, name);
    if !dp.exists() {
        return json!({"error": format!("Doc not found: {}/{}", space, name)});
    }
    let _ = std::fs::remove_file(&dp);
    let mp = meta_path(space, name);
    if mp.exists() {
        let _ = std::fs::remove_file(&mp);
    }
    let pdir = proposal_dir(space, name);
    if pdir.exists() {
        let _ = std::fs::remove_dir_all(&pdir);
    }
    json!({"deleted": format!("{}/{}", space, name)})
}

// === INIT ===

pub fn collab_init() -> Value {
    let _ = std::fs::create_dir_all(spaces_dir());
    let _ = std::fs::create_dir_all(meta_dir());
    let _ = std::fs::create_dir_all(proposals_dir());

    let git_dir = config::collab_root().join(".git");
    let git_status = if git_dir.exists() {
        "exists"
    } else {
        match std::process::Command::new("git")
            .arg("init")
            .current_dir(config::collab_root())
            .output()
        {
            Ok(out) if out.status.success() => "initialized",
            _ => "not_configured",
        }
    };

    json!({
        "root": config::collab_root().to_string_lossy(),
        "spaces_dir": spaces_dir().to_string_lossy(),
        "meta_dir": meta_dir().to_string_lossy(),
        "proposals_dir": proposals_dir().to_string_lossy(),
        "git": git_status,
    })
}
