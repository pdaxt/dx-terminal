pub mod dashboard;
pub mod widgets;

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;

const TICK_MS: u64 = 250;

pub fn run_tui(app: Arc<App>) -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &app);

    // Restore terminal — always runs even on error
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &App,
) -> anyhow::Result<()> {
    let mut selected: u8 = 1;
    let tick_rate = Duration::from_millis(TICK_MS);
    let mut last_tick = Instant::now();

    loop {
        // Collect all data (locks once, releases before render)
        let data = dashboard::collect_data(app, selected);

        // Render
        terminal.draw(|f| dashboard::render(f, &data))?;

        // Event handling with timeout until next tick
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Quit
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }

                    // Pane focus (1-9+ based on config)
                    KeyCode::Char(c @ '1'..='9') => {
                        let n = c.to_digit(10).unwrap() as u8;
                        if n <= crate::config::pane_count() {
                            selected = n;
                        }
                    }

                    // Tab cycles through panes
                    KeyCode::Tab => {
                        let max = crate::config::pane_count();
                        selected = if selected >= max { 1 } else { selected + 1 };
                    }
                    KeyCode::BackTab => {
                        let max = crate::config::pane_count();
                        selected = if selected <= 1 { max } else { selected - 1 };
                    }

                    // Kill selected pane's agent
                    KeyCode::Char('k') => {
                        let mut pty = app.pty_lock();
                        let _ = pty.kill(selected);
                    }

                    // Restart selected pane (kill + re-read state for respawn)
                    KeyCode::Char('r') => {
                        let mut pty = app.pty_lock();
                        let _ = pty.kill(selected);
                        // Note: actual respawn requires MCP os_restart call.
                        // TUI kill is for emergency stop.
                    }

                    // Send Ctrl-C to selected pane
                    KeyCode::Char('x') => {
                        let mut pty = app.pty_lock();
                        if let Some(agent) = pty.agents.get_mut(&selected) {
                            let _ = agent.send_ctrl_c();
                        }
                    }

                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}
