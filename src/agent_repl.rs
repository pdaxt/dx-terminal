use std::time::Instant;

use anyhow::Result;
use dx_agent_harness::{RuntimeEvent, RuntimeListener};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::agent_display;
use crate::agent_prompt;
use crate::agent_setup;

/// CLI listener that renders rich terminal output.
struct CliListener {
    start: Instant,
    has_printed_text: bool,
}

impl CliListener {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            has_printed_text: false,
        }
    }
}

impl RuntimeListener for CliListener {
    fn on_event(&mut self, event: RuntimeEvent<'_>) {
        match event {
            RuntimeEvent::Thinking => {
                agent_display::print_spinner(0, self.start.elapsed().as_secs_f64());
            }
            RuntimeEvent::Text(text) => {
                if !self.has_printed_text {
                    agent_display::clear_spinner();
                    eprint!("  ");
                    self.has_printed_text = true;
                }
                // Print each token as it arrives — live streaming
                eprint!("{text}");
                std::io::Write::flush(&mut std::io::stderr()).ok();
            }
            RuntimeEvent::ToolCall { name, input } => {
                agent_display::clear_spinner();
                agent_display::print_tool_call(name, input);
            }
            RuntimeEvent::ToolResult { name, success } => {
                if !success {
                    eprintln!("\x1b[31m  ✗ {name} failed\x1b[0m");
                }
            }
            RuntimeEvent::Done => {
                agent_display::clear_spinner();
            }
        }
    }
}

pub fn run_repl(model: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Auto-detect or auto-setup model (zero config)
    let (client, model_name) = match dx_agent_api::ProviderClient::auto_detect(model.as_deref()) {
        Ok(result) => result,
        Err(_) => {
            let model_id = agent_setup::ensure_ready()?;
            dx_agent_api::ProviderClient::auto_detect(Some(&model_id))?
        }
    };

    // Build everything
    let policy =
        dx_agent_harness::PermissionPolicy::new(dx_agent_harness::PermissionMode::WorkspaceWrite)
            .with_tool("read_file", dx_agent_harness::PermissionMode::ReadOnly)
            .with_tool("glob", dx_agent_harness::PermissionMode::ReadOnly)
            .with_tool("grep", dx_agent_harness::PermissionMode::ReadOnly)
            .with_tool(
                "write_file",
                dx_agent_harness::PermissionMode::WorkspaceWrite,
            )
            .with_tool(
                "edit_file",
                dx_agent_harness::PermissionMode::WorkspaceWrite,
            )
            .with_tool("git", dx_agent_harness::PermissionMode::WorkspaceWrite)
            .with_tool("bash", dx_agent_harness::PermissionMode::FullAccess);

    let registry = dx_agent_tools::ToolRegistry::default_cli();
    let tools = registry.to_api_definitions();
    let system_prompt = agent_prompt::build_system_prompt(&cwd);

    let mut runtime = dx_agent_harness::ConversationRuntime::new(
        client,
        policy,
        system_prompt,
        tools,
        cwd.clone(),
    )
    .with_max_iterations(16);

    // Banner
    eprintln!();
    eprintln!("  \x1b[1;36mdxos\x1b[0m v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("  \x1b[2mmodel: {model_name}\x1b[0m");
    eprintln!("  \x1b[2mdir:   {}\x1b[0m", cwd.display());
    eprintln!("  \x1b[2m/help for commands, /quit to exit\x1b[0m");
    eprintln!();

    // Readline with history
    let mut rl = DefaultEditor::new()?;
    let history_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dxos")
        .join("history.txt");
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let _ = rl.load_history(&history_path);

    let mut turn_count: usize = 0;

    loop {
        let line = match rl.readline("\x1b[1;32m❯\x1b[0m ") {
            Ok(line) => line,
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                eprintln!("\x1b[2mbye.\x1b[0m");
                break;
            }
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        };

        let input = line.trim().to_string();
        if input.is_empty() {
            continue;
        }

        rl.add_history_entry(&input).ok();

        // Slash commands
        if input.starts_with('/') {
            match input.as_str() {
                "/quit" | "/exit" | "/q" => {
                    let _ = rl.save_history(&history_path);
                    eprintln!("\x1b[2mbye.\x1b[0m");
                    break;
                }
                "/help" | "/h" => {
                    eprintln!();
                    eprintln!("  \x1b[1mCommands:\x1b[0m");
                    eprintln!("    /help       show this help");
                    eprintln!("    /quit       exit the session");
                    eprintln!("    /clear      reset conversation");
                    eprintln!("    /model      show current model");
                    eprintln!("    /turns      show turn count");
                    eprintln!();
                    continue;
                }
                "/clear" => {
                    let new_registry = dx_agent_tools::ToolRegistry::default_cli();
                    let (new_client, _) =
                        dx_agent_api::ProviderClient::auto_detect(Some(&model_name))?;
                    runtime = dx_agent_harness::ConversationRuntime::new(
                        new_client,
                        dx_agent_harness::PermissionPolicy::new(
                            dx_agent_harness::PermissionMode::WorkspaceWrite,
                        )
                        .with_tool("read_file", dx_agent_harness::PermissionMode::ReadOnly)
                        .with_tool("glob", dx_agent_harness::PermissionMode::ReadOnly)
                        .with_tool("grep", dx_agent_harness::PermissionMode::ReadOnly)
                        .with_tool(
                            "write_file",
                            dx_agent_harness::PermissionMode::WorkspaceWrite,
                        )
                        .with_tool(
                            "edit_file",
                            dx_agent_harness::PermissionMode::WorkspaceWrite,
                        )
                        .with_tool("git", dx_agent_harness::PermissionMode::WorkspaceWrite)
                        .with_tool("bash", dx_agent_harness::PermissionMode::FullAccess),
                        agent_prompt::build_system_prompt(&cwd),
                        new_registry.to_api_definitions(),
                        cwd.clone(),
                    )
                    .with_max_iterations(16);
                    turn_count = 0;
                    eprintln!("\x1b[2mconversation cleared.\x1b[0m");
                    continue;
                }
                "/model" => {
                    eprintln!("  model: {model_name}");
                    continue;
                }
                "/turns" => {
                    eprintln!("  turns: {turn_count}");
                    continue;
                }
                _ => {
                    eprintln!("\x1b[33munknown command: {input}\x1b[0m (try /help)");
                    continue;
                }
            }
        }

        // Run turn with rich display
        turn_count += 1;
        let start = Instant::now();
        let mut listener = CliListener::new();

        match runtime.run_turn_with_listener(&input, &mut listener) {
            Ok(summary) => {
                let elapsed = start.elapsed().as_secs_f64();

                if !summary.was_streamed && !summary.text.is_empty() {
                    // Non-streaming provider — print the response now
                    eprintln!();
                    let formatted = agent_display::format_output(&summary.text);
                    for line in formatted.lines() {
                        println!("  {line}");
                    }
                }

                agent_display::print_summary(
                    summary.tool_calls,
                    summary.iterations,
                    summary.usage.total_tokens(),
                    elapsed,
                );
                eprintln!();
            }
            Err(e) => {
                agent_display::clear_spinner();
                eprintln!("\x1b[31merror: {e}\x1b[0m\n");
            }
        }
    }

    Ok(())
}
