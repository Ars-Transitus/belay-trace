---
schema_version: 1
id: PLN-20260817T190256-001-library-status-include-e
type: plan
title: library-status-include-exclude
status: approved
created_at: 2026-08-17T19:02:56+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: GOAL-20260817T185935-001-library-status-include-e
metadata: {}
---

## Intent Brief

### Problem

- Library and `belay search` accept one status equality filter. Hiding
  completed or abandoned work requires multiple searches.
- The Library dropdown omits `proposed`, `rejected`, `superseded`, and
  `abandoned`.

### Desired Outcome

- Status filters are an include set or an exclude set.
- CLI and Browse share one `SearchRequest` contract.
- Library offers Include / Exclude mode and a checklist of all thirteen
  statuses. Legacy `?status=` remains include-one.

### Success Signals

- `--status draft --status active` returns only those statuses.
- `--exclude-status completed` drops completed entries.
- Overlapping include and exclude is a validation error.
- Library HTML lists every `EntryStatus` and honors `status_mode=exclude`.

### Constraints

- Same search connection path for CLI and Browse.
- No schema migration. Browse stays read-only.
- Single `--status accepted` stays valid.

### Non-goals

- Type or tag multi-select.
- Mixing include and exclude in the Library form.
- Export filter changes, FTS ranking, Route, or Browse edits.

### Assumptions

- Mode plus checklist is enough for Library; CLI may pass both lists.
- Empty include and empty exclude means no status filter.

### Unknowns / Decisions Needed

- None identified. Human chose mode-plus-checklist and shared CLI/Browse
  semantics.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001, SC-002, SC-004 | Extend `SearchRequest` with include/exclude status lists and SQL predicates | agent | verified | EVD-20260817T193330-001; search unit tests |
| T-002 | SC-001, SC-005 | Repeatable CLI `--status` and `--exclude-status`; help and README | agent | verified | EVD-20260817T193330-001; CLI search test |
| T-003 | SC-003, SC-004 | Library Include/Exclude mode and thirteen-status checklist | agent | verified | EVD-20260817T193330-001; Browse unit test |
| T-004 | SC-001..SC-005 | Record verification Evidence | agent | verified | EVD-20260817T193330-001 |
| T-005 | SC-003 | Library grouped include/exclude dropdowns; drop tag filter from Browse | agent | verified | EVD-20260817T194642-001 |

## T-001

- **Objective**: Replace single `Option<EntryStatus>` with include and exclude
  lists in the shared search contract.
- **Scope**: in — `src/search.rs` and call sites that construct
  `SearchRequest`. Out — export filters, context ranking.
- **Steps**: Add `status_include` / `status_exclude`. Deduplicate. Reject
  overlap. Apply `IN` / `NOT IN` before LIMIT in exact, structured, and
  keyword queries. Empty lists mean no status predicate.
- **Acceptance**: Include, exclude, overlap, and empty-filter cases pass.
- **Verification**: `cargo test --lib search`.

## T-002

- **Objective**: Expose the contract on the CLI without breaking one
  `--status`.
- **Scope**: in — `src/cli.rs` SearchArgs, help examples, README. Out —
  export `--status`.
- **Steps**: Collect repeatable `--status` and `--exclude-status`. Print both
  lists. Document examples.
- **Acceptance**: Existing `--status proposed` still works; new flags filter
  as specified.
- **Verification**: CLI search test plus `--help` string check.

## T-003

- **Objective**: Library UI for include/exclude mode and all statuses.
- **Scope**: in — `src/browse.rs` library form, query parsing, CSS. Out —
  type/tag multi-select, JS-only filtering.
- **Steps**: Replace the status `<select>` with radios and checkboxes. Parse
  repeated `status` plus `status_mode`. Default mode is include so legacy
  URLs keep working.
- **Acceptance**: All thirteen statuses render. Exclude mode hides matching
  rows. Missing checkboxes means no status filter.
- **Verification**: Browse unit test against rendered HTML and search results.

## T-004

- **Objective**: Attach fresh Evidence to the Goal after tests pass.
- **Scope**: in — `belay verify record`. Out — release tagging.
- **Steps**: Run targeted then broader tests. Record pass Evidence verifying
  the Goal.
- **Acceptance**: Evidence exists and `belay coverage` sees the Goal as
  having verification.
- **Verification**: `belay verify status` on the Goal.

## T-005

- **Objective**: Replace the Library checklist with grouped include/exclude
  dropdowns and remove the unused tag search field from Browse.
- **Scope**: in — Library form, query parsing, CSS. Out — CLI `--tag`,
  tag storage, search contract.
- **Steps**: Two `<select>`s with Open/Active/Resolved/Closed optgroups.
  Ignore `tag` on `/`. Keep `?status=` as include-one.
- **Acceptance**: No tag input. All thirteen statuses appear in groups.
  `status_exclude=proposed` hides proposed entries.
- **Verification**: `cargo test --lib browse::tests::library_status_filter`.
