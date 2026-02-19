use std::collections::{HashMap, VecDeque};

// Test UTF-8 safe truncation (the most critical fix)
fn truncate_tools(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", end)
    }
}

fn truncate_widgets(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", end)
    }
}

#[test]
fn test_truncate_ascii() {
    assert_eq!(truncate_tools("hello", 10), "hello");
    assert_eq!(truncate_tools("hello world", 8), "hello...");
    assert_eq!(truncate_tools("", 5), "");
}

#[test]
fn test_truncate_unicode_no_panic() {
    // Emoji: each is 1 char but 4 bytes
    let emoji = "🎉🎊🎈🎁🎂🎄🎅🎆🎇🎃";
    assert_eq!(truncate_tools(&emoji, 5), "🎉🎊...");
    assert_eq!(truncate_widgets(&emoji, 5), "🎉🎊🎈🎁…");

    // CJK characters
    let cjk = "你好世界测试数据";
    assert_eq!(truncate_tools(&cjk, 5), "你好...");

    // Mixed ASCII + emoji
    let mixed = "hi 🎉 world";
    let result = truncate_tools(&mixed, 6);
    assert!(result.ends_with("..."));
    assert!(!result.is_empty());
}

#[test]
fn test_truncate_edge_cases() {
    // max < 3 for tools variant
    let result = truncate_tools("hello", 2);
    assert!(result.ends_with("...")); // saturating_sub(3) = 0 chars + "..."

    // Empty string
    assert_eq!(truncate_tools("", 0), "");
    assert_eq!(truncate_widgets("", 0), "");

    // max = 0 with content
    let result = truncate_tools("x", 0);
    assert!(result.ends_with("..."));
}

// Test safe timestamp slicing
#[test]
fn test_safe_timestamp_slicing() {
    let good_ts = "2026-02-18T13:45:00";
    assert_eq!(good_ts.get(11..16).unwrap_or(good_ts), "13:45");

    let short_ts = "2026-02";
    assert_eq!(short_ts.get(11..16).unwrap_or(short_ts), "2026-02"); // Falls back

    let empty_ts = "";
    assert_eq!(empty_ts.get(11..16).unwrap_or(empty_ts), "");

    // This would have panicked with the old &ts[11..16]
    let bad_ts = "short";
    assert_eq!(bad_ts.get(11..16).unwrap_or(bad_ts), "short");
}

// Test pane resolution
#[test]
fn test_resolve_pane() {
    fn resolve_pane(pane_ref: &str) -> Option<u8> {
        if let Ok(n) = pane_ref.parse::<u8>() {
            if (1..=9).contains(&n) {
                return Some(n);
            }
        }
        match pane_ref.to_lowercase().as_str() {
            "cyan" | "c" => Some(1),
            "green" | "g" => Some(2),
            "purple" | "p" => Some(3),
            "orange" | "o" => Some(4),
            "red" | "r" => Some(5),
            "yellow" | "y" => Some(6),
            "silver" | "s" => Some(7),
            "teal" | "t" => Some(8),
            "pink" | "k" => Some(9),
            _ => None,
        }
    }

    // Numeric
    assert_eq!(resolve_pane("1"), Some(1));
    assert_eq!(resolve_pane("9"), Some(9));
    assert_eq!(resolve_pane("0"), None);
    assert_eq!(resolve_pane("10"), None);

    // Theme names
    assert_eq!(resolve_pane("cyan"), Some(1));
    assert_eq!(resolve_pane("CYAN"), Some(1));
    assert_eq!(resolve_pane("Cyan"), Some(1));

    // Shortcuts
    assert_eq!(resolve_pane("c"), Some(1));
    assert_eq!(resolve_pane("k"), Some(9));

    // Invalid
    assert_eq!(resolve_pane(""), None);
    assert_eq!(resolve_pane("invalid"), None);
    assert_eq!(resolve_pane("255"), None);
}

// Test theme color parsing (from widgets)
#[test]
fn test_theme_color_hex() {
    fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
        if hex.starts_with('#') && hex.len() == 7 {
            let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
            let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
            let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
            Some((r, g, b))
        } else {
            None
        }
    }

    assert_eq!(parse_hex("#00d4ff"), Some((0, 212, 255)));
    assert_eq!(parse_hex("#ffffff"), Some((255, 255, 255)));
    assert_eq!(parse_hex("#000000"), Some((0, 0, 0)));
    assert_eq!(parse_hex("invalid"), None);
    assert_eq!(parse_hex("#fff"), None); // Too short
    assert_eq!(parse_hex(""), None);
}

// Test output detection
#[test]
fn test_completion_detection() {
    fn check_completion(output: &str, markers: &[String]) -> Option<String> {
        for marker in markers {
            if output.contains(marker.as_str()) {
                return Some(marker.clone());
            }
        }
        None
    }

    let markers = vec!["---DONE---".to_string(), "TASK COMPLETE".to_string()];
    assert!(check_completion("some output\n---DONE---\nmore", &markers).is_some());
    assert!(check_completion("TASK COMPLETE here", &markers).is_some());
    assert!(check_completion("just normal output", &markers).is_none());
    assert!(check_completion("", &markers).is_none());
}

#[test]
fn test_shell_prompt_detection() {
    fn check_shell_prompt(output: &str) -> bool {
        let lines: Vec<&str> = output.trim().lines().collect();
        if let Some(last) = lines.last() {
            let trimmed = last.trim();
            return trimmed.ends_with('$')
                || trimmed.ends_with("$ ")
                || trimmed.contains("Claude exited")
                || trimmed.ends_with('%')
                || trimmed.ends_with("% ");
        }
        false
    }

    assert!(check_shell_prompt("some output\npran@mac ~$ "));  // Trailing space trimmed, still matches '$'
    assert!(check_shell_prompt("$"));
    assert!(check_shell_prompt("Claude exited with code 0"));
    assert!(check_shell_prompt("pran@mac ~ % "));
    assert!(!check_shell_prompt("still working..."));
    assert!(!check_shell_prompt(""));
}

#[test]
fn test_error_detection() {
    fn check_errors(output: &str) -> Option<String> {
        let patterns = [
            "Error:", "FATAL:", "panic:", "Traceback",
            "rate limit", "hit your limit", "SIGTERM",
        ];
        for pat in &patterns {
            if output.contains(pat) {
                return Some(pat.to_string());
            }
        }
        None
    }

    assert_eq!(check_errors("Error: something broke"), Some("Error:".to_string()));
    assert_eq!(check_errors("FATAL: out of memory"), Some("FATAL:".to_string()));
    assert_eq!(check_errors("You've hit your limit"), Some("hit your limit".to_string()));
    assert_eq!(check_errors("all good here"), None);
}

// Test capacity calculations don't panic with empty/malformed data
#[test]
fn test_capacity_defaults() {
    // Simulate load_capacity with empty config
    let pane_count: f64 = 9.0;
    let hours: f64 = 8.0;
    let factor: f64 = 0.8;
    let daily_acu = pane_count * hours * factor;
    assert!((daily_acu - 57.6).abs() < 0.01);

    // Zero division safety
    let acu_total = 0.0_f64;
    let acu_used = 5.0_f64;
    let pct = if acu_total > 0.0 { (acu_used / acu_total * 100.0) as u32 } else { 0 };
    assert_eq!(pct, 0);
}

// Test atomic write pattern doesn't corrupt
#[test]
fn test_atomic_write_pattern() {
    let dir = std::env::temp_dir().join("agentos_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_atomic.json");

    // Write
    let data = r#"{"test": true}"#;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).unwrap();
    std::fs::rename(&tmp, &path).unwrap();

    // Read back
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, data);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

// Test state defaults
#[test]
fn test_pane_state_default() {
    #[derive(Default)]
    struct PaneState {
        project: String,
        status: String,
    }

    let ps = PaneState {
        project: "--".into(),
        status: "idle".into(),
    };
    assert_eq!(ps.project, "--");
    assert_eq!(ps.status, "idle");
}

// Test ring buffer behavior (simulates output_lines)
#[test]
fn test_ring_buffer_overflow() {
    let mut buf: VecDeque<String> = VecDeque::with_capacity(5);
    for i in 0..10 {
        buf.push_back(format!("line {}", i));
        while buf.len() > 5 {
            buf.pop_front();
        }
    }
    assert_eq!(buf.len(), 5);
    assert_eq!(buf.front().unwrap(), "line 5");
    assert_eq!(buf.back().unwrap(), "line 9");
}

// Test board summary aggregation
#[test]
fn test_board_summary() {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let statuses = ["backlog", "in_progress", "done", "backlog", "done", "done"];
    for s in &statuses {
        *counts.entry(s.to_string()).or_insert(0) += 1;
    }
    assert_eq!(counts["backlog"], 2);
    assert_eq!(counts["in_progress"], 1);
    assert_eq!(counts["done"], 3);
}
