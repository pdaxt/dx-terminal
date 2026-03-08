use crate::app::AppState;
use chrono::Local;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct HeaderWidget;

impl HeaderWidget {
    pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
        let total = state.agents.root_agents.len();
        let processing = state.agents.processing_count();
        let pending = state.agents.active_count();
        let queue_pending = state
            .queue_tasks
            .iter()
            .filter(|t| t.status == "pending")
            .count();
        let queue_running = state
            .queue_tasks
            .iter()
            .filter(|t| t.status == "running")
            .count();
        let time = Local::now().format("%H:%M").to_string();

        // Session cost from usage tracker
        let session_cost = state.usage_tracker.session_cost();
        let today_cost = state.usage_tracker.today_cost();

        let mut spans = vec![
            Span::styled(
                " DX ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {} agents ", total),
                Style::default().fg(Color::White),
            ),
        ];

        // Session cost (always visible when > 0)
        if session_cost > 0.001 || today_cost > 0.001 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            let cost_color = if today_cost > 5.0 {
                Color::Red
            } else if today_cost > 1.0 {
                Color::Yellow
            } else {
                Color::Green
            };
            spans.push(Span::styled(
                format!(" ${:.2} session ${:.2} today ", session_cost, today_cost),
                Style::default().fg(cost_color),
            ));
        }

        // Processing count
        if processing > 0 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" {} {} working ", state.spinner_frame(), processing),
                Style::default().fg(Color::Yellow),
            ));
        }

        // Pending approvals
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        if pending > 0 {
            spans.push(Span::styled(
                format!(" {} pending ", pending),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(" ready ", Style::default().fg(Color::Green)));
        }

        // Queue info (if connected to hub)
        if state.hub_connected {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" Q:{}/{} ", queue_running, queue_pending + queue_running),
                Style::default().fg(Color::Magenta),
            ));
        }

        // 24h analytics digest (from API)
        if state.digest.tool_calls > 0 || state.digest.errors > 0 || state.digest.commits > 0 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(
                    " TC:{} E:{} C:{} ",
                    state.digest.tool_calls, state.digest.errors, state.digest.commits
                ),
                Style::default().fg(Color::Cyan),
            ));
        }

        // Alert badge
        if state.alerts.count > 0 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" ! {} ", state.alerts.count),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // ACU usage from dashboard
        let cap = &state.dashboard.capacity;
        if cap.acu_total > 0.0 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            let acu_pct = cap.acu_pct();
            let acu_color = if acu_pct > 80.0 {
                Color::Red
            } else if acu_pct > 50.0 {
                Color::Yellow
            } else {
                Color::Green
            };
            spans.push(Span::styled(
                format!(" ACU:{:.0}/{:.0} ({:.0}%) ", cap.acu_used, cap.acu_total, acu_pct),
                Style::default().fg(acu_color),
            ));
        }

        // MCP tool count
        let total_tools = state.dashboard.total_mcp_tools();
        if total_tools > 0 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!(" {}+ tools ", total_tools),
                Style::default().fg(Color::Magenta),
            ));
        }

        // System stats: CPU
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        let cpu_color = if state.system_stats.cpu_usage > 80.0 {
            Color::Red
        } else if state.system_stats.cpu_usage > 50.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        let sparkline = state.system_stats.cpu_sparkline();
        if !sparkline.is_empty() {
            spans.push(Span::styled(
                format!(" {}", sparkline),
                Style::default().fg(cpu_color),
            ));
        }
        spans.push(Span::styled(
            format!(" {:4.1}% ", state.system_stats.cpu_usage),
            Style::default().fg(cpu_color),
        ));

        // System stats: Memory
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        let mem_percent = state.system_stats.memory_percent();
        let mem_color = if mem_percent > 80.0 {
            Color::Red
        } else if mem_percent > 60.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        spans.push(Span::styled(
            format!(
                " MEM {} ({:.0}%) ",
                state.system_stats.memory_display(),
                mem_percent
            ),
            Style::default().fg(mem_color),
        ));

        // API connection status
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        if state.hub_connected {
            spans.push(Span::styled(" API ", Style::default().fg(Color::Green)));
        } else {
            spans.push(Span::styled(" API ", Style::default().fg(Color::DarkGray)));
        }

        // Time
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!(" {} ", time),
            Style::default().fg(Color::DarkGray),
        ));

        let line = Line::from(spans);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(line).block(block);
        frame.render_widget(paragraph, area);
    }
}
