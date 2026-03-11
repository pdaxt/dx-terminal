use std::sync::mpsc;
use std::time::{Duration, Instant};

// Re-use the actual crate types
use dx_terminal::tui::input;
use dx_terminal::tui::{PendingAction, TuiCommand, TuiMode, TuiResult, ViewMode};

// ========== Command Parsing ==========

#[test]
fn test_parse_spawn_command() {
    let cmd = input::parse_command("spawn 3 dataxlr8").unwrap();
    match cmd {
        TuiCommand::Spawn {
            pane,
            project,
            role,
            task,
        } => {
            assert_eq!(pane, "3");
            assert_eq!(project, "dataxlr8");
            assert!(role.is_none());
            assert!(task.is_none());
        }
        _ => panic!("Expected Spawn command"),
    }
}

#[test]
fn test_parse_spawn_shorthand() {
    let cmd = input::parse_command("s 1 myproject").unwrap();
    match cmd {
        TuiCommand::Spawn { pane, project, .. } => {
            assert_eq!(pane, "1");
            assert_eq!(project, "myproject");
        }
        _ => panic!("Expected Spawn command"),
    }
}

#[test]
fn test_parse_kill_command() {
    let cmd = input::parse_command("kill 3").unwrap();
    match cmd {
        TuiCommand::Kill { pane, reason } => {
            assert_eq!(pane, "3");
            assert!(reason.is_none());
        }
        _ => panic!("Expected Kill command"),
    }
}

#[test]
fn test_parse_kill_with_reason() {
    let cmd = input::parse_command("kill 5 agent is stuck").unwrap();
    match cmd {
        TuiCommand::Kill { pane, reason } => {
            assert_eq!(pane, "5");
            assert_eq!(reason.unwrap(), "agent is stuck");
        }
        _ => panic!("Expected Kill command"),
    }
}

#[test]
fn test_parse_done_command() {
    let cmd = input::parse_command("done 2").unwrap();
    match cmd {
        TuiCommand::Complete { pane, summary } => {
            assert_eq!(pane, "2");
            assert!(summary.is_none());
        }
        _ => panic!("Expected Complete command"),
    }
}

#[test]
fn test_parse_complete_with_summary() {
    let cmd = input::parse_command("complete 4 auth feature done").unwrap();
    match cmd {
        TuiCommand::Complete { pane, summary } => {
            assert_eq!(pane, "4");
            assert_eq!(summary.unwrap(), "auth feature done");
        }
        _ => panic!("Expected Complete command"),
    }
}

#[test]
fn test_parse_auto_cycle() {
    let cmd = input::parse_command("auto").unwrap();
    assert!(matches!(cmd, TuiCommand::AutoCycle));

    let cmd2 = input::parse_command("cycle").unwrap();
    assert!(matches!(cmd2, TuiCommand::AutoCycle));
}

#[test]
fn test_parse_invalid_commands() {
    assert!(input::parse_command("").is_none());
    // "invalid" now routes to McpDispatch (universal dispatch)
    assert!(matches!(
        input::parse_command("invalid"),
        Some(TuiCommand::McpDispatch { .. })
    ));
    // spawn/kill without enough args also route to McpDispatch
    assert!(matches!(
        input::parse_command("spawn"),
        Some(TuiCommand::McpDispatch { .. })
    ));
    assert!(matches!(
        input::parse_command("spawn 3"),
        Some(TuiCommand::McpDispatch { .. })
    ));
    assert!(matches!(
        input::parse_command("kill"),
        Some(TuiCommand::McpDispatch { .. })
    ));
}

#[test]
fn test_parse_case_insensitive() {
    assert!(input::parse_command("SPAWN 1 proj").is_some());
    assert!(input::parse_command("Kill 3").is_some());
    assert!(input::parse_command("AUTO").is_some());
}

// ========== Form Creation ==========

#[test]
fn test_spawn_form_creation() {
    let form = input::create_spawn_form(3);
    assert_eq!(form.title, "Spawn Agent");
    assert_eq!(form.fields.len(), 4);
    assert_eq!(form.fields[0].value, "3"); // Pane pre-filled
    assert_eq!(form.fields[0].label, "Pane");
    assert!(form.fields[0].required);
    assert_eq!(form.fields[1].label, "Project");
    assert!(form.fields[1].required);
    assert_eq!(form.fields[2].label, "Role");
    assert!(!form.fields[2].required);
    assert_eq!(form.fields[2].value, "developer"); // Default
    assert_eq!(form.fields[3].label, "Task");
    assert!(!form.fields[3].required);
    assert_eq!(form.focused, 1); // Focus on Project, not Pane
}

#[test]
fn test_queue_form_creation() {
    let form = input::create_queue_form();
    assert_eq!(form.title, "Add to Queue");
    assert_eq!(form.fields.len(), 4);
    assert!(form.fields[0].required); // Project
    assert!(form.fields[1].required); // Task
    assert!(!form.fields[2].required); // Role
    assert!(!form.fields[3].required); // Priority
    assert_eq!(form.fields[3].value, "3"); // Default priority
    assert_eq!(form.focused, 0);
}

// ========== Form Validation ==========

#[test]
fn test_form_validation_valid() {
    let mut form = input::create_spawn_form(1);
    form.fields[1].value = "dataxlr8".into(); // Fill required Project
    assert!(input::form_is_valid(&form));
}

#[test]
fn test_form_validation_missing_required() {
    let form = input::create_spawn_form(1);
    // Project (fields[1]) is empty and required
    assert!(!input::form_is_valid(&form));
}

#[test]
fn test_form_validation_whitespace_only() {
    let mut form = input::create_spawn_form(1);
    form.fields[1].value = "   ".into(); // Whitespace-only
    assert!(!input::form_is_valid(&form));
}

#[test]
fn test_queue_form_validation() {
    let mut form = input::create_queue_form();
    // Both Project and Task are required
    assert!(!input::form_is_valid(&form));

    form.fields[0].value = "myproject".into();
    assert!(!input::form_is_valid(&form)); // Task still empty

    form.fields[1].value = "fix the bug".into();
    assert!(input::form_is_valid(&form)); // Both filled
}

// ========== Form → Command Conversion ==========

#[test]
fn test_spawn_form_to_command() {
    let mut form = input::create_spawn_form(5);
    form.fields[1].value = "dataxlr8".into();
    form.fields[3].value = "Fix auth".into();

    let cmd = input::form_to_command(&form).unwrap();
    match cmd {
        TuiCommand::Spawn {
            pane,
            project,
            role,
            task,
        } => {
            assert_eq!(pane, "5");
            assert_eq!(project, "dataxlr8");
            assert_eq!(role.unwrap(), "developer"); // Default
            assert_eq!(task.unwrap(), "Fix auth");
        }
        _ => panic!("Expected Spawn"),
    }
}

#[test]
fn test_queue_form_to_command() {
    let mut form = input::create_queue_form();
    form.fields[0].value = "bskiller".into();
    form.fields[1].value = "Add login page".into();

    let cmd = input::form_to_command(&form).unwrap();
    match cmd {
        TuiCommand::QueueAdd {
            project,
            task,
            role,
            priority,
        } => {
            assert_eq!(project, "bskiller");
            assert_eq!(task, "Add login page");
            assert_eq!(role.unwrap(), "developer");
            assert_eq!(priority.unwrap(), 3);
        }
        _ => panic!("Expected QueueAdd"),
    }
}

#[test]
fn test_form_to_command_empty_optional_fields() {
    let mut form = input::create_spawn_form(2);
    form.fields[1].value = "proj".into();
    form.fields[2].value = String::new(); // Clear role
    form.fields[3].value = String::new(); // Clear task

    let cmd = input::form_to_command(&form).unwrap();
    match cmd {
        TuiCommand::Spawn { role, task, .. } => {
            assert!(role.is_none());
            assert!(task.is_none());
        }
        _ => panic!("Expected Spawn"),
    }
}

// ========== Extract Summary ==========

#[test]
fn test_extract_summary_json_status() {
    let result = TuiResult {
        description: "Spawn P3 dataxlr8".into(),
        success: true,
        message: r#"{"status":"spawned","pane":3}"#.into(),
    };
    let summary = extract_summary_test(&result);
    assert!(summary.contains("spawned"));
    assert!(summary.contains("P3"));
}

#[test]
fn test_extract_summary_json_error() {
    let result = TuiResult {
        description: "Kill P5".into(),
        success: false,
        message: r#"{"error":"pane 5 not active"}"#.into(),
    };
    let summary = extract_summary_test(&result);
    assert!(summary.contains("pane 5 not active"));
}

#[test]
fn test_extract_summary_raw_message() {
    let result = TuiResult {
        description: "Auto-cycle".into(),
        success: true,
        message: "cycle completed with 0 actions".into(),
    };
    let summary = extract_summary_test(&result);
    assert!(summary.contains("Auto-cycle"));
    assert!(summary.contains("cycle completed"));
}

#[test]
fn test_extract_summary_long_message_truncated() {
    let result = TuiResult {
        description: "Test".into(),
        success: true,
        message: "x".repeat(200),
    };
    let summary = extract_summary_test(&result);
    assert!(summary.len() < 100); // Truncated
}

// Re-implement extract_summary for testing (since it's private)
fn extract_summary_test(result: &TuiResult) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result.message) {
        if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
            let pane = v
                .get("pane")
                .and_then(|p| p.as_u64())
                .map(|p| format!(" P{}", p))
                .unwrap_or_default();
            return format!("{}: {}{}", result.description, status, pane);
        }
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return format!("{}: {}", result.description, err);
        }
    }
    let msg: String = result.message.chars().take(60).collect();
    format!("{}: {}", result.description, msg)
}

// ========== Channel Communication ==========

#[test]
fn test_command_channel_roundtrip() {
    let (tx, rx) = mpsc::channel::<TuiCommand>();

    tx.send(TuiCommand::AutoCycle).unwrap();
    tx.send(TuiCommand::Kill {
        pane: "3".into(),
        reason: None,
    })
    .unwrap();

    let cmd1 = rx.recv_timeout(Duration::from_millis(100)).unwrap();
    assert!(matches!(cmd1, TuiCommand::AutoCycle));

    let cmd2 = rx.recv_timeout(Duration::from_millis(100)).unwrap();
    match cmd2 {
        TuiCommand::Kill { pane, .. } => assert_eq!(pane, "3"),
        _ => panic!("Expected Kill"),
    }
}

#[test]
fn test_result_channel_roundtrip() {
    let (tx, rx) = mpsc::channel::<TuiResult>();

    tx.send(TuiResult {
        description: "Test".into(),
        success: true,
        message: "ok".into(),
    })
    .unwrap();

    let result = rx.recv_timeout(Duration::from_millis(100)).unwrap();
    assert!(result.success);
    assert_eq!(result.message, "ok");
}

#[test]
fn test_result_try_recv_nonblocking() {
    let (_tx, rx) = mpsc::channel::<TuiResult>();

    // Should not block — just returns Err
    let result = rx.try_recv();
    assert!(result.is_err());
}

// ========== TuiMode State Machine ==========

#[test]
fn test_mode_starts_navigate() {
    let mode = TuiMode::Navigate;
    assert!(matches!(mode, TuiMode::Navigate));
}

#[test]
fn test_mode_command_transition() {
    let mode = TuiMode::Command {
        input: "spawn".into(),
        cursor: 5,
        completions: Vec::new(),
        comp_idx: None,
    };
    match &mode {
        TuiMode::Command { input, cursor, .. } => {
            assert_eq!(input, "spawn");
            assert_eq!(*cursor, 5);
        }
        _ => panic!("Expected Command mode"),
    }
}

#[test]
fn test_mode_confirm_transition() {
    let mode = TuiMode::Confirm {
        action: PendingAction::Kill { pane: 3 },
        message: "Kill pane 3?".into(),
    };
    match &mode {
        TuiMode::Confirm { action, message } => {
            assert_eq!(message, "Kill pane 3?");
            match action {
                PendingAction::Kill { pane } => assert_eq!(*pane, 3),
                _ => panic!("Expected Kill action"),
            }
        }
        _ => panic!("Expected Confirm mode"),
    }
}

#[test]
fn test_mode_result_auto_dismiss() {
    let mode = TuiMode::Result {
        message: "Done".into(),
        is_error: false,
        shown_at: Instant::now() - Duration::from_secs(10),
    };
    if let TuiMode::Result { shown_at, .. } = &mode {
        assert!(shown_at.elapsed() >= Duration::from_secs(4));
        // In run_loop, this would trigger transition to Navigate
    }
}

// ========== ViewMode ==========

#[test]
fn test_view_mode_toggle() {
    let mut vm = ViewMode::Normal;

    // Toggle to Features
    vm = if vm == ViewMode::Features {
        ViewMode::Normal
    } else {
        ViewMode::Features
    };
    assert_eq!(vm, ViewMode::Features);

    // Toggle back
    vm = if vm == ViewMode::Features {
        ViewMode::Normal
    } else {
        ViewMode::Features
    };
    assert_eq!(vm, ViewMode::Normal);

    // Toggle to Board
    vm = if vm == ViewMode::Board {
        ViewMode::Normal
    } else {
        ViewMode::Board
    };
    assert_eq!(vm, ViewMode::Board);

    // Toggle to Coord (replaces Board)
    vm = if vm == ViewMode::Coord {
        ViewMode::Normal
    } else {
        ViewMode::Coord
    };
    assert_eq!(vm, ViewMode::Coord);

    // Toggle Projects
    vm = if vm == ViewMode::Projects {
        ViewMode::Normal
    } else {
        ViewMode::Projects
    };
    assert_eq!(vm, ViewMode::Projects);
}

// ========== Form Field Cursor ==========

#[test]
fn test_field_cursor_operations() {
    let mut form = input::create_spawn_form(1);
    let field = &mut form.fields[1]; // Project field

    // Type "abc"
    field.value.insert(field.cursor, 'a');
    field.cursor += 1;
    field.value.insert(field.cursor, 'b');
    field.cursor += 1;
    field.value.insert(field.cursor, 'c');
    field.cursor += 1;

    assert_eq!(field.value, "abc");
    assert_eq!(field.cursor, 3);

    // Move left
    field.cursor -= 1;
    assert_eq!(field.cursor, 2);

    // Insert at cursor position
    field.value.insert(field.cursor, 'X');
    field.cursor += 1;
    assert_eq!(field.value, "abXc");
    assert_eq!(field.cursor, 3);

    // Backspace
    field.value.remove(field.cursor - 1);
    field.cursor -= 1;
    assert_eq!(field.value, "abc");
    assert_eq!(field.cursor, 2);

    // Home
    field.cursor = 0;
    assert_eq!(field.cursor, 0);

    // End
    field.cursor = field.value.len();
    assert_eq!(field.cursor, 3);
}

#[test]
fn test_field_cursor_boundary() {
    let mut form = input::create_spawn_form(1);
    let field = &mut form.fields[1];

    // Cursor at 0, backspace should not go negative
    field.cursor = 0;
    if field.cursor > 0 {
        field.cursor -= 1;
    }
    assert_eq!(field.cursor, 0); // Still 0

    // Cursor at end, right should not exceed length
    field.value = "test".into();
    field.cursor = field.value.len();
    if field.cursor < field.value.len() {
        field.cursor += 1;
    }
    assert_eq!(field.cursor, 4); // Still at end
}

// ========== Tab Navigation ==========

#[test]
fn test_tab_cycles_fields() {
    let mut form = input::create_spawn_form(1);
    assert_eq!(form.focused, 1); // Starts on Project

    // Tab forward
    form.focused = (form.focused + 1) % form.fields.len();
    assert_eq!(form.focused, 2); // Role

    form.focused = (form.focused + 1) % form.fields.len();
    assert_eq!(form.focused, 3); // Task

    form.focused = (form.focused + 1) % form.fields.len();
    assert_eq!(form.focused, 0); // Wraps to Pane

    // BackTab
    form.focused = if form.focused == 0 {
        form.fields.len() - 1
    } else {
        form.focused - 1
    };
    assert_eq!(form.focused, 3); // Wraps to Task
}

// ========== PendingAction ==========

#[test]
fn test_pending_action_clone() {
    let action = PendingAction::Kill { pane: 5 };
    let cloned = action.clone();
    match cloned {
        PendingAction::Kill { pane } => assert_eq!(pane, 5),
        _ => panic!("Expected Kill"),
    }

    let action2 = PendingAction::Complete { pane: 2 };
    let cloned2 = action2.clone();
    match cloned2 {
        PendingAction::Complete { pane } => assert_eq!(pane, 2),
        _ => panic!("Expected Complete"),
    }
}
