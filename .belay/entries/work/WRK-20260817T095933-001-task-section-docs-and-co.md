---
schema_version: 1
id: WRK-20260817T095933-001-task-section-docs-and-co
type: work
title: task-section-docs-and-concurrency
status: in-progress
created_at: 2026-08-17T09:59:33+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260817T093100-001-agent-consumable-plan-en#t-004
- relation: fulfills
  id: GOAL-20260817T093055-001-agent-consumable-plan-en#sc-008
metadata: {}
---

- T-004: `docs/id-reference-standard.md` gained §2.1 (the task body section, and
  that a heading does not define a fragment), a `plan lint` paragraph in §4, and
  §5 on retrieval. The generated skill teaches `belay show <id>#t-nnn`,
  `belay plan lint`, and a new Map step 7 requiring a `## T-NNN` section per
  task. Skill artifacts regenerated and reinstalled; `doctor` reports both
  active.
- The skill's retrieval guidance carries its own counterweight: cheap retrieval
  is not licence to skip the Intent Brief, because a task read alone loses the
  Constraints and Non-goals that make it correct. Without that line the feature
  would encourage worse work while costing fewer tokens.
- T-005: the concurrency question resolved to a guarantee, not a limitation.
  Traced the write path — `create` opens an IMMEDIATE transaction, writes the
  Markdown mirror *inside* it, then commits — so writers serialize rather than
  interleave, and `allocate_display_id` runs under the same lock.
- Two tests characterize it rather than assert it: 8 simultaneous `add`
  processes (all succeed, IDs unique, mirrors complete, no drift), and 4
  concurrent `link` plus 1 `status` against one entry (all succeed, no lost
  link, no drift).
- Documented the bounds honestly in §6, including the two things that are not
  guaranteed: the 5-second busy timeout turns extreme contention into an error,
  and a kill between the mirror write and the commit leaves a file the database
  does not know about until the next `sync`.
- Consequence for consumers: a single-writer rule is not required for
  correctness. erwin's precautionary constraint can be relaxed to a note about
  the busy timeout.
- Diff locator: jj change wzzsyspq, parent e4abaa1b.
