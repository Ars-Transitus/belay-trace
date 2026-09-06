---
schema_version: 1
id: PLN-20260906T120000-001-context-packet-baseline
type: plan
title: context-packet-baseline
status: active
created_at: 2026-09-06T12:00:00+09:00
updated_at: 2026-09-06T12:00:00+09:00
revision: 1
tags: [fixture, working-set]
links:
- relation: fulfills
  id: GOAL-20260906T120000-001-context-packet-baseline
metadata: {}
---

## Intent Brief
### Problem
- Repeated context can hide a task's decision-relevant fields.
### Desired Outcome
- A complete focused packet is available in one retrieval.
### Success Signals
- Required fields are present without an additional command.
### Constraints
- Preserve schema, canonical identifiers, deterministic ordering, and FTS ranking.
- 長い制約: fixtureは英日混在の説明、複数Evidence、重複したGoal、working setの複数Taskを含む。出力を短くする変更であっても、利用者が人間承認、Unknown、Acceptance、Verificationを再読込なしで判断できることを確認する。
### Non-goals
- No runtime mutation, model call, API price claim, archive, or installed configuration update.
### Assumptions
- The byte-and-scalar estimate is a comparison proxy, not a model tokenizer.
### Unknowns / Decisions Needed
- Unknown: whether a 256-token packet can contain every required item before compiler changes.
- Human decision: later tasks decide any output-contract change.

## Delivery Map
| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Measure focused baseline | implement_medium | in-progress | fixture packet |
| T-002 | SC-001 | Remove duplicate context | implement_high | not-started | regression tests |
| T-003 | SC-001 | Preserve low-budget fields | implement_high | not-started | focused review |

## T-001
- Objective: measure the packet before compiler changes.
- Scope: fixture and measurement only; runtime is excluded.
- Steps: run the fixed focus command once.
- Acceptance: preserve every required packet field or record its absence deterministically.
- Verification: compare bytes, estimate, required strings, and retrieval count.

## T-002
- Objective: a sibling working-set task that must not enter the focused section.
- Scope: future compiler implementation.
- Steps: inspect duplicate Goal material.
- Acceptance: no ranking change.
- Verification: focused regression tests.

## T-003
- Objective: preserve Unknowns at low budget.
- Scope: future compiler implementation.
- Steps: test overflow behavior.
- Acceptance: no silent omission.
- Verification: independent review.
