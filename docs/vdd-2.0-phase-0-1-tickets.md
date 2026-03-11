# VDD 2.0 Phase 0 + Phase 1 Tickets

This is the execution checklist for the first delivery slice:

- stabilize the current VDD baseline
- introduce the minimum VDD 2.0 schema
- expose the first authoritative read model fields

## Status key

- `[ ]` not started
- `[-]` in progress
- `[x]` done

## Phase 0: Baseline

### P0-01 Validate current baseline

- `[x]` Run the current test suite and confirm the repo is buildable.
- `[x]` Confirm VDD-related tests are present before refactoring.

Done when:

- `cargo test -q` passes.

### P0-02 Record the current VDD storage contract

- `[x]` Document the current `.vision/vision.json` feature, question, decision, and task shape.
- `[x]` Note the current compatibility assumptions for web and MCP consumers.

Depends on:

- `P0-01`

Done when:

- A short doc exists describing the legacy VDD shape that VDD 2.0 must keep reading during migration.

### P0-03 Tighten regression coverage around feature lifecycle

- `[x]` Add tests for legacy feature status transitions.
- `[x]` Add tests for load/save compatibility when new fields are absent.
- `[x]` Add tests for new read-model fields once added.

Depends on:

- `P0-01`

Done when:

- The first VDD 2.0 schema changes are protected by compatibility tests.

## Phase 1: Canonical VDD 2.0 Schema

### P1-01 Add explicit feature `phase` and `state`

- `[x]` Add a VDD 2.0 `phase` enum for `planned/discovery/build/test/done`.
- `[x]` Add a VDD 2.0 `state` enum for `planned/active/blocked/complete`.
- `[x]` Persist both on `Feature` while keeping legacy `status`.
- `[x]` Normalize old feature records that only have legacy `status`.

Depends on:

- `P0-01`

Done when:

- New feature JSON includes `phase` and `state`.
- Old `vision.json` files still load correctly.
- Legacy `status` remains available for existing consumers.

### P1-02 Add lifecycle normalization helpers

- `[x]` Centralize mapping between legacy `status` and VDD 2.0 `phase/state`.
- `[x]` Route all feature lifecycle mutations through the helper.
- `[x]` Ensure add-question, add-task, task-update, git-sync, and feature-update all keep fields aligned.

Depends on:

- `P1-01`

Done when:

- No feature mutation path updates `status` without also updating `phase/state`.

### P1-03 Add a first readiness read model

- `[x]` Compute `ready_for_build`, `ready_for_test`, and `ready_for_done`.
- `[x]` Return blockers for unmet readiness conditions.
- `[x]` Surface readiness in at least one machine-readable API.

Depends on:

- `P1-02`

Done when:

- A caller can ask for feature readiness and receive blockers plus readiness booleans.

### P1-04 Expose readiness through MCP and web

- `[x]` Add a VDD MCP read tool for feature readiness.
- `[x]` Add a web endpoint for feature readiness.
- `[x]` Include `phase/state` and readiness in existing vision tree or drill payloads.

Depends on:

- `P1-03`

Done when:

- MCP and web clients can consume readiness without parsing raw `vision.json`.

### P1-05 Add compatibility tests for the new schema

- `[x]` Test that old features with only `status` are normalized on load.
- `[x]` Test that question/task lifecycle changes update both legacy and VDD 2.0 fields.
- `[x]` Test readiness output for planned, discovery, build, and test scenarios.

Depends on:

- `P1-01`
- `P1-02`
- `P1-03`

Done when:

- The VDD 2.0 minimum schema is protected by tests.

## Recommended execution order

1. `P1-01`
2. `P1-02`
3. `P1-03`
4. `P1-04`
5. `P1-05`
6. `P0-02`
7. `P0-03`

## Current implementation target

The code work starting now covers:

- `P0-03`
- `P1-01`
- `P1-02`
- `P1-03`
- `P1-04`
- `P1-05`
