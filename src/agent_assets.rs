use serde_json::{json, Value};
use std::path::Path;
use walkdir::WalkDir;

pub fn collect_automation_assets(project_root: &str) -> Value {
    collect_automation_assets_with_home(Path::new(project_root), &crate::config::home_dir())
}

fn collect_automation_assets_with_home(project_root: &Path, home_root: &Path) -> Value {
    let project_commands = collect_command_assets(
        &project_root.join(".claude").join("commands"),
        "claude",
        "project",
    );
    let user_commands = collect_command_assets(
        &home_root.join(".claude").join("commands"),
        "claude",
        "user",
    );
    let project_skills = collect_skill_assets(
        &project_root.join(".codex").join("skills"),
        "codex",
        "project",
    );
    let user_skills =
        collect_skill_assets(&home_root.join(".codex").join("skills"), "codex", "user");
    let external_mcps = crate::external_mcp::load_external_descriptors()
        .into_iter()
        .map(|descriptor| {
            json!({
                "name": descriptor.name,
                "description": descriptor.description,
                "capabilities": descriptor.capabilities,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "commands": {
            "project": project_commands,
            "user": user_commands,
        },
        "skills": {
            "project": project_skills,
            "user": user_skills,
        },
        "external_mcps": external_mcps,
        "counts": {
            "project_commands": project_commands.len(),
            "user_commands": user_commands.len(),
            "project_skills": project_skills.len(),
            "user_skills": user_skills.len(),
            "external_mcps": external_mcps.len(),
        }
    })
}

fn collect_command_assets(dir: &Path, provider: &str, scope: &str) -> Vec<Value> {
    let mut assets = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return assets;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        assets.push(asset_json(
            &path,
            provider,
            scope,
            "command",
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("command"),
        ));
    }

    assets.sort_by(|left, right| compare_asset_name(left, right));
    assets
}

fn collect_skill_assets(dir: &Path, provider: &str, scope: &str) -> Vec<Value> {
    let mut assets = Vec::new();
    if !dir.exists() {
        return assets;
    }

    for entry in WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() || path.file_name().and_then(|value| value.to_str()) != Some("SKILL.md")
        {
            continue;
        }

        let skill_name = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("skill");
        assets.push(asset_json(path, provider, scope, "skill", skill_name));
    }

    assets.sort_by(|left, right| compare_asset_name(left, right));
    assets
}

fn asset_json(path: &Path, provider: &str, scope: &str, kind: &str, name: &str) -> Value {
    json!({
        "provider": provider,
        "scope": scope,
        "kind": kind,
        "name": name,
        "path": path.to_string_lossy(),
        "summary": read_summary(path),
    })
}

fn compare_asset_name(left: &Value, right: &Value) -> std::cmp::Ordering {
    left.get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .cmp(
            right
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(""),
        )
}

fn read_summary(path: &Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            return trimmed.trim_start_matches('#').trim().to_string();
        }
        return trimmed.to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collects_project_commands_and_skills() {
        let project = tempdir().unwrap();
        let home = tempdir().unwrap();

        let project_command_dir = project.path().join(".claude").join("commands");
        let project_skill_dir = project
            .path()
            .join(".codex")
            .join("skills")
            .join("reviewer");
        let user_skill_dir = home.path().join(".codex").join("skills").join("builder");

        std::fs::create_dir_all(&project_command_dir).unwrap();
        std::fs::create_dir_all(&project_skill_dir).unwrap();
        std::fs::create_dir_all(&user_skill_dir).unwrap();

        std::fs::write(
            project_command_dir.join("handoff.md"),
            "# Handoff\nProject handoff command",
        )
        .unwrap();
        std::fs::write(
            project_skill_dir.join("SKILL.md"),
            "# Reviewer\nReview workflows",
        )
        .unwrap();
        std::fs::write(user_skill_dir.join("SKILL.md"), "Builder skill").unwrap();

        let assets = collect_automation_assets_with_home(project.path(), home.path());

        assert_eq!(assets["counts"]["project_commands"], json!(1));
        assert_eq!(assets["counts"]["project_skills"], json!(1));
        assert_eq!(assets["counts"]["user_skills"], json!(1));
        assert_eq!(assets["commands"]["project"][0]["name"], json!("handoff"));
        assert_eq!(assets["skills"]["project"][0]["name"], json!("reviewer"));
        assert_eq!(
            assets["skills"]["user"][0]["summary"],
            json!("Builder skill")
        );
    }
}
