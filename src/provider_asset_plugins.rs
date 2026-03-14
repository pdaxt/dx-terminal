use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const BRIDGE_VERSION: u32 = 1;
const SHARED_SOURCE: &str = "dx";
const MARKER_PREFIX: &str = "<!-- dx-automation-bridge:";

#[derive(Debug, Clone)]
struct AssetRecord {
    provider: String,
    scope: String,
    kind: String,
    name: String,
    path: PathBuf,
    content: String,
    summary: String,
    modified_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct BridgeTarget {
    provider: String,
    label: String,
    format: String,
    project_path: String,
    user_path: String,
    project_exists: bool,
    user_exists: bool,
    project_exported_assets: usize,
    user_exported_assets: usize,
    project_exported_workflows: usize,
    user_exported_workflows: usize,
    project_conflicts: usize,
    user_conflicts: usize,
    available_project_assets: usize,
    available_user_assets: usize,
    available_project_workflows: usize,
    available_user_workflows: usize,
    available_project_commands: usize,
    available_project_skills: usize,
    available_user_commands: usize,
    available_user_skills: usize,
}

#[derive(Debug, Clone)]
struct ExportOutcome {
    asset: AssetRecord,
    target_path: PathBuf,
    sources: Vec<String>,
    status: &'static str,
}

pub fn plugin_inventory(project_root: Option<&str>) -> Value {
    plugin_inventory_with_home(
        project_root.map(PathBuf::from).as_deref(),
        &crate::config::home_dir(),
    )
}

pub fn convert_provider_asset_plugin(
    project_root: Option<&str>,
    source_provider: Option<&str>,
    target_provider: &str,
    dry_run: bool,
) -> Result<Value> {
    convert_provider_asset_plugin_with_home(
        project_root.map(PathBuf::from).as_deref(),
        &crate::config::home_dir(),
        source_provider,
        target_provider,
        dry_run,
    )
}

pub fn runtime_guidance(project_root: Option<&str>, provider: &str, max_items: usize) -> Value {
    runtime_guidance_with_home(
        project_root.map(PathBuf::from).as_deref(),
        &crate::config::home_dir(),
        provider,
        max_items,
    )
}

pub fn shared_workflow_catalog(project_root: Option<&str>, source_provider: Option<&str>) -> Value {
    let project_root = project_root.map(PathBuf::from);
    let items = shared_assets(
        project_root.as_deref(),
        &crate::config::home_dir(),
        source_provider,
    );
    json!({
        "source_of_truth": SHARED_SOURCE,
        "source_provider": source_provider
            .map(crate::provider_plugins::normalized_provider)
            .unwrap_or(SHARED_SOURCE),
        "project_scope": project_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        "workflows": workflow_records_from_assets(&items),
    })
}

pub fn shared_workflow_definition(
    project_root: Option<&str>,
    source_provider: Option<&str>,
    workflow_id: &str,
) -> Option<Value> {
    if workflow_id.trim().is_empty() {
        return None;
    }
    let catalog = shared_workflow_catalog(project_root, source_provider);
    catalog
        .get("workflows")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(workflow_id.trim()))
        })
        .cloned()
}

pub fn runtime_guidance_markdown(
    project_root: Option<&str>,
    provider: &str,
    max_items: usize,
) -> String {
    let guidance = runtime_guidance(project_root, provider, max_items);
    let mut lines = vec![
        "## DX Automation Workflows".to_string(),
        format!(
            "- Provider bridge: {}",
            guidance
                .get("provider_label")
                .and_then(Value::as_str)
                .unwrap_or(provider)
        ),
    ];

    if let Some(path) = guidance
        .get("project_manifest_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- Project automation manifest: {}", path));
    }
    if let Some(path) = guidance
        .get("project_workflow_catalog_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- Project workflow catalog: {}", path));
    }
    if let Some(path) = guidance
        .get("user_manifest_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- User automation manifest: {}", path));
    }
    if let Some(path) = guidance
        .get("user_workflow_catalog_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- User workflow catalog: {}", path));
    }

    append_catalog_section(
        &mut lines,
        "Project command packs",
        guidance.get("project_commands"),
    );
    append_catalog_section(&mut lines, "Project skills", guidance.get("project_skills"));
    append_catalog_section(
        &mut lines,
        "Project structured workflows",
        guidance.get("project_workflows"),
    );
    append_catalog_section(
        &mut lines,
        "User command packs",
        guidance.get("user_commands"),
    );
    append_catalog_section(&mut lines, "User skills", guidance.get("user_skills"));
    append_catalog_section(
        &mut lines,
        "User structured workflows",
        guidance.get("user_workflows"),
    );
    lines.push(
        "- Use the DX-managed provider-local assets above before inventing a new workflow from scratch. If a needed workflow is missing, document the gap explicitly."
            .to_string(),
    );

    lines.join("\n")
}

fn plugin_inventory_with_home(project_root: Option<&Path>, home_root: &Path) -> Value {
    let shared = shared_assets(project_root, home_root, None);
    let shared_counts = asset_breakdown(&shared);
    let providers = ["claude", "codex", "gemini"]
        .into_iter()
        .map(|provider| bridge_target(project_root, home_root, provider, &shared_counts))
        .collect::<Vec<_>>();

    json!({
        "source_of_truth": "dx_shared_automation_manifest",
        "shared_asset_count": shared.len(),
        "counts": {
            "project_assets": shared_counts.project_assets,
            "user_assets": shared_counts.user_assets,
            "project_workflows": shared_counts.project_workflows,
            "user_workflows": shared_counts.user_workflows,
            "project_commands": shared_counts.project_commands,
            "project_skills": shared_counts.project_skills,
            "user_commands": shared_counts.user_commands,
            "user_skills": shared_counts.user_skills,
        },
        "providers": providers,
        "bridge_contract": {
            "project_scope": project_root.map(|path| path.to_string_lossy().to_string()),
            "user_scope": home_root.to_string_lossy().to_string(),
            "format": "generated provider assets + dx-automation-plugin.json manifest",
        }
    })
}

fn convert_provider_asset_plugin_with_home(
    project_root: Option<&Path>,
    home_root: &Path,
    source_provider: Option<&str>,
    target_provider: &str,
    dry_run: bool,
) -> Result<Value> {
    let target = crate::provider_plugins::normalized_provider(target_provider);
    let source = source_provider
        .map(crate::provider_plugins::normalized_provider)
        .unwrap_or(SHARED_SOURCE);
    let deduped = shared_assets(project_root, home_root, source_provider);

    let mut project_outcomes = Vec::new();
    let mut user_outcomes = Vec::new();

    for (asset, sources) in deduped {
        let outcome = export_asset(project_root, home_root, target, &asset, &sources, dry_run)?;
        match asset.scope.as_str() {
            "project" => project_outcomes.push(outcome),
            _ => user_outcomes.push(outcome),
        }
    }

    let project_manifest =
        write_manifest(project_root, target, "project", &project_outcomes, dry_run)?;
    let user_manifest = write_manifest(Some(home_root), target, "user", &user_outcomes, dry_run)?;
    let project_workflow_catalog =
        write_workflow_catalog(project_root, target, "project", &project_outcomes, dry_run)?;
    let user_workflow_catalog =
        write_workflow_catalog(Some(home_root), target, "user", &user_outcomes, dry_run)?;

    Ok(json!({
        "ok": true,
        "source": source,
        "target": target,
        "dry_run": dry_run,
        "project_manifest_path": project_manifest,
        "user_manifest_path": user_manifest,
        "project_workflow_catalog_path": project_workflow_catalog,
        "user_workflow_catalog_path": user_workflow_catalog,
        "project": summarize_outcomes(&project_outcomes),
        "user": summarize_outcomes(&user_outcomes),
    }))
}

fn export_asset(
    project_root: Option<&Path>,
    home_root: &Path,
    target_provider: &str,
    asset: &AssetRecord,
    sources: &[String],
    dry_run: bool,
) -> Result<ExportOutcome> {
    let target_path = target_asset_path(project_root, home_root, target_provider, asset)?;
    let rendered = render_asset_content(asset, sources);
    let status = if target_path.exists() {
        if is_dx_managed(&target_path) {
            if !dry_run {
                write_asset(&target_path, &rendered)?;
            }
            "updated"
        } else {
            "conflict"
        }
    } else {
        if !dry_run {
            write_asset(&target_path, &rendered)?;
        }
        "created"
    };

    Ok(ExportOutcome {
        asset: asset.clone(),
        target_path,
        sources: sources.to_vec(),
        status,
    })
}

fn write_manifest(
    root: Option<&Path>,
    target_provider: &str,
    scope: &str,
    outcomes: &[ExportOutcome],
    dry_run: bool,
) -> Result<Option<String>> {
    let Some(root) = root else {
        return Ok(None);
    };
    let manifest_path = provider_root(root, target_provider).join("dx-automation-plugin.json");
    let payload = json!({
        "dxAutomationPlugin": {
            "version": BRIDGE_VERSION,
            "provider": target_provider,
            "scope": scope,
            "sourceOfTruth": SHARED_SOURCE,
            "exportedAt": unix_timestamp(),
            "assets": outcomes.iter().map(|outcome| {
                json!({
                    "name": outcome.asset.name,
                    "kind": outcome.asset.kind,
                    "scope": outcome.asset.scope,
                    "status": outcome.status,
                    "targetPath": outcome.target_path.to_string_lossy().to_string(),
                    "sources": outcome.sources,
                    "sourcePath": outcome.asset.path.to_string_lossy().to_string(),
                    "summary": outcome.asset.summary,
                })
            }).collect::<Vec<_>>(),
        }
    });

    if !dry_run {
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = manifest_path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&payload)?)?;
        std::fs::rename(&tmp, &manifest_path)?;
    }

    Ok(Some(manifest_path.to_string_lossy().to_string()))
}

fn write_workflow_catalog(
    root: Option<&Path>,
    target_provider: &str,
    scope: &str,
    outcomes: &[ExportOutcome],
    dry_run: bool,
) -> Result<Option<String>> {
    let Some(root) = root else {
        return Ok(None);
    };
    let path = provider_root(root, target_provider).join("dx-workflow-catalog.json");
    let payload = build_workflow_catalog_payload(target_provider, scope, outcomes);
    if !dry_run {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&payload)?)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(Some(path.to_string_lossy().to_string()))
}

fn build_workflow_catalog_payload(
    target_provider: &str,
    scope: &str,
    outcomes: &[ExportOutcome],
) -> Value {
    json!({
        "dxWorkflowCatalog": {
            "version": BRIDGE_VERSION,
            "provider": target_provider,
            "scope": scope,
            "sourceOfTruth": SHARED_SOURCE,
            "exportedAt": unix_timestamp(),
            "workflows": workflow_records(outcomes),
        }
    })
}

fn summarize_outcomes(outcomes: &[ExportOutcome]) -> Value {
    let created = outcomes
        .iter()
        .filter(|item| item.status == "created")
        .count();
    let updated = outcomes
        .iter()
        .filter(|item| item.status == "updated")
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|item| item.status == "conflict")
        .count();
    json!({
        "assets": outcomes.len(),
        "created": created,
        "updated": updated,
        "conflicts": conflicts,
        "workflows": outcomes.len(),
        "commands": outcomes.iter().filter(|item| item.asset.kind == "command").count(),
        "skills": outcomes.iter().filter(|item| item.asset.kind == "skill").count(),
        "conflict_paths": outcomes
            .iter()
            .filter(|item| item.status == "conflict")
            .map(|item| item.target_path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
    })
}

fn bridge_target(
    project_root: Option<&Path>,
    home_root: &Path,
    provider: &str,
    shared_counts: &AssetBreakdown,
) -> BridgeTarget {
    let project_path = project_root
        .map(|path| provider_root(path, provider).join("dx-automation-plugin.json"))
        .unwrap_or_else(|| PathBuf::from(format!(".{provider}/dx-automation-plugin.json")));
    let user_path = provider_root(home_root, provider).join("dx-automation-plugin.json");
    let project_manifest = read_manifest(&project_path);
    let user_manifest = read_manifest(&user_path);
    BridgeTarget {
        provider: provider.to_string(),
        label: crate::provider_plugins::provider_label(provider).to_string(),
        format: "json".to_string(),
        project_path: project_path.to_string_lossy().to_string(),
        user_path: user_path.to_string_lossy().to_string(),
        project_exists: project_path.exists(),
        user_exists: user_path.exists(),
        project_exported_assets: manifest_status_count(&project_manifest, &["created", "updated"]),
        user_exported_assets: manifest_status_count(&user_manifest, &["created", "updated"]),
        project_exported_workflows: manifest_workflow_count(&project_path),
        user_exported_workflows: manifest_workflow_count(&user_path),
        project_conflicts: manifest_status_count(&project_manifest, &["conflict"]),
        user_conflicts: manifest_status_count(&user_manifest, &["conflict"]),
        available_project_assets: shared_counts.project_assets,
        available_user_assets: shared_counts.user_assets,
        available_project_workflows: shared_counts.project_workflows,
        available_user_workflows: shared_counts.user_workflows,
        available_project_commands: shared_counts.project_commands,
        available_project_skills: shared_counts.project_skills,
        available_user_commands: shared_counts.user_commands,
        available_user_skills: shared_counts.user_skills,
    }
}

fn runtime_guidance_with_home(
    project_root: Option<&Path>,
    home_root: &Path,
    provider: &str,
    max_items: usize,
) -> Value {
    let shared = shared_assets(project_root, home_root, None);
    let provider_root_project = project_root.map(|path| provider_root(path, provider));
    let provider_root_user = provider_root(home_root, provider);

    json!({
        "provider": crate::provider_plugins::normalized_provider(provider),
        "provider_label": crate::provider_plugins::provider_label(provider),
        "project_manifest_path": provider_root_project
            .as_ref()
            .map(|path| path.join("dx-automation-plugin.json").to_string_lossy().to_string()),
        "project_workflow_catalog_path": provider_root_project
            .as_ref()
            .map(|path| path.join("dx-workflow-catalog.json").to_string_lossy().to_string()),
        "user_manifest_path": provider_root_user
            .join("dx-automation-plugin.json")
            .to_string_lossy()
            .to_string(),
        "user_workflow_catalog_path": provider_root_user
            .join("dx-workflow-catalog.json")
            .to_string_lossy()
            .to_string(),
        "project_commands": collect_names_for_scope_kind(&shared, "project", "command", max_items),
        "project_skills": collect_names_for_scope_kind(&shared, "project", "skill", max_items),
        "project_workflows": collect_names_for_scope_kind(&shared, "project", "workflow", max_items),
        "user_commands": collect_names_for_scope_kind(&shared, "user", "command", max_items),
        "user_skills": collect_names_for_scope_kind(&shared, "user", "skill", max_items),
        "user_workflows": collect_names_for_scope_kind(&shared, "user", "workflow", max_items),
    })
}

fn read_manifest(path: &Path) -> Value {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Value::Null;
    };
    serde_json::from_str(&raw).unwrap_or(Value::Null)
}

fn manifest_status_count(manifest: &Value, statuses: &[&str]) -> usize {
    manifest
        .get("dxAutomationPlugin")
        .and_then(|value| value.get("assets"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("status")
                        .and_then(Value::as_str)
                        .map(|status| statuses.iter().any(|candidate| candidate == &status))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn manifest_workflow_count(manifest_path: &Path) -> usize {
    let workflow_path = manifest_path
        .parent()
        .map(|parent| parent.join("dx-workflow-catalog.json"));
    let Some(workflow_path) = workflow_path else {
        return 0;
    };
    let Ok(raw) = std::fs::read_to_string(workflow_path) else {
        return 0;
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return 0;
    };
    value
        .get("dxWorkflowCatalog")
        .and_then(|value| value.get("workflows"))
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0)
}

fn target_asset_path(
    project_root: Option<&Path>,
    home_root: &Path,
    target_provider: &str,
    asset: &AssetRecord,
) -> Result<PathBuf> {
    let base = match asset.scope.as_str() {
        "project" => project_root.ok_or_else(|| {
            anyhow::anyhow!("project scope export requested without project root")
        })?,
        _ => home_root,
    };
    let root = provider_root(base, target_provider);
    Ok(match asset.kind.as_str() {
        "skill" => root.join("skills").join(&asset.name).join("SKILL.md"),
        _ => root.join("commands").join(format!("{}.md", asset.name)),
    })
}

fn write_asset(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn render_asset_content(asset: &AssetRecord, sources: &[String]) -> String {
    let header = json!({
        "version": BRIDGE_VERSION,
        "sourceOfTruth": SHARED_SOURCE,
        "kind": asset.kind,
        "scope": asset.scope,
        "name": asset.name,
        "sourceProvider": asset.provider,
        "sources": sources,
        "exportedAt": unix_timestamp(),
    });
    format!(
        "{} {} -->\n\n{}",
        MARKER_PREFIX,
        serde_json::to_string(&header).unwrap_or_else(|_| "{}".to_string()),
        asset.content.trim_start()
    )
}

fn is_dx_managed(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content
        .lines()
        .next()
        .unwrap_or_default()
        .starts_with(MARKER_PREFIX)
}

fn strip_dx_header(content: &str) -> String {
    let mut lines = content.lines();
    let first = lines.next().unwrap_or_default();
    if !first.starts_with(MARKER_PREFIX) {
        return content.to_string();
    }
    let mut stripped = lines.collect::<Vec<_>>().join("\n");
    while stripped.starts_with('\n') {
        stripped.remove(0);
    }
    stripped
}

fn shared_assets(
    project_root: Option<&Path>,
    home_root: &Path,
    source_provider: Option<&str>,
) -> Vec<(AssetRecord, Vec<String>)> {
    let normalized = source_provider.map(crate::provider_plugins::normalized_provider);
    let mut records = Vec::new();
    for (provider, dir_name) in provider_dirs() {
        if let Some(filter) = normalized {
            if filter != SHARED_SOURCE && provider != filter {
                continue;
            }
        }
        if let Some(project_root) = project_root {
            records.extend(collect_command_assets(
                &project_root.join(dir_name).join("commands"),
                provider,
                "project",
            ));
            records.extend(collect_skill_assets(
                &project_root.join(dir_name).join("skills"),
                provider,
                "project",
            ));
        }
        records.extend(collect_command_assets(
            &home_root.join(dir_name).join("commands"),
            provider,
            "user",
        ));
        records.extend(collect_skill_assets(
            &home_root.join(dir_name).join("skills"),
            provider,
            "user",
        ));
    }

    let mut merged: HashMap<(String, String, String), (AssetRecord, Vec<String>)> = HashMap::new();
    for record in records {
        let key = (
            record.scope.clone(),
            record.kind.clone(),
            record.name.clone(),
        );
        match merged.get_mut(&key) {
            Some((current, sources)) => {
                if !sources.iter().any(|source| source == &record.provider) {
                    sources.push(record.provider.clone());
                }
                if record.modified_unix_ms >= current.modified_unix_ms {
                    *current = record;
                }
            }
            None => {
                merged.insert(key, (record.clone(), vec![record.provider.clone()]));
            }
        }
    }

    let mut items = merged.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.0
            .scope
            .cmp(&right.0.scope)
            .then(left.0.kind.cmp(&right.0.kind))
            .then(left.0.name.cmp(&right.0.name))
    });
    items
}

fn collect_command_assets(dir: &Path, provider: &str, scope: &str) -> Vec<AssetRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md"))
        .filter_map(|path| read_asset(path, provider, scope, "command"))
        .collect()
}

fn collect_skill_assets(dir: &Path, provider: &str, scope: &str) -> Vec<AssetRecord> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("SKILL.md"))
                .unwrap_or(false)
        })
        .filter_map(|path| read_asset(path, provider, scope, "skill"))
        .collect()
}

fn read_asset(path: PathBuf, provider: &str, scope: &str, kind: &str) -> Option<AssetRecord> {
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return None;
    };
    let content = strip_dx_header(&raw);
    let name = match kind {
        "skill" => path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("skill")
            .to_string(),
        _ => path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("command")
            .to_string(),
    };
    Some(AssetRecord {
        provider: provider.to_string(),
        scope: scope.to_string(),
        kind: kind.to_string(),
        name,
        summary: read_summary(&content),
        modified_unix_ms: modified_unix_ms(&path),
        path,
        content,
    })
}

fn read_summary(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("No summary available")
        .trim_start_matches('#')
        .trim()
        .to_string()
}

fn modified_unix_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn provider_dirs() -> [(&'static str, &'static str); 3] {
    [
        ("claude", ".claude"),
        ("codex", ".codex"),
        ("gemini", ".gemini"),
    ]
}

fn provider_root(base: &Path, provider: &str) -> PathBuf {
    let dir_name = match crate::provider_plugins::normalized_provider(provider) {
        "codex" => ".codex",
        "gemini" => ".gemini",
        _ => ".claude",
    };
    base.join(dir_name)
}

#[derive(Default)]
struct AssetBreakdown {
    project_assets: usize,
    user_assets: usize,
    project_workflows: usize,
    user_workflows: usize,
    project_commands: usize,
    project_skills: usize,
    user_commands: usize,
    user_skills: usize,
}

fn asset_breakdown(items: &[(AssetRecord, Vec<String>)]) -> AssetBreakdown {
    let mut counts = AssetBreakdown::default();
    for (asset, _) in items {
        match (asset.scope.as_str(), asset.kind.as_str()) {
            ("project", "command") => {
                counts.project_assets += 1;
                counts.project_workflows += 1;
                counts.project_commands += 1;
            }
            ("project", _) => {
                counts.project_assets += 1;
                counts.project_workflows += 1;
                counts.project_skills += 1;
            }
            ("user", "command") => {
                counts.user_assets += 1;
                counts.user_workflows += 1;
                counts.user_commands += 1;
            }
            _ => {
                counts.user_assets += 1;
                counts.user_workflows += 1;
                counts.user_skills += 1;
            }
        }
    }
    counts
}

fn collect_names_for_scope_kind(
    items: &[(AssetRecord, Vec<String>)],
    scope: &str,
    kind: &str,
    max_items: usize,
) -> Vec<String> {
    items
        .iter()
        .filter(|(asset, _)| {
            asset.scope == scope
                && (asset.kind == kind
                    || (kind == "workflow" && matches!(asset.kind.as_str(), "command" | "skill")))
        })
        .map(|(asset, _)| asset.name.clone())
        .take(max_items.max(1))
        .collect()
}

fn workflow_records(outcomes: &[ExportOutcome]) -> Vec<Value> {
    outcomes
        .iter()
        .map(|outcome| {
            workflow_record_value(
                &outcome.asset,
                &outcome.sources,
                Some(outcome.target_path.to_string_lossy().to_string()),
                Some(outcome.status.to_string()),
            )
        })
        .collect()
}

fn workflow_records_from_assets(items: &[(AssetRecord, Vec<String>)]) -> Vec<Value> {
    items
        .iter()
        .map(|(asset, sources)| workflow_record_value(asset, sources, None, None))
        .collect()
}

fn workflow_record_value(
    asset: &AssetRecord,
    sources: &[String],
    target_path: Option<String>,
    export_status: Option<String>,
) -> Value {
    let (headings, steps) = extract_workflow_structure(&asset.content);
    json!({
        "id": format!("{}:{}:{}", asset.scope, asset.kind, asset.name),
        "name": asset.name,
        "kind": asset.kind,
        "scope": asset.scope,
        "summary": asset.summary,
        "canonical_provider": asset.provider,
        "sources": sources,
        "source_path": asset.path.to_string_lossy().to_string(),
        "target_path": target_path,
        "export_status": export_status,
        "sections": headings,
        "steps": steps,
    })
}

fn extract_workflow_structure(content: &str) -> (Vec<String>, Vec<String>) {
    let mut headings = Vec::new();
    let mut steps = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(heading) = line.strip_prefix('#') {
            let value = heading.trim_start_matches('#').trim();
            if !value.is_empty() {
                headings.push(value.to_string());
            }
            continue;
        }
        if let Some(step) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            let value = step.trim();
            if !value.is_empty() {
                steps.push(value.to_string());
            }
            continue;
        }
        if let Some((prefix, rest)) = line.split_once(". ") {
            if prefix.chars().all(|ch| ch.is_ascii_digit()) && !rest.trim().is_empty() {
                steps.push(rest.trim().to_string());
            }
        }
    }
    if steps.is_empty() {
        steps.extend(
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .take(5)
                .map(|line| line.to_string()),
        );
    }
    (
        headings.into_iter().take(6).collect(),
        steps.into_iter().take(8).collect(),
    )
}

fn append_catalog_section(lines: &mut Vec<String>, heading: &str, values: Option<&Value>) {
    let items = values
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if items.is_empty() {
        return;
    }
    lines.push(format!("- {}:", heading));
    for item in items {
        lines.push(format!("  - {}", item));
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_assets_to_target_provider_dirs() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        let command_dir = project.path().join(".claude").join("commands");
        let skill_dir = home.path().join(".codex").join("skills").join("reviewer");
        std::fs::create_dir_all(&command_dir).unwrap();
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(command_dir.join("handoff.md"), "# Handoff\nProject command").unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Reviewer\nReview skill").unwrap();

        let result = convert_provider_asset_plugin_with_home(
            Some(project.path()),
            home.path(),
            None,
            "gemini",
            false,
        )
        .unwrap();

        assert_eq!(result["project"]["commands"], json!(1));
        assert_eq!(result["user"]["skills"], json!(1));
        assert!(project
            .path()
            .join(".gemini")
            .join("commands")
            .join("handoff.md")
            .exists());
        assert!(home
            .path()
            .join(".gemini")
            .join("skills")
            .join("reviewer")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn preserves_user_owned_conflicts() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        let source_dir = home.path().join(".claude").join("commands");
        let target_dir = home.path().join(".codex").join("commands");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(source_dir.join("deploy.md"), "# Deploy\nShared deploy flow").unwrap();
        std::fs::write(target_dir.join("deploy.md"), "# Deploy\nUser owned command").unwrap();

        let result = convert_provider_asset_plugin_with_home(
            Some(project.path()),
            home.path(),
            Some("claude"),
            "codex",
            false,
        )
        .unwrap();

        assert_eq!(result["user"]["conflicts"], json!(1));
        let target = std::fs::read_to_string(target_dir.join("deploy.md")).unwrap();
        assert!(target.contains("User owned command"));
    }

    #[test]
    fn runtime_guidance_lists_shared_assets() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        let project_command_dir = project.path().join(".claude").join("commands");
        let user_skill_dir = home.path().join(".codex").join("skills").join("reviewer");
        std::fs::create_dir_all(&project_command_dir).unwrap();
        std::fs::create_dir_all(&user_skill_dir).unwrap();
        std::fs::write(project_command_dir.join("handoff.md"), "# Handoff").unwrap();
        std::fs::write(user_skill_dir.join("SKILL.md"), "# Reviewer").unwrap();

        let guidance = runtime_guidance_with_home(Some(project.path()), home.path(), "gemini", 5);
        assert_eq!(guidance["project_commands"][0], json!("handoff"));
        assert_eq!(guidance["user_skills"][0], json!("reviewer"));
    }

    #[test]
    fn extracts_structured_workflow_steps() {
        let content = r#"
# Ship Flow

## Steps
1. Inspect the diff
2. Run tests
- Summarize the release risk
"#;
        let (headings, steps) = extract_workflow_structure(content);
        assert_eq!(headings[0], "Ship Flow");
        assert!(headings.iter().any(|item| item == "Steps"));
        assert_eq!(steps[0], "Inspect the diff");
        assert_eq!(steps[1], "Run tests");
        assert_eq!(steps[2], "Summarize the release risk");
    }
}
