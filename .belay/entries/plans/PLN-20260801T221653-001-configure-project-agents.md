---
schema_version: 1
id: PLN-20260801T221653-001-configure-project-agents
type: plan
title: configure-project-agents
status: approved
created_at: 2026-08-01T22:16:53+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260801T221649-001-configure-project-agent
metadata: {}
---

## Tier Classification

- Tier 2: repository developer workflow changes are meaningful but reversible, with bounded files and no runtime application behavior change.

## Problem

- The repository defines review budgets but does not configure or distinguish the primary coordinator, implementation worker, and independent reviewer models.

## Desired Outcome

- Codex uses Sol low as the primary agent, Luna low for bounded approved implementation, and Sol for independent review with reasoning escalation when risk requires it.

## Success Signals

- Project config and two custom agent files are valid and discoverable.
- AGENTS.md routes Tier 2 implementation to the implementer only when scope is explicit and verification is available.
- Tier 3 implementation remains with Sol-capable handling and Tier 3 review raises reasoning above low.

## Constraints

- Only repository-scoped files may change.
- Explicit implementation approval is already provided by the current user message.
- Existing unrelated parent-change files remain untouched.

## Non-goals

- Global Codex configuration.
- Autonomous delegation for Tier 1.
- Luna implementation for unresolved design, security, migrations, public APIs, concurrency, or difficult rollback.

## Assumptions

- The user wants persistent project behavior rather than a one-thread instruction.
- The custom agent model field accepts the public Codex model slug `gpt-5.6-luna` even if this session cannot explicitly override to it.

## Unknowns / Decisions Needed

- Unknown: whether this account/runtime currently permits spawning Luna. Validate configuration recognition and report runtime availability separately.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
|---|---|---|---|---|---|
| T-001 | SC-001 | Add repository primary defaults and explicit agent registrations | Codex | verified | Four TOML files parse; registrations resolve to existing files |
| T-002 | SC-002 | Add bounded Luna implementer | Codex | verified | Model, low reasoning, write sandbox, and Tier 3 refusal inspected |
| T-003 | SC-003 | Add independent Sol reviewer with risk escalation | Codex | verified | Sol low reviewer and Sol high high-risk reviewer are read-only |
| T-004 | SC-004 | Add delegation rules to AGENTS.md | Codex | verified | Independent review passed after one remediation pass |
| T-005 | SC-005 | Validate and record runtime limitations | Codex | verified | TOML parse passed; runtime discovery Unknown due user-global config error |
