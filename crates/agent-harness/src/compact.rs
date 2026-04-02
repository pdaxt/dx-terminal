use dx_agent_core::{ContentBlock, ConversationMessage, MessageRole, Session};

#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Trigger auto-compact when message count exceeds this
    pub max_messages: usize,
    /// Always keep this many recent messages
    pub keep_recent: usize,
    /// Estimated tokens per message (for budget calculations)
    pub tokens_per_message: usize,
    /// Max context window in tokens (model-dependent)
    pub context_window: usize,
    /// Buffer tokens to leave for output
    pub output_buffer: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            max_messages: 80,
            keep_recent: 15,
            tokens_per_message: 200,
            context_window: 32_000, // conservative for local models
            output_buffer: 4_000,
        }
    }
}

/// Check if compaction should trigger.
pub fn should_compact(session: &Session, config: &CompactionConfig) -> bool {
    let estimated_tokens = session.messages.len() * config.tokens_per_message;
    let threshold = config.context_window.saturating_sub(config.output_buffer);

    // Trigger on message count OR estimated token budget
    session.messages.len() > config.max_messages || estimated_tokens > threshold
}

/// Three-layer compaction inspired by production agent architectures.
///
/// Layer 1 (MicroCompact): Trim tool results to summaries
/// Layer 2 (AutoCompact): Summarize old conversation into a single context message
/// Layer 3 (Emergency): Drop everything except system + recent messages
pub fn compact_session(session: &mut Session, config: &CompactionConfig) {
    let msg_count = session.messages.len();
    if msg_count <= config.keep_recent {
        return;
    }

    let estimated_tokens = msg_count * config.tokens_per_message;
    let threshold = config.context_window.saturating_sub(config.output_buffer);

    if estimated_tokens < threshold * 80 / 100 {
        // Under 80% of budget — Layer 1: micro-compact tool results
        micro_compact(session);
    } else if estimated_tokens < threshold {
        // 80-100% of budget — Layer 2: summarize old messages
        auto_compact(session, config);
    } else {
        // Over budget — Layer 3: emergency trim
        emergency_compact(session, config);
    }
}

/// Layer 1: MicroCompact — trim verbose tool results to short summaries.
/// Keeps the conversation flow but reduces bloat from large file reads, grep results, etc.
fn micro_compact(session: &mut Session) {
    for msg in &mut session.messages {
        if msg.role != MessageRole::Tool {
            continue;
        }
        for block in &mut msg.blocks {
            if let ContentBlock::ToolResult {
                output,
                tool_name: _,
                ..
            } = block
            {
                let original_len = output.len();
                if original_len > 2000 {
                    // Trim long tool outputs — keep first and last 500 chars
                    let head = &output[..500];
                    let tail = &output[output.len().saturating_sub(500)..];
                    *output = format!(
                        "{head}\n\n[... {} chars trimmed by compaction ...]\n\n{tail}",
                        original_len - 1000
                    );
                }
            }
        }
    }
}

/// Layer 2: AutoCompact — replace old messages with a summary message.
/// Keeps recent messages intact, summarizes older ones into context.
fn auto_compact(session: &mut Session, config: &CompactionConfig) {
    // First, micro-compact everything
    micro_compact(session);

    let keep_from = session.messages.len().saturating_sub(config.keep_recent);
    if keep_from == 0 {
        return;
    }

    // Build a summary of the old messages
    let old_messages = &session.messages[..keep_from];
    let summary = build_conversation_summary(old_messages);

    // Replace old messages with a single summary message
    let summary_msg = ConversationMessage {
        role: MessageRole::User,
        blocks: vec![ContentBlock::Text {
            text: format!(
                "[Conversation history compacted — {} messages summarized]\n\n{}",
                keep_from, summary
            ),
        }],
        usage: None,
    };

    let mut new_messages = vec![summary_msg];
    new_messages.extend_from_slice(&session.messages[keep_from..]);
    session.messages = new_messages;
}

/// Layer 3: Emergency compact — drop everything except recent messages.
fn emergency_compact(session: &mut Session, config: &CompactionConfig) {
    let keep_from = session.messages.len().saturating_sub(config.keep_recent);
    let emergency_msg = ConversationMessage {
        role: MessageRole::User,
        blocks: vec![ContentBlock::Text {
            text: format!(
                "[Context compacted — {} old messages dropped to stay within token budget]",
                keep_from
            ),
        }],
        usage: None,
    };

    let mut new_messages = vec![emergency_msg];
    new_messages.extend_from_slice(&session.messages[keep_from..]);
    session.messages = new_messages;
}

/// Build a text summary of old conversation messages.
fn build_conversation_summary(messages: &[ConversationMessage]) -> String {
    let mut tool_calls: Vec<String> = Vec::new();
    let mut key_decisions: Vec<String> = Vec::new();

    for msg in messages {
        for block in &msg.blocks {
            match block {
                ContentBlock::Text { text } => {
                    match msg.role {
                        MessageRole::User => {
                            // Keep user requests as context
                            let truncated = if text.len() > 200 {
                                format!("{}...", &text[..197])
                            } else {
                                text.clone()
                            };
                            key_decisions.push(format!("User asked: {truncated}"));
                        }
                        MessageRole::Assistant => {
                            // Keep key decisions from assistant
                            let first_line = text.lines().next().unwrap_or("").trim();
                            if !first_line.is_empty() && first_line.len() > 10 {
                                let truncated = if first_line.len() > 150 {
                                    format!("{}...", &first_line[..147])
                                } else {
                                    first_line.to_string()
                                };
                                key_decisions.push(format!("Agent: {truncated}"));
                            }
                        }
                        _ => {}
                    }
                }
                ContentBlock::ToolUse { name, .. } => {
                    tool_calls.push(name.clone());
                }
                ContentBlock::ToolResult {
                    tool_name,
                    is_error,
                    ..
                } => {
                    if *is_error {
                        tool_calls.push(format!("{tool_name} (failed)"));
                    }
                }
            }
        }
    }

    let mut summary = String::new();

    if !key_decisions.is_empty() {
        summary.push_str("Key points from earlier conversation:\n");
        for (i, decision) in key_decisions.iter().take(10).enumerate() {
            summary.push_str(&format!("{}. {decision}\n", i + 1));
        }
        if key_decisions.len() > 10 {
            summary.push_str(&format!(
                "... and {} more exchanges\n",
                key_decisions.len() - 10
            ));
        }
    }

    if !tool_calls.is_empty() {
        // Deduplicate and count tool calls
        let mut tool_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::<&str, usize>::new();
        for tc in &tool_calls {
            *tool_counts.entry(tc.as_str()).or_default() += 1;
        }
        summary.push_str("\nTools used: ");
        let tool_summary: Vec<String> = tool_counts
            .iter()
            .map(|(name, count)| {
                if *count > 1 {
                    format!("{name} x{count}")
                } else {
                    name.to_string()
                }
            })
            .collect();
        summary.push_str(&tool_summary.join(", "));
        summary.push('\n');
    }

    if summary.is_empty() {
        summary = "Previous conversation context was compacted.".to_string();
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(n: usize) -> Session {
        let mut session = Session::new();
        for i in 0..n {
            session
                .messages
                .push(ConversationMessage::user(format!("message {i}")));
            session.messages.push(ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: format!("response to message {i}"),
                }],
                usage: None,
            });
        }
        session
    }

    #[test]
    fn micro_compact_trims_long_tool_results() {
        let mut session = Session::new();
        let long_output = "x".repeat(5000);
        session.messages.push(ConversationMessage {
            role: MessageRole::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_use_id: "t1".to_string(),
                tool_name: "read_file".to_string(),
                output: long_output,
                is_error: false,
            }],
            usage: None,
        });

        micro_compact(&mut session);

        if let ContentBlock::ToolResult { output, .. } = &session.messages[0].blocks[0] {
            assert!(output.len() < 2000);
            assert!(output.contains("trimmed by compaction"));
        } else {
            panic!("expected tool result");
        }
    }

    #[test]
    fn auto_compact_preserves_recent_messages() {
        let mut session = make_session(50); // 100 messages
        let config = CompactionConfig {
            max_messages: 80,
            keep_recent: 10,
            ..Default::default()
        };

        auto_compact(&mut session, &config);

        // Should have 1 summary + 10 recent = 11
        assert_eq!(session.messages.len(), 11);
        // First message should be the summary
        if let ContentBlock::Text { text } = &session.messages[0].blocks[0] {
            assert!(text.contains("compacted"));
        }
    }

    #[test]
    fn emergency_compact_drops_to_minimum() {
        let mut session = make_session(100); // 200 messages
        let config = CompactionConfig {
            keep_recent: 5,
            ..Default::default()
        };

        emergency_compact(&mut session, &config);

        assert_eq!(session.messages.len(), 6); // 1 notice + 5 recent
    }

    #[test]
    fn summary_captures_key_decisions() {
        let messages = vec![
            ConversationMessage::user("fix the auth bug in handler.rs"),
            ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: "I'll read the file first to understand the issue.".to_string(),
                }],
                usage: None,
            },
        ];

        let summary = build_conversation_summary(&messages);
        assert!(summary.contains("fix the auth bug"));
        assert!(summary.contains("read the file"));
    }
}
