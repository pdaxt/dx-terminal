# DX Terminal Phase 2 Strategy — Category Killer

## Date: 2026-03-15
## Source: Codex (gpt-5.4) + Claude (opus-4.6) dual strategic analysis

---

## Core Reframe

**Don't be another agent. Be the control plane for ALL agents.**

DX Terminal wins by orchestrating Claude, Codex, Gemini, Aider, and future agents — not by replacing them. Position: "Kubernetes for coding agents."

---

## 4 Moats (Hard to Replicate)

1. **Provider-neutral control plane** — Schedule, supervise, verify, and optimize any coding agent
2. **Evidence graph** — Requirement → research → task → code → test → PR → deploy → rollback
3. **Repo-specific routing data** — Which model works best for this codebase, for this task, at this cost
4. **Operational state ownership** — Branches, worktrees, browser sessions, secrets, approvals, CI, incidents

## 5 Competitor Pain Points We Nail

1. Single-agent tools are bad at parallel, async, multi-repo work
2. No tool answers: "What changed, why is this safe, should I merge?"
3. Teams switch between editor, terminal, GitHub, CI, Slack, PM tools
4. No one has nailed human supervision for many agents at once
5. Cost governance, retry policy, failure recovery, auditability are weak everywhere

## Killer Feature: Backlog → PR Swarm

Connect GitHub/Linear/Jira, pick 20 issues, DX spawns supervised parallel agents that return tested PRs with evidence, risk summaries, and rollback plans.

The overnight-switch moment: "I can clear a sprint in one afternoon without losing control."

## Phase 2 Goals

| ID | Goal | Description |
|----|------|-------------|
| G10 | Session Control Protocol | Zero-touch agent lifecycle: spawn → work → commit → handoff → next issue. Self-healing. Cross-session memory. |
| G11 | Provider-Neutral Agent Router | Run Claude, Codex, Gemini, Llama. Route by task type and cost. Track $/PR metrics. |
| G12 | Issue → PR Swarm | Connect GitHub Issues, spawn N agents in parallel, return tested PRs with evidence. |
| G13 | Trust Chain / Evidence Graph | Every PR ships with machine-readable proof chain: requirement → decision → code → test → evidence. |
| G14 | `dx go` Zero-Config Launch | `cargo install dx-terminal && dx go` — detects project, connects GitHub, starts working on issues. Under 60 seconds. |
| G15 | Cost Governance Dashboard | $/PR, $/issue, model comparison, team analytics. Enterprise CFO sell. |

## Go-to-Market

1. Open-source orchestration core. Charge for hosted control plane + enterprise governance
2. Launch with brutal live demo: 10 issues, 10 agents, tested PRs, one dashboard
3. Position as "Kubernetes for coding agents" or "engineering control plane for AI teams"
4. Don't ask devs to abandon Cursor/Codex — make DX orchestrate them
5. Give OSS maintainers free "issue-to-PR swarm" workflow — maintainers are the fastest star amplifier

## What's Missing for $1B

- Hosted multi-user SaaS (terminal-only is niche ceiling)
- CI/CD, deploys, incidents, environments, rollback (become "software delivery OS")
- Enterprise controls: SSO, RBAC, policy, secrets, compliance, approvals, audit
- Financial proof: throughput, cost per merged PR, bug escape rate, model ROI
- A standard others adopt: extend AGENTS.md into agent work contracts + evidence + supervision protocol

## Brutal Truth

> 206 tools, a Rust binary, and a dashboard are not moats. They are product features. The category-killer move is to become the system every other coding agent runs through.
