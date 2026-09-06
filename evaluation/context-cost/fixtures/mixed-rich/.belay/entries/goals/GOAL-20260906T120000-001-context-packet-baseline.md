---
schema_version: 1
id: GOAL-20260906T120000-001-context-packet-baseline
type: goal
title: context-packet-baseline
status: active
created_at: 2026-09-06T12:00:00+09:00
updated_at: 2026-09-06T12:00:00+09:00
revision: 1
tags: [fixture, context-cost]
links: []
metadata: {}
---

## Summary
- Keep a context packet complete while reducing repeated material.

## Success Criteria
- [SC-001] A focused packet retains the required task and goal information.

## Constraints
- Keep canonical IDs, deterministic selection, and FTS ranking unchanged.
- 長い制約: 日本語の制約文を含め、利用者が追加の検索なしで安全に判断できる必要情報を保持する。The packet must retain the task Acceptance and Verification text even when the surrounding history contains repeated status reports and evidence summaries.

## Non-goals
- Do not estimate API cost or model-token usage.

## Assumptions / Unknowns
- Assumption: deterministic estimates are sufficient for this fixture.
- Unknown: production prompt-cache behavior is outside this measurement.
