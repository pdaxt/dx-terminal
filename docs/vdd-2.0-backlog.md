# VDD 2.0 Implementation Backlog

## Goal

Turn the VDD 2.0 design into a deliverable system for:

`discovery -> build -> test -> done`

This backlog is ordered for execution. Each phase has:

- objective
- scope
- concrete backlog items
- likely file touchpoints
- exit criteria

The sequence assumes we want an incremental rollout with compatibility for the current VDD store until the new path is stable.

## Delivery order

1. Stabilize the current event and VDD baseline.
2. Add the new VDD 2.0 schema and storage.
3. Make discovery first-class.
4. Make build execution first-class.
5. Make test and acceptance evidence first-class.
6. Make phase advancement evidence-driven.
7. Make hooks and dashboard consume the new model.
8. Migrate off the old status-driven path.

## Phase 0: Baseline Stabilization

### Objective

Start from a buildable baseline and remove obvious inconsistencies that will block VDD 2.0 work.

### Backlog

- Fix current compile failures caused by `StateEvent` expansion not being fully handled in websocket and SSE paths.
- Add a narrow VDD regression test suite around current feature status transitions, task transitions, and discovery doc reads.
- Remove or mark dead code paths that create ambiguity around the active MCP server path.
- Document the current `.vision` storage contract before changing it.

### Likely file touchpoints

- `src/web/ws.rs`
- `src/web/sse.rs`
- `src/state/events.rs`
- `src/vision.rs`
- `src/mcp/mod.rs`
- `tests/`

### Exit criteria

- `cargo test` is green or the remaining failures are unrelated and explicitly documented.
- Event consumers compile against the current runtime event model.
- There is a small test harness covering the current VDD surface so refactors have a safety net.

## Phase 1: Canonical VDD 2.0 Schema

### Objective

Introduce the new phase-aware model without breaking the old store immediately.

### Backlog

- Add explicit `phase` and `state` fields for features.
- Add new entities:
  - `DiscoveryRecord`
  - `AcceptanceCriterion`
  - `BuildPlan`
  - `BuildTask`
  - `VerificationPlan`
  - `VerificationRun`
  - `GateEvidence`
  - `Signoff`
- Add typed readiness checks for `ready_for_build`, `ready_for_test`, and `ready_for_done`.
- Add history entries for all mutating operations.
- Add sharded storage under `.vision/` while keeping read compatibility with `vision.json`.
- Build a derived `read-model.json` generator.

### Likely file touchpoints

- `src/vision.rs`
- new `src/vision_store.rs` or similar storage module
- `src/web/api.rs`
- `src/mcp/types.rs`
- `tests/`

### Exit criteria

- A feature can be loaded with explicit `phase` and `state`.
- New entities can be created and persisted in the sharded store.
- A read model can be rebuilt from canonical entity files.
- Existing projects can still be read.

## Phase 2: Discovery Engine

### Objective

Make discovery a real phase with required artifacts and MCP support.

### Backlog

- Add MCP tools for discovery lifecycle:
  - `vision_discovery_start`
  - `vision_research_doc_upsert`
  - `vision_discovery_doc_upsert`
  - `vision_question_add`
  - `vision_question_answer`
  - `vision_decision_add`
  - `vision_acceptance_add`
  - `vision_discovery_ready_check`
  - `vision_discovery_complete`
- Expose existing core artifacts through MCP:
  - milestones
  - architecture decisions
  - sub-visions
- Make research and discovery docs writable through MCP and web APIs, not read-only side files.
- Define blocking versus non-blocking discovery questions.
- Implement discovery readiness rules:
  - discovery record exists
  - docs exist
  - blocking questions resolved
  - acceptance criteria drafted
  - build plan seeded
- Emit VDD change events when discovery artifacts change.

### Likely file touchpoints

- `src/mcp/mod.rs`
- `src/mcp/tools/vision_tools.rs`
- `src/vision.rs`
- `src/web/api.rs`
- `src/state/events.rs`
- `src/bin/vision_hook.rs`
- `tests/`

### Exit criteria

- A feature can move from `planned` to `discovery` through MCP-backed actions.
- Discovery artifacts are first-class and persisted.
- Discovery readiness is computed, not guessed from UI defaults.

## Phase 3: Build Model and Execution Linkage

### Objective

Separate planning from execution while linking the two explicitly.

### Backlog

- Replace generic feature tasks with `BuildTask` semantics in the VDD 2.0 path.
- Add MCP tools for build orchestration:
  - `vision_build_plan_create`
  - `vision_build_task_add`
  - `vision_build_task_update`
  - `vision_build_task_link_queue`
  - `vision_build_task_link_pipeline`
  - `vision_build_sync_git`
  - `vision_build_progress`
  - `vision_build_complete_check`
- Link build tasks to queue task IDs, branches, PRs, commits, and pipeline IDs.
- Import queue status changes into build task state.
- Import git and PR metadata as build evidence, not just free-form strings.
- Prevent entry into `build` unless discovery readiness passes.

### Likely file touchpoints

- `src/vision.rs`
- `src/mcp/mod.rs`
- `src/mcp/tools/vision_tools.rs`
- `src/mcp/tools/queue_tools.rs`
- `src/mcp/tools/factory_tools.rs`
- `src/queue.rs`
- `src/web/api.rs`
- `tests/`

### Exit criteria

- Build tasks are first-class VDD entities.
- Queue and factory identifiers can be linked to build tasks.
- Build readiness and completion are calculated from task state, not manual status edits.

## Phase 4: Verification and Acceptance Engine

### Objective

Make `test` a proof-producing phase rather than a label after coding.

### Backlog

- Add MCP tools for verification:
  - `vision_verification_plan_create`
  - `vision_test_case_add`
  - `vision_verification_run_record`
  - `vision_gate_import`
  - `vision_acceptance_verify`
  - `vision_signoff_add`
  - `vision_test_complete_check`
- Define verification plans with required gates and signoffs.
- Record verification runs from test commands and imported gate results.
- Attach evidence to acceptance criteria.
- Add `pass`, `fail`, `flaky`, and `skipped` verification run states.
- Wire factory and quality gate outcomes into `GateEvidence`.
- Require signoff roles where configured.

### Likely file touchpoints

- `src/vision.rs`
- `src/mcp/mod.rs`
- `src/mcp/tools/vision_tools.rs`
- `src/mcp/tools/factory_tools.rs`
- `src/mcp/tools/quality_tools.rs`
- `src/web/api.rs`
- `tests/`

### Exit criteria

- A feature can enter `test` only with a verification plan.
- Acceptance criteria can be verified against concrete evidence.
- `test -> done` is blocked when gates or signoffs are missing.

## Phase 5: Evidence-Driven Phase Control

### Objective

Replace the current manual status mutation model with guarded transitions.

### Backlog

- Add MCP tools:
  - `vision_phase_status`
  - `vision_phase_advance`
  - `vision_phase_override`
  - `vision_block`
  - `vision_unblock`
- Deprecate direct arbitrary status mutation for normal agent flows.
- Make `phase_advance` run readiness checks and return blockers.
- Restrict overrides to explicit admin-only flows with audit reasons.
- Add rollback and reopen semantics for regressions.
- Write history entries for every phase transition with actor, reason, and evidence references.

### Likely file touchpoints

- `src/vision.rs`
- `src/mcp/mod.rs`
- `src/mcp/tools/vision_tools.rs`
- `src/web/api.rs`
- `tests/`

### Exit criteria

- Normal MCP flows cannot jump directly from `discovery` to `done`.
- Phase changes are derived from evidence or explicit audited overrides.
- Feature blocker reporting is machine-readable.

## Phase 6: Hook Enforcement

### Objective

Make the local agent workflow phase-aware at the point where work happens.

### Backlog

- Upgrade `UserPromptSubmit` classification to route prompts by feature phase.
- On `PreToolUse`, block or warn based on phase:
  - block implementation edits during `discovery`
  - require active build task during `build`
  - warn on fresh implementation edits during `test`
- On `PostToolUse`, detect:
  - build commands
  - test commands
  - lint commands
  - pipeline or PR operations
  - failed runs
- Automatically record build or verification evidence when possible.
- On `Stop`, summarize missing blockers by phase.
- Add override mechanisms that are explicit and logged.

### Likely file touchpoints

- `src/bin/vision_hook.rs`
- `src/vision.rs`
- `src/mcp/mod.rs`
- `tests/`

### Exit criteria

- The hook can enforce the phase contract for common local flows.
- Test and build commands can create evidence automatically.
- Override usage is explicit and auditable.

## Phase 7: Dashboard and Read Model Integration

### Objective

Make the UI display authoritative VDD phase state from a derived read model.

### Backlog

- Add VDD 2.0 read endpoints for:
  - feature readiness
  - blockers
  - acceptance status
  - evidence map
  - signoff state
- Build dashboard views for:
  - phase board
  - discovery blockers
  - open questions
  - active build tasks
  - verification readiness
  - acceptance checklist
  - evidence map
- Replace UI-only `phase` fallback logic with canonical read-model data.
- Emit and consume VDD runtime events like `VisionChanged`, `FeatureChanged`, and `PhaseChanged`.

### Likely file touchpoints

- `src/web/api.rs`
- `src/state/events.rs`
- `src/web/ws.rs`
- `src/web/sse.rs`
- `assets/dashboard.html`
- `tests/`

### Exit criteria

- The dashboard no longer invents feature phase from partial fields.
- Live updates reflect real VDD entity changes.
- Blockers and readiness are visible without manual inspection of files.

## Phase 8: Migration and Decommissioning

### Objective

Move existing projects onto VDD 2.0 and retire the old path safely.

### Backlog

- Build a migration command for `vision.json -> sharded .vision/`.
- Map old statuses into `phase/state`.
- Map old tasks into `BuildTask` and create placeholder verification artifacts where reasonable.
- Backfill discovery records from existing research and discovery markdown.
- Mark incomplete migrations clearly in the read model.
- Deprecate or remove old MCP methods that mutate legacy status directly.
- Add migration validation and rollback docs.

### Likely file touchpoints

- `src/vision.rs`
- new migration module or command
- `src/mcp/mod.rs`
- `src/web/api.rs`
- `tests/`
- `docs/`

### Exit criteria

- Existing projects can be migrated with deterministic output.
- Legacy direct status mutation is no longer the default flow.
- Old and new data paths do not silently diverge.

## Cross-cutting test backlog

These tests should be added progressively, not left until the end.

- feature cannot enter `build` with blocking discovery questions
- feature cannot enter `test` without verification plan
- feature cannot enter `done` with unverified acceptance criteria
- queue status changes update linked build tasks
- imported gate results update readiness
- hook blocks implementation edits during discovery
- hook records verification evidence from test commands
- migration from legacy `vision.json` preserves phase meaning
- dashboard read model reflects canonical VDD state

## Recommended implementation slices

If the goal is to ship value quickly, use these slices instead of trying to land everything at once.

### Slice A

Phase 0 plus Phase 1 minimal:

- explicit `phase` and `state`
- sharded feature storage
- compatibility reads
- feature readiness API

### Slice B

Phase 2 minimal:

- discovery record
- research/discovery upsert
- acceptance criteria
- discovery readiness check

### Slice C

Phase 3 plus Phase 5 minimal:

- build plans and build tasks
- queue linkage
- evidence-driven `phase_advance`

### Slice D

Phase 4 minimal:

- verification plan
- gate evidence
- acceptance verification
- done gating

### Slice E

Phase 6 plus Phase 7:

- hook enforcement
- dashboard blocker/readiness views

## Immediate next build order

If this were started now, the first three engineering batches should be:

1. Phase 0 and Phase 1 together.
2. Phase 2 with discovery MCP and doc upserts.
3. Phase 3 and the minimal Phase 5 guard rails.

That order gets the system to a useful state quickly:

- phases become explicit
- discovery becomes real
- build cannot start by accident

After that, Phase 4 is the critical quality milestone because it turns `test` and `done` into real gates instead of labels.
