---
schema_version: 1
id: WRK-20260817T094502-001-fragment-scoped-show
type: work
title: fragment-scoped-show
status: in-progress
created_at: 2026-08-17T09:45:02+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260817T093100-001-agent-consumable-plan-en#t-001
- relation: fulfills
  id: GOAL-20260817T093055-001-agent-consumable-plan-en#sc-001
metadata: {}
---

- Added `fragment_definition` to `src/trace_ids.rs`: the Delivery Map row for a
  Plan task, the Success Criteria item for a Goal criterion. Goal items keep
  their continuation lines, because a truncated criterion reads as a whole one.
- `store::show` now takes a reference rather than a display ID. With a fragment
  it reuses `validate_reference_fragment`, so a fragment means the same thing
  when read as when stored: canonical, present, unambiguous.
- Section content comes from `entry_chunks`, which belay already generates, so
  sectioning agrees with search and context instead of being reparsed.
- `Show` got its own `ShowArgs`; `EntryIdArgs` is shared with Route status,
  which does not accept fragments.
- The fragment view omits tags, metadata, links, and the full body — they would
  reintroduce the cost the fragment exists to avoid. `Source` stays so a reader
  can still fall back to the file.
- Diff locator: jj change `wzzsyspq`, parent `e4abaa1b`.
- Assumption held: no schema migration was needed. `entry_chunks.section`
  carries the heading verbatim.
- Not done here: filtering inbound links by `to_fragment` would make this a task
  view rather than a text excerpt. Outside T-001's stated scope; recorded as a
  follow-up candidate rather than absorbed silently.
