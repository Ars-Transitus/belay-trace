---
schema_version: 1
id: PLN-20260820T214216-001-retrieval-hygiene-archiv
type: plan
title: retrieval-hygiene-archive
status: approved
created_at: 2026-08-20T21:42:16+09:00
updated_at: 2026-08-20T21:57:17+09:00
revision: 4
tags: []
links:
- relation: implements
  id: GOAL-20260820T214155-001-retrieval-hygiene-archiv
metadata: {}
---

## Intent Brief

### Problem

- Trace history pollutes `context compile`, IDs must be fully canonical, compile always needs a query, and a task read still costs a whole Plan plus a missed Intent Brief. Light models leave Unknowns in the Map.

### Desired Outcome

- Archived entries drop out of default search and compile. Unique prefix/slug IDs work. No-arg compile is the live working set with a Next index. `--focus` returns a task packet. The Skill states the Frame/Map quality bar.

### Success Signals

- Incomplete unique IDs resolve; ambiguous ones fail with candidates.
- Archived Goal/Plan/Decision/Work/Review/Note are hidden from default search/compile and visible with an opt-in.
- `belay context compile` with no task lists live work and the next Delivery Map row.
- `--focus PLN-...#t-001` is smaller than `show` of the Plan and still carries Constraints and Non-goals.
- Generated Skill documents the new retrieval forms.

### Constraints

- Deterministic core; no LLM in belay.
- No fragment-standard, Route, or FTS-ranking change.
- No silent FTS fallback for ID resolution.
- Do not edit `.agent-safety/`.

### Non-goals

- Auto archive apply. Embeddings. `belay task`. Model names in belay. doctor overload. `plan improve`.

### Assumptions

- Human accepted the overlay-all-types archive model and working-set compile (not a separate next-task command).
- `completed` stays in default compile until archived.

### Unknowns / Decisions Needed

- None identified

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Unique prefix and slug ID resolution | AI | verified | cargo test |
| T-002 | SC-002 | Archived overlay and default exclude | AI | verified | cargo test |
| T-003 | SC-003 | archive candidates command | AI | verified | cargo test |
| T-004 | SC-004 | Working-set compile and Next index | AI | verified | cargo test |
| T-005 | SC-005 | context compile --focus task packet | AI | verified | cargo test |
| T-006 | SC-006 | Generated Skill retrieval and quality bar | AI | verified | cargo test |

## T-001

- **Objective**: Resolve unique display-id prefixes and slugs for CLI-facing show, status, link, lint, and context seed/focus.
- **Scope**: in — store resolve helper, clap ID args. out — Route stored IDs, FTS fallback.
- **Steps**: split optional fragment; exact match; unique prefix; unique slug; echo canonical; fail 0 or 2+.
- **Acceptance**: SC-001.
- **Verification**: unit tests plus CLI `show`/`status` with prefix and slug.

## T-002

- **Objective**: Allow `archived` on every entry type and hide it from default search and context.
- **Scope**: in — allows_status, search/context exclude, `--include-archived`. out — Route, auto-apply.
- **Steps**: extend allowlists; inject exclude unless include-archived or `--status archived`; keep `show` visible.
- **Acceptance**: SC-002.
- **Verification**: status transition tests; search omits archived by default.

## T-003

- **Objective**: List deterministic archive candidates without changing status.
- **Scope**: in — `belay archive candidates`. out — LLM judgment, doctor.
- **Steps**: terminal status, no live inbound link, superseded old side; print reason codes.
- **Acceptance**: SC-003.
- **Verification**: CLI fixture with completed unlinked vs live-linked entries.

## T-004

- **Objective**: No-arg context / context compile builds the live working set plus Next index.
- **Scope**: in — parse_context_task empty, live seeds. out — BM25 over all history as the primary seed.
- **Steps**: seed live statuses; Next from in-progress else first not-started; exclude archived.
- **Acceptance**: SC-004.
- **Verification**: CLI no-arg compile contains live plan task and omits archived.

## T-005

- **Objective**: `--focus` compiles a task packet: Brief slices, task, SC, recent Evidence.
- **Scope**: in — compile --focus. out — whole-plan dump.
- **Steps**: resolve focus; extract Intent Brief sections; task fragment; mapped Goal item; evidence status.
- **Acceptance**: SC-005.
- **Verification**: packet contains Constraints and the task, not sibling tasks.

## T-006

- **Objective**: Update SHARED_SKILL command reference, token discipline, archive workflow, and Frame/Map quality bar.
- **Scope**: in — src/agent.rs SHARED_SKILL, README. out — `.agent-safety/` edits.
- **Steps**: document no-arg compile, --focus, unique IDs, archive candidates, stop on open Unknowns.
- **Acceptance**: SC-006.
- **Verification**: `belay init` generated skill contains the new forms.
