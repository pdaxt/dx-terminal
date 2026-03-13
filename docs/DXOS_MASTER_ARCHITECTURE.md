# DXOS Master Architecture

## Purpose

DX Terminal is no longer just a terminal multiplexer with vision tracking. The target product is `DXOS`: a multi-tenant AI operating system with a portal, a runtime shell, and a control plane.

This document is the top-level architecture statement for that direction.

## Product Shape

DXOS has two primary user surfaces:

- `DX Portal`
  - client onboarding
  - discovery and design approvals
  - delivery, QA, security, and release visibility
  - company admin and provider setup
- `DX Runtime Shell`
  - high-speed operator and agent execution shell
  - custom PTY substrate
  - scoped sessions, browser ownership, and live supervision

Behind both sits one shared control plane.

```mermaid
flowchart TB
    Portal[DX Portal] --> Control[DXOS Control Plane]
    Shell[DX Runtime Shell] --> Control

    Control --> VDD[VDD Kernel]
    Control --> Sessions[Session Manager]
    Control --> Registry[Capability Registry]
    Control --> Router[Model Router]
    Control --> Docs[Documentation Engine]
    Control --> Compliance[Compliance Engine]
    Control --> Reports[Observation and Reports]
    Control --> Debate[Debate and Decision Engine]
```

## Core Platform Model

The control plane owns:

- `Tenant`
- `Company`
- `Program`
- `Workspace`
- `Project`
- `Repo`
- `Environment`
- `Feature`
- `Stage`
- `Session`
- `AgentRole`
- `Capability`
- `Workflow`
- `Artifact`
- `Approval`
- `ComplianceProfile`
- `Report`
- `Incident`
- `Debate`
- `Proposal`
- `Contradiction`
- `Vote`
- `DecisionRecord`

The delivery stages are:

- `planned`
- `discovery`
- `design`
- `build`
- `test`
- `done`

## Runtime Model

The primitive is `session`, not pane.

Each session carries:

- tenant/company/project/workspace
- role
- provider/model
- autonomy level
- allowed repos and directories
- branch/worktree binding
- allowed capabilities
- browser profile and port ownership
- expected outputs
- escalation path

DXOS now exposes a brokered runtime model with:

- `pty_native_adapter` as the default substrate
- `tmux_migration_adapter` as the compatibility path
- provider inventory that declares preferred and supported adapters per runtime
- server-side auto-allocation of the next free lane when operators launch a session without binding it to a specific pane up front
- HTTP control-plane launch endpoints, so hosted portals and local dashboards can create lanes without depending on a WebSocket-only spawn path
- HTTP pane-control endpoints for `talk` and `kill`, so hosted portals can supervise live lanes through the control plane while WebSocket remains focused on live streaming and event delivery

tmux remains a migration adapter only. The target substrate is DX-owned PTY sessions, and the operator surfaces now render adapter choice and live substrate state explicitly.

## Governance and Reasoning

DXOS uses a structured council model for important reasoning.

The built-in debate workflow is:

1. start debate
2. submit proposals
3. submit contradictions
4. cast votes
5. synthesize and finalize decision

Every decision should preserve:

- rationale
- evidence refs
- dissent or contradiction
- final synthesizer
- linked feature or stage

This is how the system supports invention-grade work without reducing reasoning to one model’s answer.

## Documentation Contract

Documentation is a hard dependency, not a later summary.

Key document classes include:

- Company Handbook
- Program Charter
- Project Brief
- Discovery Brief
- Design Review
- Architecture Spec
- Decision Log
- Research Brief
- Debate Record
- Test Plan
- Verification Report
- Security Review
- Compliance Pack
- Release Packet
- Incident Report

Documentation is compiled from:

- workflow events
- Git and repo state
- artifacts
- approvals
- human edits
- debate records

Stage transitions should fail when required documentation is missing or stale.

## Compliance and Security

DXOS must treat compliance as native system behavior.

Baseline requirements:

- SOC 2-aligned control evidence
- jurisdiction-specific policy profiles
- data residency restrictions
- provider and capability restrictions by company or project
- immutable audit trails
- approval history
- human handoff for MFA, login challenge, and sensitive actions

## Implementation Direction

The current repo already contains useful seeds:

- VDD lifecycle and docs
- provider runtime monitoring
- MCP bridging
- dashboard and wiki contracts
- live event system

The next major direction is to consolidate those into:

1. a database-backed control plane
2. a DX-owned session runtime
3. a provider-neutral capability registry
4. a formal documentation and decision engine
5. a multi-surface product where portal and shell are equal clients of the same truth

## Current First Slice

The first architecture slice now implemented in the repo is:

- project-scoped DXOS control-plane state
- formal debate engine
- native session contracts and delegated work orders
- MCP and web APIs for proposal, contradiction, vote, and decision flows
- MCP and web APIs for session upsert, status updates, delegation, blocking, and resolution
- live `debate_changed` and `dxos_session_changed` events
- portal execution hub surfaces DXOS session contracts, delegated work, blocker queues, and recent decision records
- runtime cards now carry `dxos_session_id`, so pane state and control-plane session state line up from first render
- blocked work orders can be resumed directly from the portal, and mapped sessions can jump straight into their pane
- portal-native operator controls can now launch a real runtime lane on the current adapter, register provider-neutral session contracts, delegate structured work, and start formal debates without dropping into raw terminal commands
- DXOS debate/session events now update the execution hub surgically instead of forcing a full page refresh, so the portal can evolve into a stable control surface rather than a passive monitor
- the tmux migration adapter now launches provider-specific lanes for Claude, Codex, and Gemini, with provider/model persisted in pane state and reflected back into the portal and DXOS session contract
- runtime launch is no longer “Claude plus labels”; provider choice now flows from the portal form, through the websocket spawn command, into the runtime broker, and back into the control plane
- DXOS now publishes a role-and-stage provider policy matrix, so the portal can explain which runtimes are preferred or allowed before a lane is launched
- session contracts now persist `policy_violations` and `last_error`, which gives the portal a durable way to surface blocked provider choices and failed runtime launches instead of silently dropping them
- runtime lanes are registered in DXOS before the adapter launches, so failed launches still leave behind a supervised session record with the intended feature, stage, and supervisor context
- provider-native launch planning now lives in a DX runtime broker instead of inside the tmux adapter, so provider binary discovery, command construction, and launch policy are separated from window creation
- tmux is now explicitly a migration adapter that executes brokered launch plans; the DXOS runtime contract and project brief both advertise the broker and supported providers directly
- worker sessions can now raise blockers or permission requests through DXOS session context, and the control plane routes those requests to the supervising lead first before falling back to explicit human escalation
- runtime lanes now receive `DXOS_SESSION_ID`, `DX_FEATURE_ID`, `DX_STAGE`, and `DX_SUPERVISOR_SESSION_ID` so agents in a live lane can report blocker and approval state without reconstructing their control-plane identity
- the server-owned runtime replicator now detects clear approval/login/challenge prompts in live pane output and converts them into DXOS blocker events once, so “waiting for human action” is lifted out of raw terminal text and into the control plane
- resolving a blocked work order now persists an explicit resolution note and pushes that guidance back into the worker lane automatically through the live runtime target when one exists
- if DXOS can clear the blocker but cannot reach the worker lane, the failure is recorded back onto the session contract so the portal still shows the unresolved operational gap
- the stop hook now continues work while the next high-value task remains clear, which keeps auto-continue aligned with DXOS language instead of relying on looser “obvious next step” wording

That gives the platform a native place to reason, disagree, decide, supervise, and delegate inside the system itself.
