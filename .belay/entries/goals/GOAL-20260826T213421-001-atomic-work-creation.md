---
schema_version: 1
id: GOAL-20260826T213421-001-atomic-work-creation
type: goal
title: atomic-work-creation
status: draft
created_at: 2026-08-26T21:34:21+09:00
updated_at: 2026-08-26T21:35:55+09:00
revision: 2
tags: []
links: []
metadata: {}
---

## Summary

- Provide a safe `belay work create --task <plan#task>` workflow that creates a trace-complete Work without an orphan intermediate state.

## Success Criteria

- [SC-001] A valid Plan task creates exactly one Work with its `implements` Task link and the derived Goal `fulfills` link in one mutation.
- [SC-002] Task-to-Goal derivation rejects missing, invalid, ambiguous, or non-canonical mappings before creating any Work.
- [SC-003] Machine callers can obtain the created Work ID without parsing human prose, while existing `add` and `link` commands remain compatible.
- [SC-004] Tests demonstrate failure non-creation, mirror/SQLite consistency, concurrent safety, and documented command behavior.

## Constraints

- Keep Belay deterministic and local-first; do not add an LLM or schema migration.
- Accept the canonical full Plan task reference, such as `PLN-...#t-001`.
- Preserve existing generic `belay add work` and `belay link` behavior.
- Treat SQLite transaction atomicity separately from the documented filesystem crash window.

## Non-goals

- Removing or redesigning generic `add` and `link`.
- Introducing a general task-management model or changing Plan fragment syntax.
- Making indirect Task-to-Goal traversal the sole replacement for current direct `fulfills` coverage.
- Adding external publication, issue, pull-request, or merge operations.

## Verification

- Focused integration tests for valid creation, invalid mappings, machine-readable output, and compatibility.
- Full Rust tests, `belay doctor`, and the safe-autonomy boundary fixture.
- Independent review of the final diff, trace links, and Evidence.

## Risks

- A Plan changed after Work creation can make a materialized direct Goal link stale; the new command validates the mapping at creation time and preserves the existing direct Coverage contract.
- SQLite and managed Markdown cannot form a single filesystem transaction across forced process termination; the existing recovery boundary remains documented.

## Assumptions / Unknowns

- Assumption: A Plan task has one unambiguous Goal item, resolved through the Plan's Goal link when the row uses a short `SC-NNN`.
- Assumption: A fully qualified Goal fragment in a Delivery Map row is authoritative when a Plan covers multiple Goals.
- Decision: The new workflow derives and materializes the direct Goal `fulfills` link so existing Coverage remains compatible.
- Unknown: None identified.
