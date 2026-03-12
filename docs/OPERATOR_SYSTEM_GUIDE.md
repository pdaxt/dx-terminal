# DX Terminal Operator System Guide

## Purpose

DX Terminal is intended to be the operating system for software delivery across humans and agent runtimes. The target state is not "another dashboard". The target state is one control plane that keeps these things aligned:

- project intent
- discovery and architecture decisions
- active runtime work
- test and acceptance evidence
- git/worktree state
- operator visibility in the local dashboard and any hosted dashboard

If those do not converge on one shared model, the product becomes a monitor instead of a delivery system.

## End-to-End Stage Model

The delivery model is:

1. `planned`
   Work is known but not yet structured enough to execute.
2. `discovery`
   Research, open questions, architecture notes, and acceptance criteria are produced.
3. `build`
   A pane or runtime is actively implementing the feature in a tracked project/worktree.
4. `test`
   Tests, QA evidence, and acceptance verification are being collected.
5. `done`
   Implementation, verification, and documentation are aligned.

The important rule is that phase changes should be evidence-driven, not operator folklore.

## What The System Should Replace

DX Terminal does not need to copy Confluence, Jira, QA spreadsheets, and terminal notes literally. It needs to absorb the functions they serve:

- Confluence: mission, architecture, decisions, discovery, operator handbooks
- Jira: features, blockers, readiness, active ownership, progress by stage
- engineer terminal state: who is doing what, where, on which branch/worktree
- QA state: what was verified, what is still blocked, what evidence exists
- ops dashboard: live runtime health, logs, recent events, browser automation access

## Canonical State Contract

The system should have one canonical contract for both local and hosted experiences.

### Snapshot

The canonical project snapshot is:

`GET /api/project/brief?project=<name>`

It should answer:

- what the mission is
- which feature is focused
- which features are blocked or ready
- which runtimes are active
- which worktrees/branches are live
- whether docs, git, and dashboard state are synchronized
- what automation assets and MCPs are available

### Live Events

The live event stream should carry:

- pane output changes
- session/tool events
- focus changes
- vision/phase/readiness changes
- sync/documentation changes

Hosted dashboards should consume the same snapshot and the same event model as localhost. They should not maintain a second private state model.

## Architecture

### Control Plane

```text
                    +----------------------+
                    |    Human Operator    |
                    +----------+-----------+
                               |
                               v
 +-------------+      +--------+---------+      +------------------+
 | tmux panes   |<---->|  DX Terminal     |<---->|  External MCPs   |
 | Claude/Codex |      |  app + gateway   |      | Playwright, etc. |
 | Gemini/etc.  |      +---+----------+---+      +------------------+
 +------+------+          |          |
        |                 |          |
        |                 |          +----------------------+
        |                 |                                 |
        v                 v                                 v
 +-------------+   +-------------+                  +---------------+
 | filesystem   |   | state/events |                  | git/worktrees |
 | AGENTS/VDD   |   | ws/sse/ipc   |                  | branch status  |
 +------+------+   +------+------+                  +-------+-------+
        |                 |                                 |
        +-----------------+---------------+-----------------+
                                          |
                                          v
                                 +------------------+
                                 | dashboards/wiki  |
                                 | local or hosted  |
                                 +------------------+
```

### Documentation Sync

```text
         Project Filesystem
   +-----------------------------+
   | AGENTS.md                   |
   | CLAUDE.md / CODEX.md / ...  |
   | .vision/research/*.md       |
   | .vision/discovery/*.md      |
   +-------------+---------------+
                 |
                 v
        +--------+---------+
        |  git inventory   |
        | tracked / dirty  |
        +--------+---------+
                 |
                 v
        +--------+---------+
        | /api/project/brief|
        | docs + health     |
        +--------+---------+
                 |
                 v
        +--------+---------+
        | websocket events |
        | vision_changed   |
        +--------+---------+
                 |
                 v
      Local dashboard and hosted site
```

Rule: the hosted site is only valid if it uses the same filesystem-derived snapshot and event model.

### Runtime and Browser Testing Contract

Each managed pane should have a stable browser automation contract:

- pane `N` owns browser port `46000 + N`
- pane `N` owns browser profile root `~/.playwright-profiles/pane-N`
- pane `N` owns browser artifacts root `~/Projects/test-artifacts/sessions/pane-N`

That contract matters because browser automation becomes operationally simple:

- humans know where to inspect artifacts
- agents know which port to use without negotiating
- restarting a pane does not require reassigning a new port
- the dashboard can show browser ownership as runtime metadata

## Operator Workflow

### 1. Start from the project brief

Use the execution map to answer:

- what is the mission
- what stage each feature is in
- what is blocked
- which runtime is currently doing work
- whether docs and git are in sync

### 2. Focus a feature

The active focus tells auto-continue, the panel context, and the VDD viewer what the current work target is. Focus should be explicit and shared across:

- dashboard interactions
- MCP/VDD mutations
- hook-driven continuation

### 3. Work in isolated runtimes

Each runtime should expose:

- provider
- pane number
- tmux target
- project/workspace path
- branch/worktree
- browser automation port

If that metadata is missing, coordination degrades quickly.

### 4. Keep discovery and verification visible

Before moving fast on implementation, operators should be able to inspect:

- research and discovery docs
- open questions and blockers
- acceptance criteria
- test evidence and verification status

### 5. Review documentation health continuously

Docs should not be a trailing artifact. The dashboard should keep showing:

- missing discovery coverage
- missing acceptance coverage
- dirty/uncommitted docs
- whether the hosted mirror would be trustworthy right now

## Guidance For Humans

When working in DX Terminal, humans should assume:

- the dashboard is the read model
- markdown files are the authored source of truth
- git is the integrity layer
- websocket events are the freshness layer
- VDD is the delivery state machine

Recommended human loop:

1. pick or confirm active focus
2. inspect discovery/docs before implementation
3. execute in a pane/worktree
4. use the pane-scoped browser contract for UI testing
5. verify acceptance before calling work done
6. commit documentation changes with the code they explain

## Guidance For Developers

When extending DX Terminal, do not add isolated feature state to the UI if the backend contract does not already expose it. Prefer:

1. add it to the backend snapshot/event model
2. use the same field in local and hosted dashboards
3. make docs explain the operator meaning of the field

The system gets stronger when new capabilities are:

- provider-neutral
- pane-aware
- stage-aware
- documentation-aware
- replayable from filesystem + events

## Current Target Outcome

The finished product should let a small team run multi-provider delivery from one place:

- plan and discovery are visible
- runtime execution is visible
- browser testing is deterministic per pane
- docs, git, and dashboard do not drift silently
- local and hosted dashboards consume the same contract

That is the difference between a terminal monitor and a real delivery operating system.
