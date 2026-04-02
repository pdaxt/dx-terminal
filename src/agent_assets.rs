use serde_json::{json, Value};
use std::path::Path;
use walkdir::WalkDir;

const PROVIDER_DIRS: &[(&str, &str)] = &[
    ("claude", ".claude"),
    ("codex", ".codex"),
    ("gemini", ".gemini"),
];

pub fn collect_automation_assets(project_root: &str) -> Value {
    collect_automation_assets_with_home(Path::new(project_root), &crate::config::home_dir())
}

fn collect_automation_assets_with_home(project_root: &Path, home_root: &Path) -> Value {
    let mut project_commands = Vec::new();
    let mut user_commands = Vec::new();
    let mut project_skills = Vec::new();
    let mut user_skills = Vec::new();
    let mut command_providers = serde_json::Map::new();
    let mut skill_providers = serde_json::Map::new();

    for (provider, dir_name) in PROVIDER_DIRS {
        let provider_project_commands = collect_command_assets(
            &project_root.join(dir_name).join("commands"),
            provider,
            "project",
        );
        let provider_user_commands =
            collect_command_assets(&home_root.join(dir_name).join("commands"), provider, "user");
        let provider_project_skills = collect_skill_assets(
            &project_root.join(dir_name).join("skills"),
            provider,
            "project",
        );
        let provider_user_skills =
            collect_skill_assets(&home_root.join(dir_name).join("skills"), provider, "user");

        project_commands.extend(provider_project_commands.iter().cloned());
        user_commands.extend(provider_user_commands.iter().cloned());
        project_skills.extend(provider_project_skills.iter().cloned());
        user_skills.extend(provider_user_skills.iter().cloned());

        command_providers.insert(
            (*provider).to_string(),
            json!({
                "project": provider_project_commands,
                "user": provider_user_commands,
            }),
        );
        skill_providers.insert(
            (*provider).to_string(),
            json!({
                "project": provider_project_skills,
                "user": provider_user_skills,
            }),
        );
    }

    project_commands.sort_by(compare_asset_name);
    user_commands.sort_by(compare_asset_name);
    project_skills.sort_by(compare_asset_name);
    user_skills.sort_by(compare_asset_name);

    let external_mcps = crate::external_mcp::load_external_catalog()
        .into_iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "description": entry.description,
                "category": entry.category,
                "capabilities": entry.capabilities,
                "sources": entry.sources,
            })
        })
        .collect::<Vec<_>>();
    let provider_plugins = crate::provider_plugins::plugin_inventory();
    let automation_bridges =
        crate::provider_asset_plugins::plugin_inventory(Some(&project_root.to_string_lossy()));

    json!({
        "commands": {
            "project": project_commands,
            "user": user_commands,
            "providers": command_providers,
        },
        "skills": {
            "project": project_skills,
            "user": user_skills,
            "providers": skill_providers,
        },
        "external_mcps": external_mcps,
        "provider_plugins": provider_plugins,
        "automation_bridges": automation_bridges,
        "counts": {
            "project_commands": project_commands.len(),
            "user_commands": user_commands.len(),
            "project_skills": project_skills.len(),
            "user_skills": user_skills.len(),
            "external_mcps": external_mcps.len(),
            "provider_plugins": provider_plugins
                .get("providers")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
                .unwrap_or(0),
            "automation_bridges": automation_bridges
                .get("providers")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
                .unwrap_or(0),
            "commands_by_provider": counts_by_provider(
                json!({
                    "claude": {
                        "project": project_commands_for_provider(&command_providers, "claude"),
                        "user": user_commands_for_provider(&command_providers, "claude"),
                    },
                    "codex": {
                        "project": project_commands_for_provider(&command_providers, "codex"),
                        "user": user_commands_for_provider(&command_providers, "codex"),
                    },
                    "gemini": {
                        "project": project_commands_for_provider(&command_providers, "gemini"),
                        "user": user_commands_for_provider(&command_providers, "gemini"),
                    }
                }),
            ),
            "skills_by_provider": counts_by_provider(
                json!({
                    "claude": {
                        "project": project_commands_for_provider(&skill_providers, "claude"),
                        "user": user_commands_for_provider(&skill_providers, "claude"),
                    },
                    "codex": {
                        "project": project_commands_for_provider(&skill_providers, "codex"),
                        "user": user_commands_for_provider(&skill_providers, "codex"),
                    },
                    "gemini": {
                        "project": project_commands_for_provider(&skill_providers, "gemini"),
                        "user": user_commands_for_provider(&skill_providers, "gemini"),
                    }
                }),
            ),
        }
    })
}

fn project_commands_for_provider(
    providers: &serde_json::Map<String, Value>,
    provider: &str,
) -> usize {
    providers
        .get(provider)
        .and_then(|value| value.get("project"))
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn user_commands_for_provider(providers: &serde_json::Map<String, Value>, provider: &str) -> usize {
    providers
        .get(provider)
        .and_then(|value| value.get("user"))
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn counts_by_provider(value: Value) -> Value {
    let Some(providers) = value.as_object() else {
        return json!({});
    };

    let mut counts = serde_json::Map::new();
    for (provider, scopes) in providers {
        counts.insert(
            provider.clone(),
            json!({
                "project": scopes.get("project").and_then(|entry| entry.as_u64()).unwrap_or(0),
                "user": scopes.get("user").and_then(|entry| entry.as_u64()).unwrap_or(0),
            }),
        );
    }
    Value::Object(counts)
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

    assets.sort_by(compare_asset_name);
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
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !path.is_file() || !file_name.eq_ignore_ascii_case("SKILL.md") {
            continue;
        }

        let skill_name = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("skill");
        assets.push(asset_json(path, provider, scope, "skill", skill_name));
    }

    assets.sort_by(compare_asset_name);
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
        let project_codex_command_dir = project.path().join(".codex").join("commands");
        let project_skill_dir = project
            .path()
            .join(".codex")
            .join("skills")
            .join("reviewer");
        let user_skill_dir = home.path().join(".codex").join("skills").join("builder");
        let user_gemini_command_dir = home.path().join(".gemini").join("commands");

        std::fs::create_dir_all(&project_command_dir).unwrap();
        std::fs::create_dir_all(&project_codex_command_dir).unwrap();
        std::fs::create_dir_all(&project_skill_dir).unwrap();
        std::fs::create_dir_all(&user_skill_dir).unwrap();
        std::fs::create_dir_all(&user_gemini_command_dir).unwrap();

        std::fs::write(
            project_command_dir.join("handoff.md"),
            "# Handoff\nProject handoff command",
        )
        .unwrap();
        std::fs::write(
            project_codex_command_dir.join("review.md"),
            "# Review\nCodex project command",
        )
        .unwrap();
        std::fs::write(
            project_skill_dir.join("SKILL.md"),
            "# Reviewer\nReview workflows",
        )
        .unwrap();
        std::fs::write(user_skill_dir.join("SKILL.md"), "Builder skill").unwrap();
        std::fs::write(
            user_gemini_command_dir.join("triage.md"),
            "# Triage\nGemini command",
        )
        .unwrap();

        let assets = collect_automation_assets_with_home(project.path(), home.path());

        assert_eq!(assets["counts"]["project_commands"], json!(2));
        assert_eq!(assets["counts"]["user_commands"], json!(1));
        assert_eq!(assets["counts"]["project_skills"], json!(1));
        assert_eq!(assets["counts"]["user_skills"], json!(1));
        assert_eq!(
            assets["counts"]["commands_by_provider"]["claude"]["project"],
            json!(1)
        );
        assert_eq!(
            assets["counts"]["commands_by_provider"]["codex"]["project"],
            json!(1)
        );
        assert_eq!(
            assets["counts"]["commands_by_provider"]["gemini"]["user"],
            json!(1)
        );
        assert_eq!(assets["commands"]["project"][0]["name"], json!("handoff"));
        assert_eq!(assets["skills"]["project"][0]["name"], json!("reviewer"));
        assert_eq!(
            assets["skills"]["user"][0]["summary"],
            json!("Builder skill")
        );
    }

    #[test]
    fn collects_lowercase_skill_files() {
        let project = tempdir().unwrap();
        let home = tempdir().unwrap();
        let claude_skill_dir = project
            .path()
            .join(".claude")
            .join("skills")
            .join("gov-pdf-downloader");
        std::fs::create_dir_all(&claude_skill_dir).unwrap();
        std::fs::write(
            claude_skill_dir.join("skill.md"),
            "# Gov PDF Downloader\nDownload and extract policy PDFs",
        )
        .unwrap();

        let assets = collect_automation_assets_with_home(project.path(), home.path());
        assert_eq!(assets["counts"]["project_skills"], json!(1));
        assert_eq!(assets["skills"]["project"][0]["provider"], json!("claude"));
        assert_eq!(
            assets["skills"]["project"][0]["name"],
            json!("gov-pdf-downloader")
        );
    }
}
