---
schema_version: 1
id: PLN-20260925T214756-001-simplify-belay-guidance
type: plan
title: simplify-belay-guidance
status: completed
created_at: 2026-09-25T21:47:56+09:00
updated_at: 2026-09-26T00:46:13+09:00
revision: 6
tags: []
links:
- relation: fulfills
  id: GOAL-20260925T214740-001-belay-guidance-boundarie
metadata: {}
---

## Intent Brief

### Problem

- Generic Belay guidance duplicates consumer review/acceptance policy and loads Route details for unrelated work.

### Desired Outcome

- Lean guidance with unchanged trace and authority contracts; complete Route reference distribution.

### Success Signals

- Focused regression tests, full required Rust checks, and fresh independent review pass.

### Constraints

- User authorized Belay-only fixes in current conversation. Preserve prior src/agent.rs changes. No installed consumer mutation.

### Non-goals

- Erwin configuration, model selection, coverage semantics, schema and external publication.

### Assumptions

- Brief optionality is guidance-only if lint already permits it; validate before editing runtime.

### Unknowns / Decisions Needed

- Runtime implications checked before implementation; performance improvement remains unmeasured.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Simplify generic guidance and distribute Route reference | implement_medium | verified | EVD-20260926T004556-001; EVD-20260926T004556-002 |

## T-001

- Objective: Deliver the scoped Belay guidance changes with complete reference lifecycle.
- Scope: src/agent.rs, canonical reference asset if needed, tests/cli.rs, docs/design/agent-rules-audit.md, directly relevant docs.
- Steps: Check lint and coverage contracts; revise policy wording and brief template; extract Route instructions without weakening approval; update generation/install/doctor safely; test and review.
- Acceptance: Goal SC-001; no installed policy changes; missing/stale Route reference detected and refreshed; unrelated files and symlink safety preserved.
- Verification: cargo fmt --all --check; relevant tests then cargo test --all-targets --locked; independent review.
