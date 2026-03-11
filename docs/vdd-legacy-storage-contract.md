# VDD Legacy Storage Contract

This document records the current `.vision` storage shape that VDD 2.0 must keep reading during migration.

## Canonical file

The current canonical file is:

`<project>/.vision/vision.json`

Audit history is appended to:

`<project>/.vision/history.jsonl`

Recursive sub-visions are stored under:

`<project>/.vision/features/<feature_id>.json`

## Top-level shape

`vision.json` currently stores one large object with these top-level keys:

- `project`
- `mission`
- `principles`
- `goals`
- `milestones`
- `architecture`
- `changes`
- `features`
- `github`
- `updated_at`

## Goal shape

Each goal contains:

- `id`
- `title`
- `description`
- `status`
- `priority`
- `linked_issues`
- `metrics`

Legacy goal statuses:

- `planned`
- `in_progress`
- `achieved`
- `deferred`
- `dropped`

## Feature shape

Legacy feature records are still expected to serialize with:

- `id`
- `goal_id`
- `title`
- `description`
- `status`
- `questions`
- `decisions`
- `tasks`
- `acceptance_criteria`
- `sub_vision`
- `parent_vision`
- `created_at`
- `updated_at`

The current tree also persists VDD 2.0 compatibility fields:

- `phase`
- `state`

Compatibility rule:

- old files may contain only `status`
- current loaders must backfill `phase/state` from `status`
- current writers must keep `status` aligned with `phase`

Legacy feature statuses:

- `planned`
- `specifying`
- `building`
- `testing`
- `done`

Current VDD 2.0 phase mapping:

- `planned -> planned`
- `specifying -> discovery`
- `building -> build`
- `testing -> test`
- `done -> done`

Current VDD 2.0 state mapping:

- `planned -> planned`
- `specifying/building/testing -> active`
- `done -> complete`

## Question shape

Each feature question contains:

- `id`
- `text`
- `status`
- `answer`
- `asked_at`
- `answered_at`
- `decision_id`

Legacy question statuses:

- `open`
- `answered`
- `revised`

## Decision shape

Each feature decision contains:

- `id`
- `question_id`
- `decision`
- `rationale`
- `date`
- `alternatives`

## Task shape

Each feature task contains:

- `id`
- `feature_id`
- `title`
- `description`
- `status`
- `branch`
- `pr`
- `commit`
- `assignee`
- `created_at`
- `updated_at`

Legacy task statuses:

- `planned`
- `in_progress`
- `done`
- `verified`
- `blocked`

## Change history shape

Each `changes` item in `vision.json`, and each JSON object line in `history.jsonl`, contains:

- `timestamp`
- `change_type`
- `field`
- `old_value`
- `new_value`
- `reason`
- `triggered_by`
- `github_issue`

## Current compatibility consumers

The current codebase still has consumers that depend on the legacy aggregate shape:

- `src/vision.rs`
  - canonical load/save and legacy status transitions
- `src/mcp/tools/vision_tools.rs`
  - thin wrappers over `vision.rs`
- `src/mcp/mod.rs`
  - VDD MCP tool registration
- `src/web/api.rs`
  - tree, drill, feature status, docs, and feature readiness endpoints
- `src/web/mod.rs`
  - web route registration
- `assets/dashboard.html`
  - reads the aggregated API responses, not sharded entity files directly

## Migration constraint

Until VDD 2.0 sharded storage becomes authoritative:

- `vision.json` remains the compatibility source of truth
- new sharded entity files must be derivable from it
- migration code must preserve legacy `status` semantics even when `phase/state` are present
