---
schema_version: 1
id: GOAL-20260826T213655-001-orphan-tolerant-work-ret
type: goal
title: orphan-tolerant-work-return
status: archived
created_at: 2026-08-26T21:36:55+09:00
updated_at: 2026-08-26T21:39:55+09:00
revision: 3
tags: []
links: []
metadata: {}
---

## Goal

### Desired outcome
- Herdr implementation rounds adopt exactly one Work with exact `implements` to the current Task; unlinked placeholder Works do not fail or block that adoption.
- Implementer return contract uses atomic `belay work create --task <Task>` once per round (Belay ships the CLI; erwin documents and expects it).

### Success criteria
- SC-001: `Outbound Links:` followed by indented `none` parses as an empty link list (not Failure).
- SC-002: `collect_return` keeps only exact `implements <current-task>` Works from the round; ignores unlinked new Works; raises an explicit duplicate error when two or more exact matches exist; zero matches yield empty work (no-record path).
- SC-003: Implementer `return_contract` instructs a single `belay work create --task <Task> --title ... --body ...` and forbids nested `belay add work` / placeholder Works / separate link-only create patterns in that duty text.

### Constraints
- Edit staging `agent-safety/` only (herdr-routing runner, tests, skill text as needed). Do not install into `.agent-safety/`.
- Do not implement Belay's `work create` CLI here (Belay owns that).
- Do not weaken fail-closed parsing for ambiguous non-`none` link displays.

### Non-goals
- Changing Reviewer return contract (`add review` + `link`).
- Auto-deleting orphan Works.
- Softening malformed/ambiguous link lines other than exact `none`.

### Assumptions / Unknowns
- Assumption: Belay will ship `belay work create --task ...` with creates+links semantics; until then live workers need that Belay version.
- Assumption: Indented `none` under Outbound Links is Belay's canonical empty-links display.
- Unknown: None identified
