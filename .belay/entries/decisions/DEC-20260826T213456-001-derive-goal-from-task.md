---
schema_version: 1
id: DEC-20260826T213456-001-derive-goal-from-task
type: decision
title: derive-goal-from-task
status: proposed
created_at: 2026-08-26T21:34:56+09:00
updated_at: 2026-08-26T21:35:02+09:00
revision: 3
tags: []
links:
- relation: supports
  id: GOAL-20260826T213421-001-atomic-work-creation
- relation: supports
  id: PLN-20260826T213447-001-atomic-work-creation
metadata: {}
---

## Decision

- Fact: A Plan Delivery Map Task identifies its Goal item, and the Plan links to the Goal.
- Human decision: `belay work create` accepts only the canonical full Plan Task reference; Goal input is not required.
- Decision: Belay resolves the Task row and materializes both `implements Plan#task` and direct `fulfills Goal#criterion` links in the new Work.
- Rationale: Preserve the current Work reference standard and direct Goal Coverage while removing the unsafe create-then-link gap.
- Constraint: Missing, malformed, non-canonical, or ambiguous Task-to-Goal mappings fail before Work insertion.
- Non-goal: Do not make indirect Task-to-Goal traversal the sole Coverage model in this change.
