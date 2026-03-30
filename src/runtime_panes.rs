use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::state::types::{DxTerminalState, PaneState};
use crate::tmux::{self, LivePane};

#[derive(Clone, Debug)]
pub struct ResolvedRuntimePane {
    pub pane: u8,
    pub pane_state: PaneState,
    pub live: Option<LivePane>,
    pub state_backed: bool,
}

impl ResolvedRuntimePane {
    pub fn tmux_target(&self) -> Option<&str> {
        self.pane_state.tmux_target.as_deref()
    }
}

pub fn resolve_runtime_panes(
    state: &DxTerminalState,
    live_panes: &[LivePane],
    previous_targets: Option<&HashMap<u8, String>>,
) -> Vec<ResolvedRuntimePane> {
    let mut records: BTreeMap<u8, ResolvedRuntimePane> = state
        .panes
        .iter()
        .filter_map(|(pane_key, pane_state)| {
            pane_key.parse::<u8>().ok().map(|pane| {
                (
                    pane,
                    ResolvedRuntimePane {
                        pane,
                        pane_state: pane_state.clone(),
                        live: None,
                        state_backed: true,
                    },
                )
            })
        })
        .collect();

    let mut used_panes: BTreeSet<u8> = records
        .iter()
        .filter_map(|(pane, record)| pane_reserves_slot(&record.pane_state).then_some(*pane))
        .collect();
    let mut target_to_pane: HashMap<String, u8> = records
        .iter()
        .filter_map(|(pane, record)| {
            record
                .pane_state
                .tmux_target
                .as_ref()
                .map(|target| (target.clone(), *pane))
        })
        .collect();

    let mut previous_by_target = HashMap::new();
    if let Some(previous_targets) = previous_targets {
        for (pane, target) in previous_targets {
            previous_by_target.entry(target.clone()).or_insert(*pane);
        }
    }

    let mut live_sorted = live_panes.to_vec();
    live_sorted.sort_by(|a, b| {
        (&a.session, a.window, a.pane_idx, &a.target)
            .cmp(&(&b.session, b.window, b.pane_idx, &b.target))
    });

    for live in live_sorted {
        let pane = target_to_pane
            .get(&live.target)
            .copied()
            .or_else(|| {
                previous_by_target
                    .get(&live.target)
                    .copied()
                    .filter(|candidate| !used_panes.contains(candidate))
            })
            .unwrap_or_else(|| next_unused_pane(&used_panes));

        let record = records.entry(pane).or_insert_with(|| ResolvedRuntimePane {
            pane,
            pane_state: synthesize_pane_state(pane, &live),
            live: None,
            state_backed: false,
        });

        enrich_pane_state(&mut record.pane_state, pane, &live);
        record.live = Some(live.clone());
        target_to_pane.insert(live.target.clone(), pane);
        used_panes.insert(pane);
    }

    records.into_values().collect()
}

pub fn project_from_cwd(cwd: &str) -> String {
    if cwd.trim().is_empty() || cwd == "--" {
        return "--".to_string();
    }

    let path = Path::new(cwd);
    let home = std::env::var("HOME").unwrap_or_default();
    let projects_dir = format!("{}/Projects", home);

    if cwd == projects_dir || cwd == home {
        return "--".to_string();
    }

    if let Ok(rel) = path.strip_prefix(&projects_dir) {
        if let Some(first) = rel.components().next() {
            return first.as_os_str().to_string_lossy().to_string();
        }
    }

    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

pub fn project_root_from_cwd(cwd: &str) -> String {
    if cwd.trim().is_empty() || cwd == "--" {
        return String::new();
    }

    let path = Path::new(cwd);
    let home = std::env::var("HOME").unwrap_or_default();
    let home_path = Path::new(&home);
    let projects_dir = home_path.join("Projects");

    if path == projects_dir || path == home_path {
        return String::new();
    }

    if let Ok(rel) = path.strip_prefix(&projects_dir) {
        if let Some(first) = rel.components().next() {
            return projects_dir
                .join(first.as_os_str())
                .to_string_lossy()
                .to_string();
        }
    }

    cwd.to_string()
}

fn pane_reserves_slot(pane_state: &PaneState) -> bool {
    pane_state.tmux_target.is_some()
        || matches!(
            pane_state.status.trim(),
            "active" | "running" | "done" | "error" | "stuck" | "blocked" | "live"
        )
        || pane_state
            .workspace_path
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        || pane_state
            .dxos_session_id
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        || pane_state
            .provider
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        || !pane_state.project.trim().is_empty() && pane_state.project != "--"
        || !pane_state.project_path.trim().is_empty()
        || !pane_state.task.trim().is_empty()
}

fn synthesize_pane_state(pane: u8, live: &LivePane) -> PaneState {
    let provider =
        tmux::infer_provider(&live.command, &live.window_name, live.jsonl_path.as_deref())
            .to_string();
    let project_cwd = live_project_cwd(live);
    let project_path = project_root_from_cwd(&project_cwd);

    PaneState {
        theme: crate::config::theme_name(pane).to_string(),
        project: project_from_cwd(&project_cwd),
        project_path: if project_path.is_empty() {
            project_cwd.clone()
        } else {
            project_path.clone()
        },
        role: String::new(),
        provider: Some(provider.clone()),
        model: None,
        runtime_adapter: Some("tmux_migration_adapter".to_string()),
        dxos_session_id: live.session_id.clone(),
        task: format!("{} in {}", tmux::provider_label(&provider), live.target),
        issue_id: None,
        space: None,
        status: "active".to_string(),
        started_at: None,
        acu_spent: 0.0,
        workspace_path: Some(project_cwd),
        branch_name: None,
        base_branch: None,
        machine_ip: None,
        machine_hostname: None,
        machine_mac: None,
        tmux_target: Some(live.target.clone()),
        ..Default::default()
    }
}

fn enrich_pane_state(pane_state: &mut PaneState, pane: u8, live: &LivePane) {
    let provider =
        tmux::infer_provider(&live.command, &live.window_name, live.jsonl_path.as_deref())
            .to_string();
    let project_cwd = live_project_cwd(live);
    let project = project_from_cwd(&project_cwd);
    let project_path = project_root_from_cwd(&project_cwd);

    pane_state.theme = crate::config::theme_name(pane).to_string();
    pane_state.project = project;
    pane_state.project_path = if project_path.is_empty() {
        project_cwd.clone()
    } else {
        project_path
    };
    pane_state.provider = Some(provider.clone());
    pane_state.runtime_adapter = Some("tmux_migration_adapter".to_string());
    if let Some(session_id) = live
        .session_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        pane_state.dxos_session_id = Some(session_id.clone());
    } else if pane_state.dxos_session_id.is_none() {
        pane_state.dxos_session_id = live.session_id.clone();
    }
    if pane_state.task.trim().is_empty()
        || pane_state.status != "active"
        || pane_state.tmux_target.as_deref() != Some(live.target.as_str())
    {
        pane_state.task = format!("{} in {}", tmux::provider_label(&provider), live.target);
    }
    pane_state.status = "active".to_string();
    pane_state.workspace_path = Some(project_cwd);
    pane_state.tmux_target = Some(live.target.clone());
}

fn live_project_cwd(live: &LivePane) -> String {
    if !live.cwd.trim().is_empty() {
        live.cwd.clone()
    } else {
        live.jsonl_path
            .as_deref()
            .and_then(tmux::read_jsonl_cwd)
            .unwrap_or_default()
    }
}

fn next_unused_pane(used_panes: &BTreeSet<u8>) -> u8 {
    for candidate in 1..=u8::MAX {
        if !used_panes.contains(&candidate) {
            return candidate;
        }
    }
    u8::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_all_live_panes_without_state() {
        let state = DxTerminalState::default();
        let live = vec![
            LivePane {
                target: "dx:3.1".into(),
                session: "dx".into(),
                window: 3,
                pane_idx: 1,
                window_name: "codex".into(),
                command: "codex".into(),
                cwd: "/tmp/project-a".into(),
                pid: 10,
                jsonl_path: None,
                session_id: None,
            },
            LivePane {
                target: "dx:1.1".into(),
                session: "dx".into(),
                window: 1,
                pane_idx: 1,
                window_name: "claude".into(),
                command: "claude".into(),
                cwd: "/tmp/project-b".into(),
                pid: 20,
                jsonl_path: None,
                session_id: None,
            },
        ];

        let resolved = resolve_runtime_panes(&state, &live, None);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].pane, 1);
        assert_eq!(resolved[0].tmux_target(), Some("dx:1.1"));
        assert_eq!(resolved[1].pane, 2);
        assert_eq!(resolved[1].tmux_target(), Some("dx:3.1"));
    }

    #[test]
    fn preserves_state_panes_above_default_range() {
        let mut state = DxTerminalState::default();
        let mut pane = PaneState::default();
        pane.status = "active".into();
        pane.tmux_target = Some("dx:12.1".into());
        pane.project = "demo".into();
        state.panes.insert("12".into(), pane);

        let live = vec![LivePane {
            target: "dx:12.1".into(),
            session: "dx".into(),
            window: 12,
            pane_idx: 1,
            window_name: "claude".into(),
            command: "claude".into(),
            cwd: "/tmp/demo".into(),
            pid: 12,
            jsonl_path: None,
            session_id: None,
        }];

        let resolved = resolve_runtime_panes(&state, &live, None);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].pane, 12);
        assert_eq!(resolved[0].tmux_target(), Some("dx:12.1"));
        assert!(resolved[0].state_backed);
    }

    #[test]
    fn reuses_previous_assignments_for_live_only_targets() {
        let state = DxTerminalState::default();
        let live = vec![LivePane {
            target: "dx:7.1".into(),
            session: "dx".into(),
            window: 7,
            pane_idx: 1,
            window_name: "claude".into(),
            command: "claude".into(),
            cwd: "/tmp/demo".into(),
            pid: 7,
            jsonl_path: None,
            session_id: None,
        }];
        let previous = HashMap::from([(24u8, "dx:7.1".to_string())]);

        let resolved = resolve_runtime_panes(&state, &live, Some(&previous));
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].pane, 24);
        assert_eq!(resolved[0].tmux_target(), Some("dx:7.1"));
        assert!(!resolved[0].state_backed);
    }

    #[test]
    fn reuses_idle_slots_before_allocating_new_ones() {
        let mut state = DxTerminalState::default();
        state.panes.insert("1".into(), PaneState::default());
        state.panes.insert("2".into(), PaneState::default());

        let mut reserved = PaneState::default();
        reserved.status = "active".into();
        reserved.tmux_target = Some("dx:9.1".into());
        state.panes.insert("9".into(), reserved);

        let live = vec![
            LivePane {
                target: "dx:1.1".into(),
                session: "dx".into(),
                window: 1,
                pane_idx: 1,
                window_name: "claude".into(),
                command: "claude".into(),
                cwd: "/Users/pran/Projects/alpha".into(),
                pid: 1,
                jsonl_path: None,
                session_id: None,
            },
            LivePane {
                target: "dx:3.1".into(),
                session: "dx".into(),
                window: 3,
                pane_idx: 1,
                window_name: "codex".into(),
                command: "codex".into(),
                cwd: "/Users/pran/Projects/beta".into(),
                pid: 3,
                jsonl_path: None,
                session_id: None,
            },
            LivePane {
                target: "dx:9.1".into(),
                session: "dx".into(),
                window: 9,
                pane_idx: 1,
                window_name: "claude".into(),
                command: "claude".into(),
                cwd: "/Users/pran/Projects/gamma".into(),
                pid: 9,
                jsonl_path: None,
                session_id: None,
            },
        ];

        let resolved = resolve_runtime_panes(&state, &live, None);
        let panes: Vec<(u8, Option<&str>)> = resolved
            .iter()
            .filter(|record| record.live.is_some())
            .map(|record| (record.pane, record.tmux_target()))
            .collect();
        assert_eq!(
            panes,
            vec![
                (1, Some("dx:1.1")),
                (2, Some("dx:3.1")),
                (9, Some("dx:9.1"))
            ]
        );
    }

    #[test]
    fn project_root_from_nested_projects_path_is_stable() {
        assert_eq!(
            project_from_cwd("/Users/pran/Projects/social-media-autopilot/worktrees/feat"),
            "social-media-autopilot"
        );
        assert_eq!(
            project_root_from_cwd("/Users/pran/Projects/social-media-autopilot/worktrees/feat"),
            "/Users/pran/Projects/social-media-autopilot".to_string()
        );
    }

    #[test]
    fn live_cwd_wins_over_generic_jsonl_project_metadata() {
        let mut state = DxTerminalState::default();
        let mut pane = PaneState::default();
        pane.project = "stale-project".into();
        state.panes.insert("2".into(), pane);

        let live = vec![LivePane {
            target: "dx:2.1".into(),
            session: "dx".into(),
            window: 2,
            pane_idx: 1,
            window_name: "claude".into(),
            command: "claude".into(),
            cwd: "/Users/pran/Projects/social-media-autopilot".into(),
            pid: 2,
            jsonl_path: None,
            session_id: None,
        }];

        let resolved = resolve_runtime_panes(&state, &live, None);
        assert_eq!(resolved[0].pane_state.project, "social-media-autopilot");
        assert_eq!(
            resolved[0].pane_state.project_path,
            "/Users/pran/Projects/social-media-autopilot".to_string()
        );
    }

    #[test]
    fn generic_projects_root_clears_stale_project_name() {
        let mut state = DxTerminalState::default();
        let mut pane = PaneState::default();
        pane.project = "stale-project".into();
        state.panes.insert("4".into(), pane);

        let live = vec![LivePane {
            target: "dx:4.1".into(),
            session: "dx".into(),
            window: 4,
            pane_idx: 1,
            window_name: "claude".into(),
            command: "claude".into(),
            cwd: "/Users/pran/Projects".into(),
            pid: 4,
            jsonl_path: None,
            session_id: None,
        }];

        let resolved = resolve_runtime_panes(&state, &live, None);
        assert_eq!(resolved[0].pane_state.project, "--");
    }
}
