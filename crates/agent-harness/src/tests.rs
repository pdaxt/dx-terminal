#[cfg(test)]
mod tests {
    use crate::compact::{compact_session, should_compact, CompactionConfig};
    use crate::permissions::{
        PermissionMode, PermissionOutcome, PermissionPolicy, PermissionPrompter,
    };
    use crate::runtime::{
        ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeEvent, RuntimeListener,
    };
    use dx_agent_core::{ConversationMessage, Session, TokenUsage};

    // ── Permission tests ──

    #[test]
    fn allows_tools_within_permission_level() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
            .with_tool("read_file", PermissionMode::ReadOnly)
            .with_tool("write_file", PermissionMode::WorkspaceWrite);

        assert!(matches!(
            policy.authorize("read_file", "{}", None),
            PermissionOutcome::Allow
        ));
        assert!(matches!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Allow
        ));
    }

    #[test]
    fn denies_tools_above_permission_level() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_tool("bash", PermissionMode::FullAccess);

        assert!(matches!(
            policy.authorize("bash", "{}", None),
            PermissionOutcome::Deny { .. }
        ));
    }

    #[test]
    fn unknown_tools_default_to_full_access() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        assert!(matches!(
            policy.authorize("unknown_tool", "{}", None),
            PermissionOutcome::Deny { .. }
        ));
    }

    #[test]
    fn full_access_allows_everything() {
        let policy = PermissionPolicy::new(PermissionMode::FullAccess)
            .with_tool("bash", PermissionMode::FullAccess);

        assert!(matches!(
            policy.authorize("bash", "{}", None),
            PermissionOutcome::Allow
        ));
    }

    struct AlwaysAllowPrompter;
    impl PermissionPrompter for AlwaysAllowPrompter {
        fn decide(&mut self, _tool_name: &str, _input: &str) -> PermissionOutcome {
            PermissionOutcome::Allow
        }
    }

    #[test]
    fn workspace_write_can_prompt_for_full_access() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
            .with_tool("bash", PermissionMode::FullAccess);

        let mut prompter = AlwaysAllowPrompter;
        assert!(matches!(
            policy.authorize("bash", "{}", Some(&mut prompter)),
            PermissionOutcome::Allow
        ));
    }

    // ── Compaction tests ──

    #[test]
    fn should_compact_returns_false_when_below_threshold() {
        let session = Session::new();
        let config = CompactionConfig::default();
        assert!(!should_compact(&session, &config));
    }

    #[test]
    fn should_compact_returns_true_when_above_threshold() {
        let mut session = Session::new();
        for i in 0..101 {
            session
                .messages
                .push(ConversationMessage::user(format!("msg {i}")));
        }
        let config = CompactionConfig {
            max_messages: 100,
            ..Default::default()
        };
        assert!(should_compact(&session, &config));
    }

    #[test]
    fn compact_session_keeps_recent_messages() {
        let mut session = Session::new();
        for i in 0..200 {
            session
                .messages
                .push(ConversationMessage::user(format!("msg {i}")));
        }
        let config = CompactionConfig {
            max_messages: 30,
            keep_recent: 10,
            context_window: 100,
            tokens_per_message: 200,
            output_buffer: 10,
        };
        compact_session(&mut session, &config);
        // Emergency compact: 1 notice + 10 recent = 11
        assert!(session.messages.len() <= 11);
    }

    // ── Runtime tests ──

    struct MockApi {
        responses: Vec<Vec<AssistantEvent>>,
        call_count: usize,
    }

    impl ApiClient for MockApi {
        fn stream(&mut self, _request: ApiRequest) -> dx_agent_core::Result<Vec<AssistantEvent>> {
            let idx = self.call_count;
            self.call_count += 1;
            Ok(self.responses.get(idx).cloned().unwrap_or_else(|| {
                vec![
                    AssistantEvent::TextDelta("done".to_string()),
                    AssistantEvent::Stop,
                ]
            }))
        }
    }

    struct EventRecorder {
        events: Vec<String>,
    }

    impl RuntimeListener for EventRecorder {
        fn on_event(&mut self, event: RuntimeEvent<'_>) {
            match event {
                RuntimeEvent::Thinking => self.events.push("thinking".to_string()),
                RuntimeEvent::Text(t) => self.events.push(format!("text:{t}")),
                RuntimeEvent::ToolCall { name, .. } => self.events.push(format!("tool:{name}")),
                RuntimeEvent::ToolResult { name, success } => {
                    self.events.push(format!("result:{name}:{success}"))
                }
                RuntimeEvent::Done => self.events.push("done".to_string()),
            }
        }
    }

    #[test]
    fn runtime_handles_text_only_response() {
        let api = MockApi {
            responses: vec![vec![
                AssistantEvent::TextDelta("hello world".to_string()),
                AssistantEvent::Usage(TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }),
                AssistantEvent::Stop,
            ]],
            call_count: 0,
        };

        let policy = PermissionPolicy::new(PermissionMode::FullAccess);
        let mut runtime = ConversationRuntime::new(
            api,
            policy,
            vec!["test".to_string()],
            vec![],
            std::path::PathBuf::from("/tmp"),
        );

        let summary = runtime.run_turn("hi", None).expect("run turn");
        assert_eq!(summary.text, "hello world");
        assert_eq!(summary.tool_calls, 0);
        assert_eq!(summary.iterations, 1);
        assert_eq!(summary.usage.total_tokens(), 15);
    }

    #[test]
    fn runtime_emits_events_to_listener() {
        let api = MockApi {
            responses: vec![vec![
                AssistantEvent::TextDelta("hello".to_string()),
                AssistantEvent::Stop,
            ]],
            call_count: 0,
        };

        let policy = PermissionPolicy::new(PermissionMode::FullAccess);
        let mut runtime = ConversationRuntime::new(
            api,
            policy,
            vec!["test".to_string()],
            vec![],
            std::path::PathBuf::from("/tmp"),
        );

        let mut recorder = EventRecorder { events: vec![] };
        let summary = runtime
            .run_turn_with_listener("hi", &mut recorder)
            .expect("run turn");

        assert_eq!(summary.text, "hello");
        // Spinner now runs in a background thread, not via listener events
        assert!(recorder.events.contains(&"done".to_string()));
    }

    #[test]
    fn runtime_respects_max_iterations() {
        // API always returns a tool call, creating an infinite loop
        let api = MockApi {
            responses: (0..20)
                .map(|_| {
                    vec![
                        AssistantEvent::ToolUse {
                            id: "t1".to_string(),
                            name: "nonexistent".to_string(),
                            input: "{}".to_string(),
                        },
                        AssistantEvent::Stop,
                    ]
                })
                .collect(),
            call_count: 0,
        };

        let policy = PermissionPolicy::new(PermissionMode::FullAccess);
        let mut runtime = ConversationRuntime::new(
            api,
            policy,
            vec!["test".to_string()],
            vec![],
            std::path::PathBuf::from("/tmp"),
        )
        .with_max_iterations(3);

        let result = runtime.run_turn("loop forever", None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Turn limit exceeded"));
    }
}
