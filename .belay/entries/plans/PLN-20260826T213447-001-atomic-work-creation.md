---
schema_version: 1
id: PLN-20260826T213447-001-atomic-work-creation
type: plan
title: atomic-work-creation
status: draft
created_at: 2026-08-26T21:34:47+09:00
updated_at: 2026-08-26T21:57:48+09:00
revision: 6
tags: []
links:
- relation: fulfills
  id: GOAL-20260826T213421-001-atomic-work-creation
metadata: {}
---

## Intent Brief

### Problem
- Agents can create a Work with generic `belay add work` before a later `belay link`; command substitution can also create a second Work and pass human-readable output as an ID.

### Desired Outcome
- Provide `belay work create --task` that derives the Goal criterion from the Plan Task and atomically creates the trace-complete Work.

### Success Signals
- Valid input produces one Work with both required links and invalid or ambiguous input produces no Work.
- The created mirror, SQLite state, and Coverage behavior remain consistent.
- Agents have an explicit machine-readable ID output mode.

### Constraints
- difficulty: medium
- Keep the existing generic `add` and `link` commands and current fragment standard.
- Use the existing SQLite IMMEDIATE transaction and managed Markdown mutation primitives.
- No schema migration, LLM, or external mutation.

### Non-goals
- Removing support for manually staged generic Work entries.
- Redesigning Goal Coverage to use only indirect graph traversal.
- Solving the filesystem crash window beyond the existing documented guarantee.

### Assumptions
- Assumption: the task argument is a canonical full Plan reference such as `PLN-...#t-001`.
- Assumption: a short Goal item is resolvable through exactly one Plan `fulfills` Goal link.
- Assumption: fully qualified Goal items are used when the Plan covers multiple Goals.

### Unknowns / Decisions Needed
- None identified; the human approved the Task-derived Goal design before implementation.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | Difficulty | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Implement atomic Work creation with derived links | AI | medium | verified | EVD-20260826T215625-001; WRK-20260826T215425-001-implement-task-derived-w |
| T-002 | SC-002 | Resolve and validate Task-to-Goal mappings | AI | medium | verified | EVD-20260826T215625-001 |
| T-003 | SC-003 | Add machine-readable Work ID output and documentation | AI | medium | verified | EVD-20260826T215625-001; EVD-20260826T215515-001 |
| T-004 | SC-004 | Verify failure rollback, consistency, concurrency, and boundary | AI | medium | in-progress | EVD-20260826T215630-001; boundary fixture unavailable in checkout; doctor has unrelated pre-existing drift |

## T-001
- Objective: Create one Work and its Task/Goal links through one store mutation.
- Scope: `src/store.rs` and `src/cli.rs`; preserve generic mutations.
- Steps: Add a Work-specific create primitive, validate targets before insert, insert links and mirror within the existing transaction, expose the subcommand.
- Acceptance: A valid invocation creates exactly one Work at revision 1 with `implements` and derived `fulfills` links.
- Verification: Focused integration tests, `show`/mirror inspection, and targeted lint; the repository doctor also ran with unrelated pre-existing drift.

## T-002
- Objective: Make Task-derived Goal resolution deterministic and fail closed.
- Scope: Plan fragment, Delivery Map `Goal item`, Plan Goal links, and canonical references.
- Steps: Resolve the Task row, interpret short or fully qualified Goal items, reject missing/ambiguous/malformed mappings.
- Acceptance: No Work or mirror is created for every invalid mapping case.
- Verification: Integration tests cover valid single-Goal derivation, invalid task references, ambiguous mappings, and fully qualified multi-Goal references.

## T-003
- Objective: Make the new workflow safe for shell and agent callers.
- Scope: Work create arguments, ID-only or JSON output, help text, and usage documentation.
- Steps: Add a machine-readable output option, keep human output compatible for existing commands, document the safe recipe and atomicity boundary.
- Acceptance: A caller can capture exactly one canonical ID without parsing `Created`; existing `add`/`link` tests remain passing.
- Verification: CLI parser, help, output, and compatibility tests.

## T-004
- Objective: Establish fresh evidence for the complete change.
- Scope: Tests and repository assurance only.
- Steps: Run focused and full Rust tests, formatting/clippy as available, doctor, boundary fixture, and inspect the final diff.
- Acceptance: All relevant tests pass; no new drift or safety boundary failure is introduced.
- Verification: Record passing test and boundary Evidence linked to this Goal item.
