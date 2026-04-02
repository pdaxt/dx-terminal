use std::path::PathBuf;

use dx_agent_core::{ContentBlock, ConversationMessage, DxosError, Result, Session, TokenUsage};

use crate::compact::{compact_session, should_compact, CompactionConfig};
use crate::permissions::{PermissionOutcome, PermissionPolicy, PermissionPrompter};

#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub system_prompt: Vec<String>,
    pub messages: Vec<ConversationMessage>,
    pub tools: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantEvent {
    TextDelta(String),
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    Usage(TokenUsage),
    Stop,
}

pub trait ApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>>;

    /// Stream with a callback for each token as it arrives.
    /// Default implementation delegates to `stream()` (no live output).
    fn stream_with_callback(
        &mut self,
        request: ApiRequest,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<Vec<AssistantEvent>> {
        let events = self.stream(request)?;
        for event in &events {
            if let AssistantEvent::TextDelta(text) = event {
                on_token(text);
            }
        }
        Ok(events)
    }
}

/// Runtime events emitted during execution for UI display.
pub enum RuntimeEvent<'a> {
    /// LLM is being called (show spinner)
    Thinking,
    /// Text output from the model
    Text(&'a str),
    /// Tool is about to be called
    ToolCall { name: &'a str, input: &'a str },
    /// Tool finished
    ToolResult { name: &'a str, success: bool },
    /// Turn complete
    Done,
}

/// Callback for runtime events — the CLI implements this to render the UI.
pub trait RuntimeListener {
    fn on_event(&mut self, event: RuntimeEvent<'_>);
}

/// No-op listener for when no UI is needed.
pub struct SilentListener;
impl RuntimeListener for SilentListener {
    fn on_event(&mut self, _event: RuntimeEvent<'_>) {}
}

#[derive(Debug, Clone)]
pub struct TurnSummary {
    pub text: String,
    pub tool_calls: usize,
    pub iterations: usize,
    pub usage: TokenUsage,
    /// True if text was already streamed to stderr during generation.
    pub was_streamed: bool,
}

pub struct ConversationRuntime<C: ApiClient> {
    session: Session,
    api_client: C,
    permission_policy: PermissionPolicy,
    system_prompt: Vec<String>,
    tools: Vec<serde_json::Value>,
    cwd: PathBuf,
    max_iterations: usize,
    cumulative_usage: TokenUsage,
    compaction_config: CompactionConfig,
}

impl<C: ApiClient> ConversationRuntime<C> {
    pub fn new(
        api_client: C,
        permission_policy: PermissionPolicy,
        system_prompt: Vec<String>,
        tools: Vec<serde_json::Value>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            session: Session::new(),
            api_client,
            permission_policy,
            system_prompt,
            tools,
            cwd,
            max_iterations: 16,
            cumulative_usage: TokenUsage::default(),
            compaction_config: CompactionConfig::default(),
        }
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Run a turn without UI events (backwards compatible).
    pub fn run_turn(
        &mut self,
        user_input: impl Into<String>,
        _prompter: Option<&mut dyn PermissionPrompter>,
    ) -> Result<TurnSummary> {
        self.run_turn_with_listener(user_input, &mut SilentListener)
    }

    /// Run a turn with a listener for UI events.
    pub fn run_turn_with_listener(
        &mut self,
        user_input: impl Into<String>,
        listener: &mut dyn RuntimeListener,
    ) -> Result<TurnSummary> {
        self.session
            .messages
            .push(ConversationMessage::user(user_input));

        // Compact if needed
        if should_compact(&self.session, &self.compaction_config) {
            compact_session(&mut self.session, &self.compaction_config);
        }

        let mut text_output = String::new();
        let mut tool_calls = 0;
        let mut iterations = 0;
        let mut any_streamed = false;

        loop {
            iterations += 1;
            if iterations > self.max_iterations {
                return Err(DxosError::TurnLimitExceeded { iterations });
            }

            // Start animated spinner in background thread
            let spinner_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let spinner_flag = spinner_running.clone();
            let spinner_thread = std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let mut frame = 0usize;
                while spinner_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    let elapsed = start.elapsed().as_secs_f64();
                    // Inline spinner rendering to avoid cross-crate dep
                    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let verbs = [
                        "Thinking",
                        "Analyzing",
                        "Reading",
                        "Processing",
                        "Examining",
                        "Evaluating",
                        "Searching",
                        "Scanning",
                        "Investigating",
                        "Exploring",
                        "Reasoning",
                        "Computing",
                        "Architecting",
                        "Synthesizing",
                        "Parsing",
                    ];
                    let s = spinner_chars[frame % spinner_chars.len()];
                    let v = verbs[(elapsed as usize / 3) % verbs.len()];
                    let dot_phase = ((elapsed * 3.0) as usize) % 6;
                    let dots = match dot_phase {
                        0 => "   ",
                        1 => ".  ",
                        2 => ".. ",
                        3 => "...",
                        4 => ".. ",
                        5 => ".  ",
                        _ => "...",
                    };
                    let color = if elapsed < 5.0 {
                        "\x1b[36m"
                    } else if elapsed < 15.0 {
                        "\x1b[33m"
                    } else {
                        "\x1b[31m"
                    };
                    eprint!("\r{color}{s}\x1b[0m \x1b[2m{v}{dots}\x1b[0m   ");
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                    frame += 1;
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            });

            let request = ApiRequest {
                system_prompt: self.system_prompt.clone(),
                messages: self.session.messages.clone(),
                tools: self.tools.clone(),
            };

            // Stream with live token display
            let spinner_flag2 = spinner_running.clone();
            let mut streamed = false;
            let mut streamed_len = 0usize;
            let events = self.api_client.stream_with_callback(request, &mut |text| {
                any_streamed = true;
                streamed_len += text.len();
                if !streamed {
                    // Stop spinner, clear line
                    spinner_flag2.store(false, std::sync::atomic::Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    eprint!("\r\x1b[K  ");
                    streamed = true;
                }
                eprint!("{text}");
                use std::io::Write;
                std::io::stderr().flush().ok();
            });

            // Stop spinner
            spinner_running.store(false, std::sync::atomic::Ordering::Relaxed);
            let _ = spinner_thread.join();
            if streamed {
                eprintln!();
            } else {
                eprint!("\r\x1b[K");
            }

            let events = events?;

            // Build assistant message from events
            let mut blocks = Vec::new();
            let mut turn_usage = None;
            let mut pending_tools = Vec::new();

            for event in events {
                match event {
                    AssistantEvent::TextDelta(text) => {
                        listener.on_event(RuntimeEvent::Text(&text));
                        text_output.push_str(&text);
                        blocks.push(ContentBlock::Text { text });
                    }
                    AssistantEvent::ToolUse { id, name, input } => {
                        tool_calls += 1;
                        listener.on_event(RuntimeEvent::ToolCall {
                            name: &name,
                            input: &input,
                        });
                        pending_tools.push((id.clone(), name.clone(), input.clone()));
                        blocks.push(ContentBlock::ToolUse { id, name, input });
                    }
                    AssistantEvent::Usage(usage) => {
                        self.cumulative_usage.accumulate(&usage);
                        turn_usage = Some(usage);
                    }
                    AssistantEvent::Stop => {}
                }
            }

            let mut assistant_msg = ConversationMessage::assistant(blocks);
            assistant_msg.usage = turn_usage;
            self.session.messages.push(assistant_msg);

            // If no tool calls, we're done
            if pending_tools.is_empty() {
                break;
            }

            // Execute tools
            for (tool_id, tool_name, input) in pending_tools {
                let outcome = self.permission_policy.authorize(&tool_name, &input, None);

                let result_msg = match outcome {
                    PermissionOutcome::Allow => {
                        match dx_agent_tools::execute_tool(&tool_name, &input, &self.cwd) {
                            Ok(output) => {
                                listener.on_event(RuntimeEvent::ToolResult {
                                    name: &tool_name,
                                    success: true,
                                });
                                ConversationMessage::tool_result(tool_id, tool_name, output, false)
                            }
                            Err(e) => {
                                listener.on_event(RuntimeEvent::ToolResult {
                                    name: &tool_name,
                                    success: false,
                                });
                                ConversationMessage::tool_result(
                                    tool_id,
                                    tool_name,
                                    e.to_string(),
                                    true,
                                )
                            }
                        }
                    }
                    PermissionOutcome::Deny { reason } => {
                        listener.on_event(RuntimeEvent::ToolResult {
                            name: &tool_name,
                            success: false,
                        });
                        ConversationMessage::tool_result(tool_id, tool_name, reason, true)
                    }
                };
                self.session.messages.push(result_msg);
            }
        }

        listener.on_event(RuntimeEvent::Done);

        Ok(TurnSummary {
            text: text_output,
            tool_calls,
            iterations,
            usage: self.cumulative_usage,
            was_streamed: any_streamed,
        })
    }
}
