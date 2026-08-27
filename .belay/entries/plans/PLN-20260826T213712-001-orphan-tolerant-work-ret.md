---
schema_version: 1
id: PLN-20260826T213712-001-orphan-tolerant-work-ret
type: plan
title: orphan-tolerant-work-return
status: archived
created_at: 2026-08-26T21:37:12+09:00
updated_at: 2026-08-26T21:39:55+09:00
revision: 4
tags: []
links:
- relation: fulfills
  id: GOAL-20260826T213655-001-orphan-tolerant-work-ret
metadata: {}
---

## Intent Brief

### Problem
- Workers sometimes create a valid Work plus an unlinked placeholder; the runner treated `Outbound Links: none` as a parse Failure and aborted the whole round, discarding the valid Work.
- The implementer return duty still taught `belay add work` then `belay link`, which invites double-create and orphan placeholders.

### Desired Outcome
- Empty outbound links display as empty; only exact `implements <Task>` Works from the round are adopted; duplicates error explicitly; orphans are ignored.
- Return contract tells the worker to call `belay work create --task <Task>` exactly once per round.

### Success Signals
- Three new unit fixtures: valid+orphan → adopt valid; two exact → duplicate Failure; `none` → empty list.
- `check_return_contract` asserts `belay work create --task` and does not require separate `belay add work` + `link` for implementation.
- `python3 agent-safety/herdr-routing/scripts/test_herdr_routing.py` passes.

### Constraints
- Staging `agent-safety/` only; Belay CLI implementation is out of scope; keep fail-closed for non-`none` ambiguous link lines.

### Non-goals
- Reviewer return path changes; deleting orphans; implementing `belay work create` in this repo.

### Assumptions
- Assumption: Belay will provide `belay work create --task` atomically; erwin only updates the contract text and runner tolerance.
- Assumption: Canonical empty display is indented `none` under `Outbound Links:`.

### Unknowns / Decisions Needed
- None identified

## Delivery Map
| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Treat Outbound Links `none` as empty list in `outbound_link_targets` | implementer | not-started | unit: none → [] |
| T-002 | SC-002 | `collect_return` ignores unlinked new Works; adopts exact implements; duplicate → Failure | implementer | not-started | unit: orphan+valid; two exact |
| T-003 | SC-003 | Implementer `return_contract` uses single `belay work create --task` | implementer | not-started | check_return_contract |

## T-001
- Objective: Parse Belay empty outbound-links display without Failure.
- Scope: `outbound_link_targets` in route_worker.py; tests.
- Steps: When collecting under Outbound Links, accept stripped `none` as empty (only when no links yet); keep rejecting other non-dash content.
- Acceptance: Fixture with `Outbound Links:\n  none\n` returns []; existing ambiguous cases still fail.
- Verification: python3 agent-safety/herdr-routing/scripts/test_herdr_routing.py

## T-002
- Objective: Orphan Works must not poison a round that also has one exact implements Work.
- Scope: `collect_return`; tests.
- Steps: Filter candidates with exact implements; if count > 1 raise explicit duplicate Failure; if 0 return empty work list.
- Acceptance: valid+none-orphan → [valid]; two exact → Failure mentioning duplicate; zero → [].
- Verification: same test file

## T-003
- Objective: Point workers at atomic Work+Task create.
- Scope: `return_contract` implementation branch; herdr-routing SKILL if it contradicts; tests.
- Steps: Replace add work + link with `belay work create --task <task> --title ... --body ...`; state once-per-round and forbid placeholders / nested add work.
- Acceptance: Prompt contains `belay work create` and `--task {task}`; does not teach separate implementation `belay add work` + `belay link ... implements`.
- Verification: check_return_contract
