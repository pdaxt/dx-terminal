# Dashboard Sync Review

This sync plane is not `MCP`. It is custom JSON over WebSocket, backed by local broadcast events, tmux scraping, REST refreshes, and Claude JSONL tailing.

## Main weaknesses

1. No single source of truth

The browser combines `wsState`, `statusData`, `monitorData`, `paneOutputs`, and `paneEvents`, and many socket messages just trigger `refresh()` instead of applying authoritative deltas.

Why this is weak:
- The socket behaves like an invalidation bus, not a real replication protocol.
- Different UI panels can temporarily disagree.
- Correctness depends on periodic REST refresh.

Relevant code:
- `assets/dashboard.html`
- `src/web/ws.rs`

2. Sync work is duplicated per client

Every WebSocket connection spawns its own tmux poller, sync forwarder, and initial snapshot builder.

Why this is weak:
- Cost scales with `clients x panes`.
- More tmux scraping than necessary.
- More chances for divergent client views under load.

Relevant code:
- `src/web/ws.rs`

3. Discovered pane identity is unstable

Auto-discovered panes are assigned display numbers dynamically each poll. The browser caches output and session events by pane number.

Why this is weak:
- If pane ordering changes, cached output can attach to the wrong pane.
- Session activity can appear under the wrong card.
- Pane number is being used as both display slot and entity identity.

Relevant code:
- `src/web/ws.rs`
- `assets/dashboard.html`

4. Event model is incomplete

`PaneSpawned` and `PaneKilled` exist in the event type but are not emitted by the main mutation paths. `set_pane()` changes state without broadcasting a full pane update.

Why this is weak:
- Live sync is partial.
- The UI relies on fallback refreshes.
- Some state changes are observable only on the next poll or full reload.

Relevant code:
- `src/state/events.rs`
- `src/state/mod.rs`
- `src/mcp/tools/panes.rs`

5. Session event streaming is lossy

The WebSocket loop re-reads recent JSONL tail events and replaces the client cache. A cursor-based `SessionTailer` exists but is not used in the live socket path.

Why this is weak:
- Bursts can be dropped.
- Recent events can be duplicated.
- Ordering is approximate, not authoritative.

Relevant code:
- `src/web/ws.rs`
- `src/session_stream.rs`

6. Terminal sync is approximate

The server compares captured tmux text with the previous capture and appends a suffix when possible, otherwise it sends a short tail.

Why this is weak:
- Fine for append-only logs.
- Wrong for rewritten terminal screens, spinners, partial redraws, and curses-like behavior.
- Not a true terminal-state protocol.

Relevant code:
- `src/web/ws.rs`

7. Reconciler failure semantics are too optimistic

If a tmux pane disappears, the reconciler marks the pane as `done`.

Why this is weak:
- Lost agents can be reported as successful completion.
- Operational truth is distorted at exactly the point where reliability matters.

Relevant code:
- `src/engine/reconcile.rs`

8. No replay or gap recovery

Lagged broadcast messages are skipped. There is no sequence number, no replay window, and no required full resync after a gap.

Why this is weak:
- Clients can silently miss updates.
- State convergence is accidental instead of guaranteed.

Relevant code:
- `src/web/ws.rs`
- `src/web/sse.rs`

## Best architecture

The best move is not a more exotic protocol. The best move is a stricter and more boring protocol.

### 1. Introduce one authoritative runtime replicator

Create one server-side task that:
- discovers tmux panes once
- tails Claude JSONL once
- reconciles pane state once
- builds the canonical live snapshot once
- broadcasts normalized updates to all clients

This removes per-client scraping and makes the server, not each browser session, responsible for truth.

### 2. Use stable entity identity

Primary identity should be:
- `session_id` if available
- otherwise `tmux_target`

Pane number should be a display slot only, not the durable key.

### 3. Replace invalidation-style messages with sequenced deltas

Use:
- full snapshot on connect
- monotonic `seq`
- typed delta messages

Suggested delta types:
- `PaneUpsert`
- `PaneRemoved`
- `OutputChunk`
- `SessionEventChunk`
- `QueueUpsert`
- `QueueRemoved`
- `VisionChanged`
- `SyncStatusChanged`

If the client detects a sequence gap, it should request a full resync.

### 4. Emit events from mutation boundaries

Do not emit from web handlers only. Emit from the core mutation paths:
- pane state writes
- queue changes
- reconciler decisions
- sync manager updates
- vision updates

That means `set_pane()` should produce an authoritative pane update event, not just persist state.

### 5. Use incremental JSONL tailing for live session streams

Adopt `SessionTailer` in the live WebSocket path and persist per-session offsets in the runtime replicator.

This gives:
- ordered event delivery
- no repeated tail scans
- lower IO
- fewer duplicates and drops

### 6. Separate terminal transport from semantic activity

Keep two distinct channels in the model:
- terminal screen/output stream
- semantic agent event stream

Do not infer agent semantics from terminal text if JSONL already gives structured activity.

### 7. Improve failure states

Use explicit states such as:
- `active`
- `completed`
- `error`
- `lost`
- `stale`

A disappeared tmux pane should default to `lost` unless there is positive evidence of clean completion.

### 8. Keep REST for inspection, not correctness

REST endpoints should support:
- manual refresh
- debugging
- historical views
- first-page bootstrap if needed

But correctness of live UI should come from the replicated runtime stream, not from a 30-second polling fallback.

## Practical target design

If you want the strongest version without overbuilding:

1. Build a `RuntimeReplicator` singleton in the server.
2. Give every pane and agent a stable ID.
3. Publish snapshot plus sequenced deltas over one WebSocket stream.
4. Move JSONL streaming to cursor-based incremental tailing.
5. Turn `set_pane()` and queue mutations into authoritative event producers.
6. Treat tmux as a source adapter, not as the protocol itself.

## Bottom line

The current system is pragmatic and useful, but it is not a rigorous sync protocol. It is a collection of good local mechanisms glued together:
- tmux scraping
- local pub/sub
- WebSocket push
- JSONL tailing
- REST refresh

That is good enough for a single-user operator console. It is not the best design if you want correctness, scalability, and confidence under load.

The best thing to do is:

Make the protocol explicit, authoritative, sequenced, and server-owned.

## Coverage gap

I did not find direct tests covering:
- WebSocket replication behavior
- tmux discovery stability
- JSONL incremental live streaming
- sequence gap recovery

So the current weaknesses are mostly architectural and largely unguarded by tests.
