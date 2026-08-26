---
schema_version: 1
id: GOAL-20260817T185935-001-library-status-include-e
type: goal
title: library-status-include-exclude
status: active
created_at: 2026-08-17T18:59:35+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Library status search is a single include. Operators cannot hide completed or
  abandoned entries, and four valid statuses are missing from the dropdown.
- Make status filtering an include set or an exclude set, shared by CLI search
  and Browse Library, with every `EntryStatus` selectable.

## Success Criteria

- [SC-001] `belay search` accepts repeatable `--status` (include) and
  `--exclude-status`. Results keep entries whose status is in the include set
  when that set is non-empty, and drop entries whose status is in the exclude
  set.
- [SC-002] A request that names the same status in both include and exclude
  fails with the documented validation exit status and does not return a
  partial result set.
- [SC-003] Library exposes grouped Include and Exclude status dropdowns for
  all thirteen statuses. The form is shareable as a GET query. A legacy
  `?status=accepted` URL still means include that one status. Browse does not
  offer a tag filter.
- [SC-004] CLI and Browse use the same `SearchRequest` include/exclude
  semantics, including empty mode (no status filter) and FTS plus structured
  combinations.
- [SC-005] README and `belay search --help` document the new flags. Existing
  single `--status accepted` invocations keep working.

## Constraints

- Reuse the existing search connection path. Do not add a Browse-only filter
  that can diverge from CLI results.
- No SQLite/Markdown schema migration.
- Browse stays read-only localhost HTML. No live reload, auth, or saved
  presets.
- Single `--status` remains a valid include of one value.

## Non-goals

- Multi-select type or tag filters.
- Mixing include and exclude in the Library form (CLI may pass both lists).
- Changing FTS ranking, `context compile`, export filters, or Route.
- Status transitions or editing from Browse.

## Verification

- Unit tests for include, exclude, overlap rejection, and empty filter.
- CLI test for repeatable `--status` and `--exclude-status`.
- Browse test that the Library form lists all statuses and honors
  `status_mode=exclude`.
- `cargo test` covering search, browse, and CLI.

## Risks

- Dynamic `IN` / `NOT IN` SQL must keep LIMIT applied after the status
  predicate, or structured searches will under-return.
- Over-broad exclude with no other filter is a valid structured search and
  must not be rejected as an empty query.
