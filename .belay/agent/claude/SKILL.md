---
name: belay-trace
description: Use for Tier 2 or Tier 3 coding work and for reading or updating Belay intent, plans, decisions, status, and Evidence through the local CLI.
---

# belay-trace

## Command reference

Use these forms directly. Do not run `--help` to discover syntax.

```sh
belay context compile --format agent --budget 4000            # live working set; no task
belay context compile "<task>" --format agent --budget 4000   # once per task, at start
belay context compile --focus <plan-id>#t-001 --format agent --budget 2500
belay context "<task>" --format agent --budget 2500           # fallback if compile is unavailable
belay search "<query>"                                        # targeted follow-up discovery
belay search --include-archived                               # include archived history
belay show <id>                                               # unique prefix or slug; full entry
belay show <plan-id>#t-001                                    # one task; prefer over the whole Plan
belay show <goal-id>#sc-001                                   # one Success Criterion
belay show EVD-<id>                                           # exact Evidence ID or unique prefix; NDJSON
belay archive candidates                                      # deterministic stale-history candidates
belay add <goal|plan|decision|work|review|note> --title "<short-en-slug>"
belay work create --task <plan-id>#t-001 --title "..." --body "..."
belay link <from-id> <to-id> --relation <rel>
    # rel: fulfills | supports | verifies | reviews | implements |
    #      references | supersedes | follows-up | refutes
belay status <id> <status>
belay goal lint <goal-id>
belay plan lint <plan-id>   # Delivery Map and task-section structure
belay verify record --kind <test|human-approval|...> --verdict <pass|fail> \
  --source "<command-or-url>" --summary "<what passed>" --verifies <id>
belay sync
belay rebuild                                                 # reports managed Markdown and Evidence counts
belay doctor        # when generated or active integration may be stale
belay coverage      # inspect Goal coverage before release decisions
```

## Route runs

Use Route only when the human asks to reconstruct one active decision thread
into typed Assessment and Proposal artifacts. Before a Route run, read
[the Route reference](references/route.md).

## Token discipline

- Entry titles must be short English kebab-case, at most 5 words
  (e.g. `t012-bigquery-dry-run`). The display ID embeds the title slug and is
  repeated throughout later context; never use Japanese or long phrases in a
  title. Put the descriptive detail in the entry body instead.
- Write entry bodies as terse bullets. Delete scaffold sections that would
  only say "None." Keep an Intent Brief's Problem, Desired Outcome, and
  Success Signals; include Constraints, Non-goals, material Assumptions, and
  Unknowns / Decisions Needed only when relevant. Goal lint is the exception:
  every Goal keeps its six required sections.
- Run `belay context compile` with no task to see the live working set and Next
  index, or once with a task at task start. For anything after that, use
  `belay search` or `--focus`; do not re-compile at checkpoints.
- Retrieve a fragment, not an entry, when you need one item. Prefer
  `belay context compile --focus <plan-id>#t-nnn` so Constraints and Non-goals
  travel with the task. `belay show <plan-id>#t-003` returns that task's
  Delivery Map row and its `## T-003` section; `belay show <plan-id>` returns
  every task in the Plan. On a ten-task Plan that is roughly an eighth of the
  output.
  `--focus` resolves exactly one Goal item from the task row. Each row must
  name a single `SC-NNN` or `GOAL-...#sc-nnn`; comma-separated Goal items, or a
  Plan linked to multiple Goals without fully qualified Goal items, make focus
  compile fail.
  Cheap retrieval is not licence to skip the Intent Brief: a task read alone
  loses the Constraints and Non-goals that make it correct, so read those too
  before acting, and read the whole entry when the work spans tasks.
- Display IDs may be a unique prefix or slug. Ambiguous matches fail; never
  guess. Canonical IDs are what `show` prints. Evidence uses exact `EVD-...`
  or a unique prefix from `.belay/evidence/*.ndjson`; slugs and fragments are
  rejected, and SQLite need not have indexed the record yet. `belay sync`
  indexes valid Evidence mirrors transactionally and keeps the previous index
  if validation fails. `belay rebuild` reports managed Markdown and Evidence
  counts separately. Herdr reviewers use runner provenance and must not treat
  `show EVD` as a settlement gate. Task settlement and Goal coverage stay
  separate judgments.
- `archived` means hide from default retrieval. Use `belay archive candidates`
  then `belay status <id> archived` after judging. Do not archive a Goal or
  Plan without human confirmation. `belay doctor` stale is skill/AGENTS drift,
  not entry archive.

## Entry body templates

Use these compact templates when creating an unfamiliar entry. Keep only the
sections that carry information; do not add empty placeholder sections.

```markdown
## Summary
- <outcome>

## Success Criteria
- [SC-001] <observable result and threshold>

## Constraints
- <constraint>

## Non-goals
- <explicitly excluded work>

## Verification
- <command or evidence>

## Risks
- <risk or assumption>
```

```markdown
## Intent Brief

### Problem
- <problem>
### Desired Outcome
- <outcome>
### Success Signals
- <observable signal>

<!-- Include only when relevant. -->
### Constraints
- <constraint>
### Non-goals
- <excluded work>
### Assumptions
- <assumption, explicitly labelled>
### Unknowns / Decisions Needed
- <unknown or decision needed>

## Delivery Map
| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | <outcome for SC-001> | <actor> | not-started | <evidence> |
| T-002 | SC-002 | <outcome for SC-002> | <actor> | not-started | <evidence> |

## T-001
- Objective: <objective>
- Scope: <scope>
- Steps: <steps>
- Acceptance: <acceptance condition>
- Verification: <command or evidence>

## T-002
- Objective: <objective>
- Scope: <scope>
- Steps: <steps>
- Acceptance: <acceptance condition>
- Verification: <command or evidence>
```

One task, one Goal item: each Delivery Map row maps to exactly one Success
Criterion. Add separate tasks for additional criteria; do not write
`SC-001, SC-002` in one row.

For `decision`, `work`, `review`, and `note`, use a short factual summary
followed by typed labels as needed: `Fact`, `Human decision`, `Assumption`,
`Hypothesis`, `Unknown`, `Evidence`, and `Follow-up`. Link entries with
`belay link`; record verification separately with `belay verify record`.

## Classify the work

- Tier 1 is a small, reversible change with clear scope. A separate Plan is optional.
- Tier 2 includes features and non-trivial changes. Create or update a Goal and Plan before implementation, following the consumer repository's policy for human gates.
- Tier 3 includes architecture, API contracts, security, migrations, and irreversible operations. Create or update a Goal and Plan before implementation, then follow consumer policy and existing authorization for any required human gates.
- Escalate when scope, reversibility, or risk is uncertain.

## Frame

1. Retrieve context per the command reference. Avoid broad reads of `.belay/entries/` unless a command identifies a specific source path.
2. Draft an Intent Brief in the Plan with Problem, Desired Outcome, and Success Signals. Add Constraints, Non-goals, material Assumptions, and Unknowns / Decisions Needed when relevant; omit empty placeholders.
3. Separate facts, assumptions, unknowns, and human decisions. Ask before choices that materially change the outcome, affect security or data loss, create external commitments, or are irreversible. Explicitly record and proceed with small, reversible assumptions.
4. Do not start implementation while a relevant Unknown / Decision Needed names an open choice the implementer would have to guess. Park it explicitly or get a human decision. Continue with a recorded small, reversible assumption; if a light pass at Frame/Map leaves a material unknown or vague Steps, stop and redo the Map rather than handing it to implementation.

## Map

1. Give each Goal Success Criterion a stable, document-local ID using `SC-NNN`, starting at `SC-001`. Never renumber or reuse an ID.
2. Link each Plan to exactly one Goal (`fulfills` or `implements`). If work spans multiple Goals, use separate Plans rather than multiple Goal links on one Plan.
3. Add a Delivery Map to the Plan with columns: ID, Goal item, Outcome / Task, Actor, State, and Verification / Evidence.
4. One task, one Goal item. Each row's Goal item column names exactly one Success Criterion — `SC-NNN` when the Plan has a single Goal link, or a fully qualified `GOAL-...#sc-nnn` when it does not. Never list multiple Goal items in one row; comma-separated values such as `SC-001, SC-002` break `belay context compile --focus` and fragment-scoped `belay show`. Cover each criterion with its own task; mention additional criteria only in Acceptance or Verification.
5. Map every Success Criterion to at least one task. Explain any task that has no Goal item.
6. Give tasks stable, document-local IDs using `T-NNN`, starting at `T-001`. Never renumber or reuse an ID. Outside the defining document, use fully qualified references such as `GOAL-...#sc-001` and `PLN-...#t-001`.
7. Task states are limited to `not-started`, `in-progress`, `blocked`, `implemented`, `verified`, and `dropped`. `implemented` means the change exists; `verified` requires fresh passing Evidence that actually checks the mapped outcome. A test definition is not passing Evidence.
8. Keep dropped tasks visible and record the reason and approval source.
9. Give every task a `## T-NNN` body section in the same Plan. The row is the index and the state; the section is what a reader with no prior context acts on, and it is what `belay show <plan-id>#t-nnn` returns. Carry at least Objective, Scope, Steps, Acceptance, and Verification; add whatever else your workflow needs, since `belay plan lint` ignores fields it does not require. Run `belay plan lint <plan-id>` after drafting or materially editing a Plan.

## Execute

1. Use the Delivery Map Task ID as the active work unit and keep its state current.
2. Add newly discovered tasks, assumptions, unknowns, constraints, and scope changes instead of silently absorbing them.
3. Link Work and Evidence to the relevant Goal item using `fulfills` and `verifies` relations. Create a decision entry when implementation establishes a meaningful architectural, API, operational, or tradeoff decision; link a superseding decision to the old one with `supersedes` and set the old one's status to `superseded`.
4. Reconcile after a meaningful task, a discovered requirement or risk, a scope or design change, before interruption or handoff, when the human asks for status, and before declaring completion.

Keep reconciliation consistent with the Delivery Map. Report verified outcomes,
implemented, unverified work, in-progress work, blockers, changed assumptions,
decisions needed, and the next action when relevant. Use a brief update for a small task and
criterion-level coverage for a multi-task delivery; omit empty categories.

Run checks that establish the Task's Acceptance and any repository-required
checks. Once they pass, repeat or broaden them only after relevant changes,
new failures, a concrete unresolved concern, or a required gate. Record what
was actually checked and leave unsupported outcomes unverified.

## Assure completion

Use independent review and final human acceptance when required by consumer repository policy or explicit Goal criteria; native sandbox and approval controls remain the authority. Never infer human acceptance. Do not declare the Goal complete until:

- every Success Criterion has delivery tasks and relevant passing Evidence;
- no `implemented`, `blocked`, or important unknown item is counted as complete;
- the diff respects Constraints and Non-goals;
- specification changes and dropped tasks have reasons and applicable approval sources; and
- any consumer-policy or explicit-Goal acceptance requirement is satisfied and recorded as Evidence.

## Update trace

1. Use `belay add goal` for intent, then use `belay work create --task <plan-id>#t-001` for implementation Work so the Goal criterion and required links are derived atomically. Use `belay link` for additional relationships and link Decision entries to intent with `fulfills`. Run `belay goal lint <goal-id>` after drafting or materially editing a Goal.
2. Record validation with `belay verify record` and inspect `belay coverage` before release decisions.
3. Run `belay sync` after direct managed Markdown edits. Use terminal statuses (`abandoned`, `rejected`, `superseded`, `archived`) instead of deleting trace history. `archived` hides an entry from default search and compile; it is not a substitute for `completed`.
4. Entry-body templates are embedded above, so authoring an unfamiliar entry
   type does not depend on another repository-surface file.

## Conflict safety

Never overwrite an unresolved sync conflict. Inspect both sides and use
`belay sync --prefer markdown <id>` or `belay sync --prefer sqlite <id>` only
after the intended source of truth is known.

Repository-specific policy (human gates, review budgeting and round limits,
version control, merge rules) belongs in the repository `AGENTS.md` or
`CLAUDE.md`, not in this generic skill.
