---
schema_version: 1
id: WRK-20260817T095130-001-plan-lint-structural-che
type: work
title: plan-lint-structural-checks
status: in-progress
created_at: 2026-08-17T09:51:30+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260817T093100-001-agent-consumable-plan-en#t-002
- relation: fulfills
  id: GOAL-20260817T093055-001-agent-consumable-plan-en#sc-004
metadata: {}
---

- Added `src/plan.rs` with `belay plan lint`, mirroring `goal lint`'s shape:
  report struct, findings with layer/field/line, human and JSON renderers,
  `--all`, `--strict`, and the same non-strict exit behaviour.
- Seven checks: task rows exist, a `Goal item` column exists, task IDs are
  canonical and unique (delegated to `trace_ids::plan_id_findings`, so an ID
  that lints also resolves as `#t-nnn`), states come from the fixed set, every
  task ID has a `## T-NNN` section, that section carries the baseline fields,
  and links resolve.
- Baseline fields are Objective, Scope, Steps, Acceptance, Verification —
  the set proposed in the approved Intent Brief. Unknown fields are ignored,
  verified by a test that passes a section carrying Difficulty and Owner.
- A field counts as present via `**Field**` or `Field:`, so the check does not
  impose one Markdown style.
- The checklist counts failed checks, not failed items; six tasks each missing
  a section is one failed check, not six.
- A Plan with no Delivery Map is skipped rather than failed, so entries written
  before the convention stay lintable.
- Refactored `trace_ids`: one Delivery Map parser now, `delivery_map_rows`,
  exposing columns by lowercased heading. `delivery_map_id_rows` derives from
  it. A consumer can read a column belay does not mandate without a second
  table parser existing anywhere.
- T-003 landed in this same unit rather than after it: the interface parity was
  one edit with the command wiring, and splitting it would have meant shipping
  a subcommand without `--all`.
- Observed on the existing corpus: every historical Plan reports missing task
  sections. That is the honest signal — those Plans genuinely lack task detail
  — and it matches how `goal lint` reports missing Risks on older Goals.
- Diff locator: jj change wzzsyspq, parent e4abaa1b.
