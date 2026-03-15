use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::app::App;
use crate::state::events::next_seq;

type AppState = Arc<App>;

/// GET /api/events — SSE stream of state changes
pub async fn event_stream(
    State(app): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = app.state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(event) => {
                let seq = next_seq();
                let data = serde_json::json!({
                    "seq": seq,
                    "event": serde_json::to_value(&event).unwrap_or_default(),
                })
                .to_string();
                Some(Ok::<_, Infallible>(Event::default().data(data)))
            }
            Err(_) => None, // Lagged — skip
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
