use std::convert::Infallible;
use std::sync::Arc;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::app::App;
use crate::state::events::StateEvent;

type AppState = Arc<App>;

/// GET /api/events — SSE stream of state changes
pub async fn event_stream(
    State(app): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = app.state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(event) => {
                    let data = match &event {
                        StateEvent::PaneSpawned { pane, project, role } => {
                            serde_json::json!({
                                "type": "pane_spawned",
                                "pane": pane,
                                "project": project,
                                "role": role,
                            }).to_string()
                        }
                        StateEvent::PaneKilled { pane, reason } => {
                            serde_json::json!({
                                "type": "pane_killed",
                                "pane": pane,
                                "reason": reason,
                            }).to_string()
                        }
                        StateEvent::PaneStatusChanged { pane, status } => {
                            serde_json::json!({
                                "type": "pane_status_changed",
                                "pane": pane,
                                "status": status,
                            }).to_string()
                        }
                        StateEvent::LogAppended { pane, event, summary } => {
                            serde_json::json!({
                                "type": "log",
                                "pane": pane,
                                "event": event,
                                "summary": summary,
                            }).to_string()
                        }
                        StateEvent::QueueChanged { action, task_id, task } => {
                            serde_json::json!({
                                "type": "queue_changed",
                                "action": action,
                                "task_id": task_id,
                                "task": task,
                            }).to_string()
                        }
                        StateEvent::StateRefreshed => {
                            r#"{"type":"refresh"}"#.to_string()
                        }
                    };
                    Some(Ok::<_, Infallible>(Event::default().data(data)))
                }
                Err(_) => None, // Lagged — skip
            }
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
