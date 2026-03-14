# DX Project Adoption And Recovery

## Why This Exists

Most real projects do not begin cleanly. They arrive half-started:

- code exists but the feature map is incomplete
- branches and worktrees exist but ownership is unclear
- some UI is built but approvals are missing
- documentation is partial or stale
- tests exist in fragments
- the client does not know what is actually done

DXOS needs to do more than track new work. It needs to adopt unfinished work, reconstruct the truth, and drive it to a verified finish.

## Current DXOS Behavior

Project adoption is now a governed control-plane action, not just dashboard advice. Starting adoption seeds:

- a recovery `lead` session contract
- a formal adoption council debate
- an initial assigned recovery work package bound to that lead

When the operator launches that lead lane, DXOS now injects the assigned work package, adoption summary, and recovery council context directly into the runtime prompt and shared lane guidance. The recovery lane does not start from a blank brief anymore.

The portal no longer hand-builds that first package. It now asks DXOS to start governed recovery, and the backend derives the initial summary, objective, feature, and stage from the same `project/brief` recovery model that powers the adoption rail.

That same shared recovery planner now also supplies the first follow-on specialist suggestions. When adoption is marked complete, DXOS seeds planned specialist session contracts and work orders under the recovery lead instead of stopping at a closed recovery ticket.

The operator portal now surfaces those seeded follow-on lanes as a queue. The launch form auto-seeds from the first planned follow-on lane until the operator makes a manual edit, and each queued lane can be applied to the form or launched directly.

That queue is no longer only an adoption-side convenience. DXOS now derives a scheduler-backed `launch_queue` from the control plane itself, so recovery follow-ons, planned workflow runners, and other governed specialist sessions all compete in one ordered execution view. The same scheduler also emits an `attention_queue` for blocked work that needs lead or human intervention.

The scheduler is now controllable two ways: a local autorun loop can watch the queue continuously, and operators or future hosted orchestrators can force one scheduling tick through the dedicated `scheduler_run` control endpoint/tool. That endpoint now also accepts a caller-defined `run_id`, so a hosted orchestrator can retry the same scheduling tick idempotently instead of creating a second competing launch attempt. When autorun is enabled, adoption start/completion and other queue-producing DXOS mutations now kick the scheduler immediately instead of waiting for the next poll window.

DXOS also now supports a contract-driven supervisor target. In the local runtime it can consume the router boundary directly with event-driven kicks; with `DX_HTTP_SUPERVISOR_BASE_URL` set it can instead supervise a remote DXOS instance over the published HTTP contract and its live event stream. Supervisors can now publish an explicit identity (`DX_HTTP_SUPERVISOR_ID`) and launch claims are lease-based with claim IDs, so a dead orchestrator can be replaced, stale `launching` sessions reclaimed, and same-run retries replayed safely instead of remaining stuck indefinitely or racing a duplicate launch.

The scheduler is no longer opaque in the portal. DXOS now publishes recent scheduler ticks and active launch leases as part of the same control-plane snapshot, so operators can see which supervisor claimed a lane, which `run_id` produced the current launch attempt, and whether a tick was replayed or actually advanced work.

Recovery projects can now also be tagged with `company`, `program`, and `workspace` identity through DXOS itself. That metadata is stored on the project record and propagated into the shared DXOS registry, which is the first step from a flat list of repos toward a true multi-company portfolio model.

The shared registry now also groups those project records into company, program, and workspace views. That gives operators a portfolio-level control surface without changing the underlying recovery flow for any one project.

Those portfolio views are no longer read-only derived summaries. DXOS now persists first-class `company`, `program`, and `workspace` records in the shared store, auto-seeds them when project identity is saved, and lets operators update their `status`, `owner`, and `summary` from the portal or MCP. That makes recovery planning company-aware without forcing every portfolio rule back down into project labels.

Operator policy now understands those same scopes. A lead or operator can be authorized for one company, one program, or one workspace instead of being limited to a flat project name match. The portal also uses the grouped registry to surface sibling projects, so switching between related recovery efforts no longer depends on the tmux/runtime workspace list alone.

The portal header now also filters the live project selector by company and program, and DXOS exposes a portfolio brief endpoint for that same scope. That means a hosted control plane can summarize the current portfolio slice across projects instead of pretending one repo brief is enough.

## Core Promise

If a company points DXOS at an in-progress project, the platform should be able to:

1. ingest the current project state
2. identify what is missing
3. explain the recovery plan in plain language
4. launch the right specialist lanes
5. keep docs, Git, runtime state, and approvals synchronized
6. move the project to a trusted release state

## One-Screen Model

```mermaid
flowchart LR
    A[Existing repo and docs] --> B[DXOS intake]
    B --> C[Truth reconstruction]
    C --> D[Recovery assessment]
    D --> E[Suggested specialist lanes]
    E --> F[Build, test, docs, approvals]
    F --> G[Verified completion]
```

## What DXOS Must Reconstruct

When DXOS adopts a project, it should rebuild these facts before pretending work is under control:

- which features exist
- which stage each feature is actually in
- which approvals are missing
- which docs are missing or stale
- which acceptance criteria are absent
- which runtime lanes are active
- which branches and worktrees are live
- which blockers are preventing the next stage

This is why the project brief now carries a `recovery` block instead of only a live status snapshot.

## Recovery Modes

### 1. `unscoped`

The repo has code, runtime activity, or docs, but DXOS does not yet have a trustworthy feature map.

DXOS response:

- launch a discovery or lead recovery lane
- inventory existing work
- map features and stages
- create the first structured plan

### 2. `structured_planning`

The project has defined features, but the execution network is not fully active yet.

DXOS response:

- identify what is ready
- identify what specialist lanes are missing
- suggest the next governed launch

### 3. `adopt_in_progress`

The project is already in motion and needs a recovery pass rather than a blank-start plan.

DXOS response:

- highlight blockers
- surface missing docs or acceptance coverage
- show missing client approvals
- recommend the next recovery lane immediately

## Recovery Assessment Inputs

The current recovery model uses:

- feature phase counts
- documentation health
- blocked features
- ready features
- client review queue
- active runtime count
- worktree count

This gives DXOS enough signal to say:

- the project needs discovery reconstruction
- the project needs design approval
- the project needs a lead recovery lane
- the project needs QA or docs cleanup before it can be trusted

## Suggested Specialist Lanes

Recovery is only useful if it becomes action.

DXOS should recommend specialist lanes such as:

- `discovery` when research or discovery docs are missing
- `design` when client approval is blocking build
- `frontend` or `build` when work is ready but no lane is active
- `qa` when acceptance or verification evidence is missing
- `docs` when handbook and Git have drifted
- `lead` when blockers need routing, sequencing, or approvals

Those suggestions should not live only in prose. They should prefill governed operator controls directly in the portal.

For adoption specifically, the first suggestion is now materialized as a real DXOS work package rather than a generic “launch a lead” recommendation.
The remaining specialist suggestions are preserved on the adoption record and become planned follow-on lanes when the recovery lead completes the adoption workflow.

## Operator Flow

```mermaid
flowchart TD
    A[Open project brief] --> B[Read recovery assessment]
    B --> C[Inspect adoption gaps]
    C --> D[Apply suggested lane]
    D --> E[Review provider and runtime substrate]
    E --> F[Launch governed session]
    F --> G[Collect evidence and update docs]
```

## Client-Facing Outcome

The client should not see “we found a messy repo.”

The client should see:

- what has already been understood
- what is still missing
- what is waiting for approval
- what specialist work is happening now
- what must happen before release is trusted

That is the real value of recovery mode. It turns inherited chaos into a readable delivery narrative.

## Completion Standard

A recovered project should not be called complete until these are aligned:

- feature and stage map
- discovery and design history
- implementation state
- test and acceptance evidence
- documentation health
- release readiness

If any of those are missing, DXOS should continue to describe the project as adopted but incomplete.
