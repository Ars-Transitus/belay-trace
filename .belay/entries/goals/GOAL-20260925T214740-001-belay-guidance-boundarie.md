---
schema_version: 1
id: GOAL-20260925T214740-001-belay-guidance-boundarie
type: goal
title: belay-guidance-boundaries
status: active
created_at: 2026-09-25T21:47:40+09:00
updated_at: 2026-09-26T00:46:47+09:00
revision: 5
tags: []
links: []
metadata: {}
---

## Summary

- Keep Belay guidance focused on trace contracts and load Route details only when needed.

## Success Criteria

- [SC-001] Generic skill defers review and human acceptance requirements to consumer policy and Goal, supports concise Intent Briefs without empty placeholders, and ships a discoverable Route reference through generation, installation and doctor checks with existing safety and trace contracts preserved.

## Constraints

- Preserve ID, Task/SC, Evidence, conflict safety, Route approval and native permissions. Preserve existing user changes.

## Non-goals

- Erwin, model routing, AGENTS/CLAUDE policy, installed consumer settings, schema/coverage semantics, push or deployment.

## Verification

- Generation, installation and doctor regression tests, Rust checks, independent review.

## Risks

- Reference lifecycle could miss stale or unsafe paths; preserve existing safe-write helpers and test these cases.
