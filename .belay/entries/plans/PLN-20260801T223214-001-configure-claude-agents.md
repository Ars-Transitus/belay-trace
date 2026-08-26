---
schema_version: 1
id: PLN-20260801T223214-001-configure-claude-agents
type: plan
title: configure-claude-agents
status: approved
created_at: 2026-08-01T22:32:14+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260801T223214-001-configure-claude-agent-m
metadata: {}
---

## Tier Classification

- Tier 2: bounded, reversible repository developer-workflow configuration.

## Problem

- Claude Code has the Belay skill and RTK instructions but no shared main-model setting or specialized project subagents matching the repository review policy.

## Desired Outcome

- Claude Code uses Opus low for coordination, Sonnet low for bounded approved Tier 2 implementation, Opus low for normal review, and Opus high for high-risk review.

## Success Signals

- `.claude/settings.json` and three `.claude/agents/*.md` definitions are valid.
- CLAUDE.md imports the repository policy without losing RTK guidance.
- Claude Code reports the settings as valid and exposes the custom agent names.

## Constraints

- Current user message explicitly approves repository-scoped implementation.
- Local settings and unrelated parent changes remain untouched.
- Review agents cannot edit; implementer cannot perform Tier 3 work.

## Non-goals

- Changing global Claude defaults.
- Duplicating all AGENTS.md policy in CLAUDE.md.
- Enabling autonomous Tier 1 delegation.

## Assumptions

- This is a hypothesis: Opus/Sonnet are the closest Claude role mapping for the requested Sol/Luna split.
- Claude Code 2.1.220 supports project agents, model aliases, and per-agent effort frontmatter.

## Unknowns / Decisions Needed

- Unknown: which full model IDs the `opus` and `sonnet` aliases resolve to for this account at runtime.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
|---|---|---|---|---|---|
| T-001 | SC-001 | Add shared Opus low project settings | Codex | verified | JSON parsed and Claude doctor loaded project settings |
| T-002 | SC-002 | Add bounded Sonnet low implementer | Codex | verified | Frontmatter, constraints, and agent discovery verified |
| T-003 | SC-003 | Add Opus low and Opus high read-only reviewers | Codex | verified | Tool restrictions and two-round independent review passed |
| T-004 | SC-004 | Load repository workflow from CLAUDE.md | Codex | verified | RTK block preserved and AGENTS.md import present |
| T-005 | SC-005 | Review, validate, and record Evidence | Codex | verified | All three agents executed; doctor, review, Belay, and jj checks passed |
