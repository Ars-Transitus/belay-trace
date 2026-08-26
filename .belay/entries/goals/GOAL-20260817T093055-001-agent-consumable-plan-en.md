---
schema_version: 1
id: GOAL-20260817T093055-001-agent-consumable-plan-en
type: goal
title: agent-consumable-plan-entries
status: active
created_at: 2026-08-17T09:30:55+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Belay already defines `PLN-...#t-001` and `GOAL-...#sc-001` as canonical
  fragments, validates their existence and uniqueness on link and Evidence
  targets, and reports fragment drift in `doctor`. It also already stores every
  body section as a chunk with its own token estimate. What is missing is the
  join between the two: a fragment can be referenced but not read, so a consumer
  that wants one task must retrieve the entire Plan.
- Close that gap. Make a fragment retrievable, lint a Plan's task sections the
  way `goal lint` lints a Goal's sections, and state what belay guarantees when
  several agents write to one repository at the same time.

## Success Criteria

- [SC-001] `belay show <ENTRY-ID>#<fragment>` resolves a canonical `#sc-nnn` or
  `#t-nnn` fragment and prints only that item — its defining Delivery Map row or
  Success Criterion item, plus the body section whose heading matches the ID —
  instead of the complete entry.
- [SC-002] Fragment resolution reuses the existing `trace_ids` rules. A
  non-canonical, unknown, or non-uniquely-resolving fragment fails with the
  documented invalid-display-ID exit status and never silently falls back to
  printing the whole entry.
- [SC-003] When a fragment resolves in the Delivery Map but no matching body
  section exists, the command says so explicitly instead of printing an empty
  body.
- [SC-004] `belay plan lint <plan-id>` performs deterministic structural checks
  on a Plan, mirroring `belay goal lint`: Delivery Map presence, canonical and
  unique `T-NNN`, task states drawn from the allowed set, a body section for
  every task ID, and the baseline task-section fields present.
- [SC-005] `plan lint` accepts `--all` and `--format json` with the same
  interface shape as `goal lint`, and unknown extra fields in a task section are
  accepted rather than reported, so a consumer may extend the section.
- [SC-006] `docs/id-reference-standard.md` documents the per-task body section
  as the retrievable form of a `#t-nnn` fragment, alongside the existing
  Delivery Map row that defines it.
- [SC-007] The generated agent skill teaches fragment-scoped `show` and
  `plan lint`, so an agent retrieves one task instead of one Plan.
- [SC-008] The behavior of concurrent `add`, `link`, and `status` from separate
  processes is characterized by a test and documented as either a guarantee or a
  stated limitation with a recommended usage pattern.

## Constraints

- Do not change the existing fragment standard. Lowercase canonical form,
  document-local `SC-NNN` / `T-NNN`, uniqueness, and the rejection of legacy
  forms all stay as `docs/id-reference-standard.md` specifies.
- `belay show <ENTRY-ID>` without a fragment keeps its current output contract.
- The Delivery Map row remains the sole definition of a task fragment. A body
  heading is retrievable content, not a second definition site, so headings must
  not start participating in `plan_fragments`.
- Prefer no schema migration: `entry_chunks` already carries `section`,
  `ordinal`, and `token_estimate`.
- Keep exit statuses consistent with the documented table.
- Belay lints structure, not any consumer's routing policy. The baseline field
  set stays minimal and generic.
- Tier 3 human gates apply: this plan does not authorize implementation.

## Non-goals

- A general-purpose Markdown section query language, or fragment retrieval for
  entry types that have no fragment standard.
- Mandating the Delivery Map column set. Consumers may add columns.
- Adopting any routing, difficulty, or agent-orchestration concept. Those belong
  to the consumer; belay stays product-agnostic.
- Changing FTS ranking, `context compile` selection, or Route.

## Risks

- Scope creep from retrieval into a query language. The fragment standard is
  narrow on purpose; widening `show` into arbitrary section addressing would
  make the output contract unstable for every existing consumer.
- Over-specifying the task section. If belay's baseline field set is drawn too
  tightly around one consumer's workflow, it stops being a general trace store
  and every other user fights the linter. The mitigation is a minimal baseline
  plus tolerated unknown fields, and it needs review rather than assertion.
- A retrievable fragment invites a consumer to skip the Intent Brief and read
  only its own task, losing the constraints and non-goals that make the task
  meaningful. Retrieval granularity is a cost tool, not a license to drop
  context; the skill wording matters here.
- The concurrency question may resolve into real work. SQLite carries a 5-second
  busy timeout, but the Markdown mirror write and sync baseline are separate
  steps, so the honest outcome may be a documented limitation rather than a
  guarantee.
- Fragment-scoped output becomes a public contract the moment it ships, and a
  consumer will parse it. Its shape deserves the same care as any CLI surface.

## Verification

- `cargo test` for fragment resolution, lint checks, and the concurrency
  characterization.
- `belay show` on a real Plan carrying task sections, compared against the same
  entry without a fragment, recording both token estimates.
- `belay doctor` reports no new drift class on an existing repository.
- Independent Tier 3 review.
