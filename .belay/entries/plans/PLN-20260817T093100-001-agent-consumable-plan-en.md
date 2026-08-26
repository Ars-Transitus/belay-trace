---
schema_version: 1
id: PLN-20260817T093100-001-agent-consumable-plan-en
type: plan
title: agent-consumable-plan-entries
status: approved
created_at: 2026-08-17T09:31:00+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: GOAL-20260817T093055-001-agent-consumable-plan-en
metadata: {}
---

## Intent Brief

### Problem

- A consumer can reference `PLN-...#t-001` but cannot read it. `belay show`
  takes an entry ID only and prints the complete entry, so retrieving one task
  costs the whole Plan. Measured on a real 10-task Plan: the entry is about
  5,280 estimated tokens and the single task section is 438 — a factor of
  twelve paid on every retrieval.
- `goal lint` checks a Goal's structure, but nothing checks a Plan's. A
  Delivery Map row can name a task that no part of the document explains, and
  the gap surfaces only when a reader is already confused.
- What belay guarantees when several agents write concurrently is unstated, so
  consumers write defensive rules around a behavior nobody has characterized.

### Desired Outcome

- A fragment is readable, not merely referenceable. `belay show <id>#t-001`
  returns that task's defining row and its body section.
- `plan lint` gives a Plan the same deterministic structural check `goal lint`
  gives a Goal, including that every task ID has a body section.
- The concurrency contract is written down, whether it is a guarantee or a
  limitation.

### Success Signals

- Retrieving one task costs section-sized tokens rather than entry-sized, shown
  by comparing the two `show` forms on the same entry.
- `plan lint` fails a Plan whose Delivery Map names a task with no section, and
  passes one where every task has a conforming section.
- An unknown extra field in a task section does not fail the lint.
- A consumer can state its concurrency assumption by citing belay's
  documentation rather than guessing.

### Constraints

- The fragment standard in `docs/id-reference-standard.md` is fixed: lowercase
  canonical form, document-local `SC-NNN` / `T-NNN`, uniqueness, legacy forms
  rejected.
- The Delivery Map row stays the sole definition site of a task fragment.
  `plan_fragments` continues to read only `delivery_map_ids`; headings must not
  become a second definition site, or existing uniqueness checks break.
- `belay show <ENTRY-ID>` without a fragment keeps its current contract.
- Prefer no schema migration; `entry_chunks` already stores `section`,
  `ordinal`, and `token_estimate`.
- Belay stays product-agnostic. No routing, difficulty, or agent-orchestration
  concept enters the codebase.
- Tier 3 human gates apply. This Plan does not authorize implementation, `jj
  new`, issue creation, or a pull request.

### Non-goals

- A general-purpose Markdown section query language.
- Fragment retrieval for entry types with no fragment standard.
- Mandating Delivery Map columns or a consumer's task-section extensions.
- Changing FTS ranking, `context compile` selection, or Route.

### Assumptions

- `entry_chunks.section` holds the heading text verbatim, so a `## T-001`
  heading yields section `T-001`. Verified by querying a real repository:
  sections `T-001` through `T-010` are present with individual token estimates.
- Matching a lowercase canonical fragment against a section heading with an
  ASCII case-insensitive comparison is sufficient; no slug normalization beyond
  case is needed for `SC-NNN` / `T-NNN`.
- `goal lint`'s command shape, exit-status behavior, and `--all` / `--format
  json` interface are a suitable template for `plan lint`.

### Unknowns / Decisions Needed

- Decision needed from the human: the baseline task-section field set that
  `plan lint` requires. The proposal is a minimal generic five — Objective,
  Scope, Steps, Acceptance, Verification — with unknown fields tolerated so a
  consumer can extend it. Anything more starts encoding one consumer's
  workflow into a general trace store.
- Unknown: whether concurrent `add` is already safe. SQLite carries a 5-second
  busy timeout, but the managed Markdown write and the sync baseline are
  separate steps after the transaction. T-005 characterizes this before
  anything is claimed; the outcome may be a documented limitation.
- Unknown: the exact output shape of fragment-scoped `show`. It becomes a
  public contract on release, so T-001 proposes a shape and the Tier 3 review
  is the gate on it.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001, SC-002, SC-003 | Fragment-scoped `belay show` over existing chunks | agent | verified | EVD-20260817T094351-001 (cargo test, 7 new cases); EVD-20260817T094410-001 (6x/8x output reduction) |
| T-002 | SC-004 | `belay plan lint` structural checks | agent | verified | EVD-20260817T095059-001 (cargo test); EVD-20260817T095059-002 (clippy, fmt) |
| T-003 | SC-005 | `--all` and `--format json` parity, tolerated unknown fields | agent | verified | EVD-20260817T095059-001; delivered in the same unit as T-002 rather than after it |
| T-004 | SC-006, SC-007 | Document the retrievable task section and teach it in the generated skill | agent | verified | EVD-20260817T095904-001 (tests, skill artifacts active) |
| T-005 | SC-008 | Characterize and document concurrent writer behavior | agent | verified | EVD-20260817T095904-002 (8-way add; 4 link + 1 status contended) |
| T-006 | SC-001..SC-008 | Independent Tier 3 review | reviewer | not-started | Review entry with severity-ranked findings |
| T-007 | none — discovered defect | Stop `sync` rejecting a link whose target carries a fragment | agent | verified | EVD-20260817T095436-001; the regression test fails without the fix |

T-007 has no Goal item. It was discovered while implementing T-002, not
planned: recording a Work entry that links to `PLN-...#t-002`, exactly as
`docs/id-reference-standard.md` §3 prescribes, left `belay sync` unable to
reconcile. It is kept in the map rather than absorbed into T-002.

## T-001

- **Objective**: Make `belay show <ENTRY-ID>#<fragment>` return one item's
  content instead of the complete entry.
- **Scope**: in — `src/cli.rs` show handling, fragment resolution in
  `src/trace_ids.rs`, chunk lookup against `entry_chunks`. Out — the fragment
  standard, `plan_fragments` definition sites, `show` without a fragment,
  `context`, `search`, Route.
- **Inputs**: `docs/id-reference-standard.md` sections 1 and 4;
  `src/trace_ids.rs` `fragment_exists` / `local_fragments` /
  `valid_reference_fragment`; `src/database.rs` `entry_chunks` schema;
  `src/markdown.rs` `generate_chunks`.
- **Steps**:
  1. Accept an optional `#<fragment>` suffix on the `show` ID argument. Parse it
     with the existing reference parser rather than a new one.
  2. Validate the fragment with the existing rules. Non-canonical, unknown, or
     non-uniquely-resolving fragments exit with the documented invalid-display-ID
     status. Never fall back to whole-entry output.
  3. Print the defining item — the Delivery Map row for `#t-nnn`, the Success
     Criterion list item for `#sc-nnn` — followed by the body chunk whose
     `section` matches the fragment ID, compared ASCII case-insensitively.
  4. When the fragment resolves but no matching section chunk exists, print the
     defining item and state the missing-section gap explicitly. Do not print an
     empty body and do not error; the row is legitimately all there is.
  5. Keep the managed source path in the output, as whole-entry `show` does, so
     a consumer can still fall back to a bounded file read.
- **Acceptance**: SC-001, SC-002, SC-003. Retrieving a task returns
  section-sized output; every invalid fragment path is a clean failure; a
  section-less task reports the gap.
- **Verification**: `cargo test`; then `belay show` on a real Plan with and
  without a fragment, recording both token estimates as Evidence.
- **Assumption latitude**: output formatting and internal structure are free.
  Changing what defines a fragment, or making an invalid fragment degrade to
  whole-entry output, is material — return it.

## T-002

- **Objective**: Add `belay plan lint <plan-id>` as the deterministic
  structural sibling of `belay goal lint`.
- **Scope**: in — a plan lint module alongside `src/goal.rs`, its CLI wiring,
  and its checks. Out — semantic review, LLM calls, Goal lint behavior, and any
  consumer-specific field.
- **Inputs**: `src/goal.rs` for the lint pattern and checklist output;
  `src/cli.rs` goal subcommand wiring; `docs/id-reference-standard.md` section 2
  for the Delivery Map form; the task-state list in the generated skill.
- **Steps**:
  1. Check the Plan has a Delivery Map with the ID and Goal item columns.
  2. Check every task ID is canonical `T-NNN`, unique, and monotonic from
     `T-001`, reusing `trace_ids` rather than a second parser.
  3. Check every task state is drawn from the allowed set.
  4. Check every Delivery Map task ID has a body section whose heading matches
     it, and report each missing section with its task ID.
  5. Check the baseline fields are present in each task section. The field set
     is the open decision in the Intent Brief — do not choose it unilaterally.
  6. Report every violation, not only the first, in `goal lint`'s checklist
     style with an `n/m passed` summary.
  7. Skip, rather than fail, a Plan with no Delivery Map, so historical entries
     stay lintable without rewriting them.
- **Acceptance**: SC-004. Positive and negative fixtures behave as specified;
  all violations reported; historical Plans skip cleanly.
- **Verification**: `cargo test` with fixtures covering each check, plus a run
  against this repository's existing Plans.
- **Assumption latitude**: check ordering, module layout, and message wording
  are free. The baseline field set is a human decision — return it. Making a
  heading define a fragment is out of bounds.

## T-003

- **Objective**: Give `plan lint` the same interface shape as `goal lint` and
  keep it open to consumer extension.
- **Scope**: in — `--all` and `--format json` for plan lint, and the tolerated
  unknown-field rule. Out — changing `goal lint`'s interface.
- **Inputs**: `src/goal.rs` and `src/cli.rs` for the existing `--all` and
  `--format json` behavior and exit statuses; `PLN-...#t-002`.
- **Steps**:
  1. Implement `--all` over every Plan entry with the same aggregation and exit
     behavior `goal lint --all` uses.
  2. Implement `--format json` with a structurally parallel shape, so a consumer
     that already parses goal lint output needs no second parser.
  3. Ignore unrecognized fields inside a task section. An extension field is not
     a finding.
  4. Keep the non-strict exit behavior: lint findings do not make the command
     fail, matching the documented status table.
- **Acceptance**: SC-005. Interface parity holds; an extension field passes.
- **Verification**: `cargo test` comparing the two commands' interface behavior.
- **Assumption latitude**: JSON field names are free provided the structure
  parallels goal lint. Diverging exit-status semantics is material — return it.

## T-004

- **Objective**: Document the per-task body section as the retrievable form of a
  task fragment, and teach retrieval in the generated skill.
- **Scope**: in — `docs/id-reference-standard.md` and the generated agent skill
  source. Out — the fragment standard itself, and any consumer's field
  extensions.
- **Inputs**: `docs/id-reference-standard.md` sections 1, 2, and 4;
  `src/agent.rs` for skill generation; `PLN-...#t-001`, `PLN-...#t-002`.
- **Steps**:
  1. In section 2, show the Delivery Map row and its matching `## T-001` section
     together, and state plainly that the row defines the fragment while the
     section supplies its retrievable content.
  2. In section 4, note that a task fragment resolving to a row with no section
     is a `plan lint` finding, not a fragment-resolution failure.
  3. In the generated skill, teach `belay show <id>#t-nnn` as the way to read
     one task, and add `plan lint` to the command reference next to `goal lint`.
  4. Add one line to the skill's token discipline: retrieve a fragment, not an
     entry, when only one task is needed. Pair it with the caution that a task
     read in isolation still needs the Plan's constraints and non-goals — cheap
     retrieval is not a licence to skip the Intent Brief.
  5. Regenerate the skill artifacts so `belay doctor` reports them fresh.
- **Acceptance**: SC-006, SC-007. Both documents describe the row/section split;
  the skill teaches fragment retrieval with its caveat.
- **Verification**: `belay doctor` skill freshness checks; documentation review
  in T-006.
- **Assumption latitude**: wording and placement are free. Restating the
  fragment standard differently from section 1 is material — return it.

## T-005

- **Objective**: Establish what belay actually does when two processes write at
  once, and write the answer down.
- **Scope**: in — a concurrency characterization test over `add`, `link`, and
  `status`, and the resulting documentation. Out — designing a locking scheme
  before the behavior is known; that would be a separate Goal.
- **Inputs**: `src/database.rs` connection setup, including the 5-second busy
  timeout; `src/store.rs` and `src/reconcile.rs` for the ordering of the SQLite
  transaction, the managed Markdown write, and the sync baseline.
- **Steps**:
  1. Trace the write path and record the exact ordering of the transaction, the
     mirror write, and the baseline record.
  2. Write a test that runs concurrent writers from separate processes and
     asserts what actually happens to SQLite state, the Markdown mirror, and the
     baseline.
  3. Record the observed behavior as a fact, separately from any interpretation.
  4. If the behavior is safe, document it as a guarantee with its bounds. If it
     is not, document the limitation and the recommended usage pattern, and open
     a follow-up Goal rather than fixing it inside this Plan.
  5. Do not describe untested behavior as a guarantee.
- **Acceptance**: SC-008. A test characterizes the behavior; documentation
  states a guarantee or a limitation, matching what the test showed.
- **Verification**: `cargo test` concurrent writer case; the documented claim
  must cite it.
- **Assumption latitude**: test design is free. Asserting safety that the test
  did not demonstrate is out of bounds. Implementing a locking scheme under this
  Plan is out of scope — return it as a follow-up.

## T-006

- **Objective**: Independent Tier 3 review of the whole change.
- **Scope**: in — the diff, the Intent Brief, and the Evidence. Out —
  implementing fixes.
- **Inputs**: the approved Intent Brief, the Delivery Map, the diff locator, and
  the Evidence IDs.
- **Steps**:
  1. Resolve every pointer independently before forming an opinion.
  2. Judge the fragment-scoped `show` output as a public contract, since it
     becomes one on release.
  3. Check that the baseline task-section field set did not drift toward one
     consumer's workflow, and that unknown fields are genuinely tolerated.
  4. Check that no concurrency claim exceeds what the test demonstrated.
  5. Confirm `plan_fragments` still reads only the Delivery Map, and that
     whole-entry `show` is unchanged.
  6. Report severity-ranked findings with file and line evidence.
- **Acceptance**: a review entry exists with findings or an explicit statement
  that none remain.
- **Verification**: `belay show` on the review entry; the reviewer must not have
  implemented any part of the change.
- **Assumption latitude**: none beyond review judgment. Do not edit files.

## T-007

- **Objective**: Let `belay sync` reconcile an entry whose link target carries a
  `#sc-nnn` or `#t-nnn` fragment.
- **Scope**: in — the two link-target lookups in `src/reconcile.rs`. Out —
  fragment validation on store, which already works and is where a bad fragment
  should be caught.
- **Inputs**: `docs/id-reference-standard.md` §3, which prescribes Work linking
  to a Plan task with `implements`; `src/reconcile.rs`
  `missing_database_dependency_closure` and `validate_link_targets`; the
  observed failure `entry WRK-... links to missing entry PLN-...#t-002`.
- **Steps**:
  1. Split the fragment off a link target before looking it up in the entry
     inventory and database records, which are keyed by bare display ID.
  2. Keep the reported message showing the full reference, so a genuinely
     missing target still names what was written.
  3. Add a regression test that links Work to a Plan task and then syncs.
  4. Confirm the test fails with the fix reverted, so it guards the behavior
     rather than merely passing alongside it.
- **Acceptance**: a documented fragment link survives `sync`; the full suite
  stays green; a real missing target is still reported.
- **Verification**: `cargo test`; `belay sync` and `belay doctor` clean on this
  repository, which was left unsyncable by the defect.
- **Assumption latitude**: none. This is a defect fix outside the approved
  scope, taken because it blocked the approved work, and recorded as its own
  task rather than folded into T-002.
