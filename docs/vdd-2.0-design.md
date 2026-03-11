# VDD 2.0 Design

## Purpose

VDD 2.0 turns Vision-Driven Development from a planning overlay into an enforceable workflow system.

The target workflow is:

`discovery -> build -> test -> done`

The system must do more than store goals and tasks. It must:

- model each phase explicitly
- require evidence to leave a phase
- connect planning artifacts to execution artifacts
- let agents operate through MCP tools
- let hooks guide and constrain behavior
- let the dashboard show authoritative phase status in real time

## Problems in VDD 1.x

The current VDD implementation has good primitives, but it is still mostly advisory.

Main gaps:

- discovery is mostly a UI concept, not a first-class state contract
- feature phases can be advanced manually without evidence
- test/verification is not modeled as first-class data
- research/discovery docs exist as loose markdown, outside the MCP contract
- factory quality gates are not wired back into VDD evidence
- the hook enforces branch/task linkage, but not actual discovery/build/test discipline
- the canonical store is still too monolithic for multi-agent work

## Design goals

1. Make phases explicit and authoritative.
2. Make evidence, not manual status changes, drive progression.
3. Keep the system MCP-native so agents can operate it directly.
4. Integrate execution systems like queue/factory/quality gates.
5. Reduce merge conflicts in `.vision` storage.
6. Make dashboard views derived from authoritative read models.

## Non-goals

- Full project management replacement
- Rich document editor inside DX Terminal
- External workflow engine beyond the local queue/factory system

## Core principles

### Phase-first, not status-first

VDD 2.0 separates lifecycle phase from operational state.

Example:

- phase: `discovery`
- state: `active`

A feature should not encode both concerns into one overloaded `status` field.

### Evidence over declarations

Agents should not be able to move a feature to `done` just because they finished coding.

Each phase transition requires concrete artifacts:

- discovery requires questions, decisions, docs, and acceptance criteria
- build requires implementation tasks and linked execution
- test requires verification evidence and gate results
- done requires verified acceptance criteria and signoff

### Canonical store plus derived read model

The canonical VDD store should be sharded and append-friendly.

The dashboard should read a derived aggregated view.

### Execution is not planning

VDD stores intent and evidence.

Queue and factory execute work.

The integration point is explicit linkage, not implicit inference.

## High-level architecture

VDD 2.0 has four cooperating layers:

1. VDD Store
   Canonical project vision state and phase artifacts.

2. VDD MCP
   The agent-facing protocol for reading and mutating the VDD store.

3. VDD Hooks
   Guardrails on prompt submission, file edits, commits, PRs, tests, and stop events.

4. VDD Read Model
   Aggregated, dashboard-friendly view derived from the canonical store and execution evidence.

## Canonical data model

### Top-level entities

- `Vision`
- `Goal`
- `Feature`
- `DiscoveryRecord`
- `Question`
- `Decision`
- `AcceptanceCriterion`
- `BuildPlan`
- `BuildTask`
- `VerificationPlan`
- `VerificationRun`
- `GateEvidence`
- `Signoff`

### Phase model

Each feature gets:

```json
{
  "phase": "discovery",
  "state": "active",
  "phase_entered_at": "2026-03-12T10:00:00Z",
  "blocked_reason": null
}
```

Valid feature phases:

- `planned`
- `discovery`
- `build`
- `test`
- `done`
- `dropped`

Valid feature states:

- `planned`
- `active`
- `blocked`
- `complete`
- `cancelled`

### Feature model

```json
{
  "id": "F1.1",
  "goal_id": "G1",
  "title": "WebSocket streaming",
  "description": "Real-time data via WS",
  "phase": "discovery",
  "state": "active",
  "priority": 1,
  "created_at": "2026-03-12T10:00:00Z",
  "updated_at": "2026-03-12T10:00:00Z",
  "owner": null,
  "branch_strategy": "feature_branch",
  "acceptance_criteria_ids": ["AC1", "AC2"],
  "discovery_record_id": "DR1.1",
  "build_plan_id": null,
  "verification_plan_id": null,
  "signoff_ids": []
}
```

### Discovery record

Discovery is its own structured entity, not just questions on the feature.

```json
{
  "id": "DR1.1",
  "feature_id": "F1.1",
  "research_doc": ".vision/research/F1.1.md",
  "discovery_doc": ".vision/discovery/F1.1.md",
  "status": "in_progress",
  "open_questions": 2,
  "decisions_made": 1,
  "risks": [
    "SSE fallback required for older clients"
  ],
  "alternatives_considered": [
    "SSE",
    "polling"
  ],
  "ready_for_build": false
}
```

### Acceptance criterion

Acceptance criteria need their own verification state.

```json
{
  "id": "AC1",
  "feature_id": "F1.1",
  "text": "Sub-second update latency on dashboard",
  "status": "draft",
  "verification_method": "integration_test",
  "evidence_ids": [],
  "verified_at": null
}
```

Valid criterion statuses:

- `draft`
- `mapped`
- `verified`
- `failed`

### Build plan

```json
{
  "id": "BP1.1",
  "feature_id": "F1.1",
  "tasks": ["BT1", "BT2", "BT3"],
  "pipeline_id": "pipe_123",
  "status": "active"
}
```

### Build task

Build tasks replace the generic "task only means implementation maybe" ambiguity.

```json
{
  "id": "BT1",
  "feature_id": "F1.1",
  "kind": "implementation",
  "title": "Implement WS broadcast loop",
  "description": "Add Axum websocket broadcast path",
  "status": "in_progress",
  "branch": "feat/f1-1-ws-loop",
  "pr": "#42",
  "commit": "abc123",
  "queue_task_id": "q_001",
  "pipeline_stage": "dev",
  "assignee": "pane-3"
}
```

Valid build task kinds:

- `implementation`
- `refactor`
- `migration`
- `review_fix`

### Verification plan

```json
{
  "id": "VP1.1",
  "feature_id": "F1.1",
  "test_cases": ["TC1", "TC2"],
  "required_gates": ["build", "test", "lint"],
  "required_signoffs": ["qa"],
  "status": "active"
}
```

### Verification run

```json
{
  "id": "VR1",
  "feature_id": "F1.1",
  "type": "integration_test",
  "tool": "cargo test",
  "result": "pass",
  "output_ref": ".vision/evidence/F1.1/VR1-output.txt",
  "recorded_at": "2026-03-12T12:00:00Z"
}
```

Valid verification run results:

- `pass`
- `fail`
- `flaky`
- `skipped`

### Gate evidence

This is the bridge from factory/quality systems into VDD.

```json
{
  "id": "GE1",
  "feature_id": "F1.1",
  "pipeline_id": "pipe_123",
  "gate": "test",
  "passed": true,
  "summary": "124 tests passed",
  "source": "factory_gate",
  "recorded_at": "2026-03-12T12:05:00Z"
}
```

### Signoff

```json
{
  "id": "SO1",
  "feature_id": "F1.1",
  "role": "qa",
  "actor": "pane-7",
  "decision": "approved",
  "notes": "Verified against AC1 and AC2",
  "recorded_at": "2026-03-12T12:10:00Z"
}
```

## Storage layout

VDD 1.x keeps too much in a single `vision.json`. That becomes a merge hotspot for multi-agent work.

VDD 2.0 should shard canonical state.

### Proposed `.vision/` layout

```text
.vision/
  manifest.json
  history.jsonl
  goals/
    G1.json
  features/
    F1.1.json
  discovery/
    F1.1.md
  research/
    F1.1.md
  build/
    BP1.1.json
    BT1.json
  verification/
    VP1.1.json
    VR1.json
  evidence/
    F1.1/
      VR1-output.txt
      gate-test.json
  signoffs/
    SO1.json
  read-model.json
```

### Storage rules

- `manifest.json` stores project-level metadata and indexes.
- entity files are canonical for goals, features, plans, runs, signoffs
- markdown remains valid for research/discovery prose
- `history.jsonl` stores append-only audit entries
- `read-model.json` is derived and disposable

## Phase contract

### Planned

Entry:

- feature exists

Required artifacts:

- goal linkage
- title
- description

Exit to discovery:

- feature activated explicitly or inferred from matched work

### Discovery

Purpose:

- understand the problem before coding

Required artifacts:

- discovery record exists
- at least one discovery or research doc exists
- open questions tracked
- decisions captured for resolved questions
- acceptance criteria drafted

Allowed work:

- research notes
- questions
- decisions
- sub-vision creation
- architecture decisions

Exit to build requires:

- zero blocking open questions
- discovery record marked ready
- at least one acceptance criterion in `mapped` or better
- build plan created

### Build

Purpose:

- implement the feature according to discovery decisions

Required artifacts:

- approved discovery record
- build plan
- build tasks linked to queue/factory execution

Allowed work:

- create implementation tasks
- assign queue/factory work
- link branches, PRs, commits
- track build progress

Exit to test requires:

- all build tasks complete
- no blocking build tasks
- verification plan exists

### Test

Purpose:

- prove the feature works and satisfies acceptance criteria

Required artifacts:

- verification plan
- verification runs or imported gate evidence
- acceptance criteria mapped to evidence

Allowed work:

- execute tests
- import gate results
- record QA/reviewer signoff
- mark acceptance criteria verified or failed

Exit to done requires:

- all required gates passed
- all required acceptance criteria verified
- required signoff recorded

### Done

Purpose:

- accepted, verified, and complete

Required artifacts:

- phase exit checks from test satisfied

Allowed work:

- reopen only through explicit rollback or regression workflow

## Automatic phase rules

The system should derive phase progression whenever possible.

### Rules

- A feature enters `discovery` when the first discovery record, question, or research doc is created.
- A feature enters `build` only when discovery readiness checks pass and a build plan exists.
- A feature enters `test` only when all build tasks are complete and a verification plan exists.
- A feature enters `done` only when verification checks pass.

### Manual overrides

Manual overrides should exist, but only as explicit admin actions:

- `vision_phase_override(feature_id, to_phase, reason)`

Overrides must be written to history with actor and reason.

## MCP surface

VDD 2.0 MCP should be organized by phase.

### Core

- `vision_init`
- `vision_goal_create`
- `vision_goal_update`
- `vision_feature_create`
- `vision_feature_get`
- `vision_feature_list`
- `vision_tree`
- `vision_drill`

### Discovery MCP

- `vision_discovery_start`
- `vision_research_doc_upsert`
- `vision_discovery_doc_upsert`
- `vision_question_add`
- `vision_question_answer`
- `vision_decision_add`
- `vision_acceptance_add`
- `vision_acceptance_update`
- `vision_subvision_create`
- `vision_arch_decision_add`
- `vision_discovery_ready_check`
- `vision_discovery_complete`

### Build MCP

- `vision_build_plan_create`
- `vision_build_task_add`
- `vision_build_task_update`
- `vision_build_task_link_queue`
- `vision_build_task_link_pipeline`
- `vision_build_sync_git`
- `vision_build_progress`
- `vision_build_complete_check`

### Test MCP

- `vision_verification_plan_create`
- `vision_test_case_add`
- `vision_verification_run_record`
- `vision_gate_import`
- `vision_acceptance_verify`
- `vision_signoff_add`
- `vision_test_complete_check`

### Phase control MCP

- `vision_phase_status`
- `vision_phase_advance`
- `vision_phase_override`
- `vision_block`
- `vision_unblock`

### Read model MCP

- `vision_read_model`
- `vision_feature_readiness`
- `vision_feature_blockers`
- `vision_feature_evidence`

## Hook behavior

Hooks should move from soft advisory prompts to phase-aware enforcement.

### UserPromptSubmit

Current role:

- classify prompt against existing goals/features

VDD 2.0 behavior:

- classify work against feature or goal
- if feature is in `discovery`, inject discovery workflow only
- if feature is in `build`, inject build task context and branch linkage
- if feature is in `test`, inject verification workflow and failing criteria
- if unmatched, suggest feature creation in `planned` or `discovery`

### PreToolUse Edit/Write

Current role:

- warn on edits in vision projects without task linkage

VDD 2.0 behavior:

- if feature phase is `discovery`, block implementation file edits unless explicitly overridden
- if feature phase is `build`, require active build task and branch linkage
- if feature phase is `test`, allow test/evidence/docs edits but warn on new implementation edits

### PostToolUse Bash

Current role:

- warn on commits without task status updates

VDD 2.0 behavior:

- detect build commands and attach build evidence
- detect test commands and attach verification runs
- detect lint/build/test gate commands
- detect PR creation/merge and link to build task
- detect failed test output and mark verification runs as failed

### Stop

Current role:

- summarize session work

VDD 2.0 behavior:

- summarize changes by phase
- list incomplete discovery blockers
- list build tasks without evidence
- list verification gaps before done

## Integration with queue and factory

VDD must not duplicate the execution engine.

### Queue integration

- build tasks can link to queue task IDs
- queue state updates should update build task state
- feature readiness should depend on linked task completion

### Factory integration

- build plans can create pipelines
- pipeline stages map to build and verification artifacts
- gate results import automatically as `GateEvidence`
- failed gates should prevent `test -> done`

### Quality integration

- verification plans declare required gates
- quality results become canonical evidence
- dashboard can show criterion -> gate -> evidence mapping

## Event model

VDD 2.0 should emit first-class events into the runtime stream.

### Events

- `VisionChanged`
- `GoalChanged`
- `FeatureChanged`
- `DiscoveryChanged`
- `BuildTaskChanged`
- `VerificationChanged`
- `AcceptanceChanged`
- `SignoffChanged`
- `PhaseChanged`

### Event payload

Each event should include:

- project
- entity type
- entity id
- changed fields
- phase
- summary
- actor
- timestamp

## Dashboard/read model

The dashboard should render VDD from a derived read model, not by inferring state from mixed fields.

### Required views

- feature phase board
- discovery blockers
- open questions
- active build tasks
- verification readiness
- acceptance criteria checklist
- evidence map
- signoff state

### Readiness summaries

Each feature should expose:

```json
{
  "feature_id": "F1.1",
  "phase": "test",
  "state": "active",
  "ready_for_build": true,
  "ready_for_test": true,
  "ready_for_done": false,
  "blockers": [
    "AC2 not verified",
    "QA signoff missing"
  ]
}
```

## Migration from VDD 1.x

### Storage migration

- read existing `.vision/vision.json`
- split goals and features into sharded files
- preserve `history.jsonl`
- create `read-model.json`

### Field mapping

Map old feature status:

- `planned` -> phase `planned`, state `planned`
- `specifying` -> phase `discovery`, state `active`
- `building` -> phase `build`, state `active`
- `testing` -> phase `test`, state `active`
- `done` -> phase `done`, state `complete`

Map old tasks:

- current `VisionTask` becomes `BuildTask` by default
- `verified` task state becomes build task `complete` plus verification artifact if evidence exists

### Discovery artifacts

- if `.vision/research/<feature>.md` exists, attach to discovery record
- if `.vision/discovery/<feature>.md` exists, attach to discovery record
- if neither exists, create empty discovery record for any feature in `specifying`

## Rollout plan

### Phase 1

- introduce new data types alongside old ones
- add read/write APIs for discovery, build, and verification artifacts
- keep old `vision.json` compatibility

### Phase 2

- route VDD MCP through the new sharded store
- wire queue/factory evidence into VDD
- emit VDD events to the runtime event bus

### Phase 3

- update dashboard to consume VDD 2.0 read model
- add phase blocker widgets
- stop reading fake `phase` fallbacks from UI-only logic

### Phase 4

- make hooks phase-aware and partially enforcing
- block phase-invalid operations unless override is used

### Phase 5

- deprecate direct manual status setting
- replace with evidence-based `phase_advance`

## Minimum viable implementation

If VDD 2.0 is implemented incrementally, the minimum viable version should include:

1. explicit `phase` and `state` on features
2. discovery record entity
3. verification plan and gate evidence entity
4. queue/factory linkage on build tasks
5. MCP write support for research/discovery docs
6. evidence-based `phase_advance`
7. emitted `VisionChanged` events

## Bottom line

VDD 2.0 should make the system behave like a real phase engine:

- discovery produces decisions and acceptance criteria
- build produces implementation artifacts
- test produces proof
- done means accepted, not merely coded

The key design choice is simple:

Do not let agents move phases by opinion.

Make them move phases by evidence.
