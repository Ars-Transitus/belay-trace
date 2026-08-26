# belay-trace

`belay-trace` is a local-first CLI for preserving the goals, plans, decisions,
work logs, reviews, evidence, and notes behind AI-assisted software work.

SQLite is the operational store. Deterministic Markdown files under
`.belay/entries/` are the editable Git review and recovery surface.

## Install And Initialize

The v1 implementation requires Rust 1.87 or newer.

```sh
cargo build --release --locked
./target/release/belay init
```

`belay init` creates `.belay/config.toml`, local SQLite state, managed entry
directories, and generated agent integration templates. It does not modify
`AGENTS.md` or install a skill unless explicitly requested:

```sh
belay init --update-agents
belay init --install-skill codex
belay init --install-skill claude
belay init --install-skill codex --install-skill claude
```

`--install-skill` is repeatable so repositories using both agents can activate
both integrations explicitly in one command. If a copied template may contain
local SQLite state, rebuild that ignored state from tracked Markdown rather
than trusting or deleting it in place:

```sh
belay init --reset-state
```

To update one or more existing projects from this checkout, use:

```sh
./scripts/update-existing-project.sh ~/src/project-a ~/src/project-b
```

The script builds the release binary, refreshes generated assets, preserves
which AGENTS/Codex/Claude integrations are currently active, and runs
`belay doctor`. It refuses to initialize an unrelated directory unless
`--initialize` is passed. Use `--help` for explicit activation and state-reset
options, or `--belay /path/to/belay` to skip the release build.

The reset is an atomic rebuild from `.belay/entries/`; it does not delete the
tracked Markdown source. Template archives and copy scripts should exclude
`.belay/state/` entirely.

For repositories that use belay as their trace system, keep the recommended
project instructions in the repository root `AGENTS.md`. The root file should
make belay entries the source of truth for plans, decisions, work, and reviews.
`belay init --update-agents` manages only the marker-scoped
`<!-- belay-trace:start -->` / `<!-- belay-trace:end -->` section, so existing
repository-specific instructions outside that section remain under normal human
review.

Agent guidance has three separate sources of truth:

- repository `AGENTS.md` owns repository-specific policy, approval gates, and
  prohibitions;
- the generated and repository-installed `belay-trace` Skill owns reusable
  workflow recipes, decision points, and recovery procedures;
- `belay <command> --help` owns complete command and option syntax.

Do not copy the full CLI reference into AGENTS.md or the Skill. A one-time help
lookup for an unlisted option or exceptional operation is expected; repeated
lookup of the same routine or an invalid invocation indicates a guidance gap.

## Create And Link Traces

Create entries non-interactively with inline content, a file, or standard input:

```sh
belay add goal --title "Reliable repository sync"

belay add decision \
  --title "Use SQLite for operational state" \
  --body "Keep local retrieval fast and rebuild from tracked Markdown."

belay add work \
  --title "Implement repository sync" \
  --body-file ./work-notes.md

printf '%s\n' "Review findings" | \
  belay add review --title "Sync implementation review" --stdin
```

Goal entries can omit a body source; belay writes the required Goal sections as
an editable template. Other entry types require `--body`, `--body-file`, or
`--stdin`. Entry types are `goal`, `plan`, `decision`, `work`, `review`, and
`note`. Commands print the generated display ID, for example:

```text
GOAL-20260607T085900-001-reliable-repository-sync
DEC-20260607T090000-001-use-sqlite
```

Use display IDs for links and status transitions:

```sh
belay link \
  WRK-20260607T091000-001-implement-sync \
  GOAL-20260607T085900-001-reliable-repository-sync \
  --relation fulfills

belay status DEC-20260607T090000-001-use-sqlite accepted
belay show DEC-20260607T090000-001-use-sqlite
```

Review Goal quality without calling an LLM:

```sh
belay goal lint GOAL-20260607T085900-001-reliable-repository-sync
belay goal improve GOAL-20260607T085900-001-reliable-repository-sync --budget 3000
```

## Synchronize Markdown And SQLite

Managed Markdown is editable. Import direct edits or regenerate stale mirrors:

```sh
belay sync
belay sync DEC-20260607T090000-001-use-sqlite
```

When both sides changed, sync preserves both and exits with conflict category
`5`. Resolve one entry explicitly:

```sh
belay sync --prefer markdown DEC-20260607T090000-001-use-sqlite
belay sync --prefer sqlite DEC-20260607T090000-001-use-sqlite
```

Deletion does not propagate in v1. A missing SQLite row or mirror is restored
from the remaining side.

## Search And Context

Search supports exact display IDs, structured filters, and deduplicated
FTS5/BM25 keyword results:

```sh
belay search "sqlite migration"
belay search --type decision --status accepted
belay search --status draft --status active
belay search --exclude-status completed --exclude-status abandoned
belay search --tag release
belay search --id DEC-20260607T090000-001-use-sqlite
```

Generate bounded, source-attributed context for humans or agents:

```sh
belay context "implement repository sync" --format human --budget 2500
belay context "implement repository sync" --format agent --budget 2500
belay context compile "implement repository sync" --budget 4000
belay context compile --format agent --budget 4000
belay context compile --focus PLN-20260607T090500-001-deliver-sync#t-001
belay archive candidates
belay search --include-archived
```

Context selection follows BM25 relevance, then linked entries in deterministic
order. Agent output admits a budget-scaled set of candidates, keeps at least one
Markdown evidence unit for included entries, and weights remaining space toward
higher-ranked entries while preserving the 90 percent selection limit. The
`compile` form adds Goal, constraints, non-goals, Evidence, and past-failure
sections before the ranked context. Omit the task to compile the live working
set. `--focus PLN-...#t-001` emits a task packet. Archived entries are omitted
unless `--include-archived` is passed. Display IDs may be a unique prefix or
slug.

Context uses direct links and a deterministic token estimate. Embeddings are not
required for the v1 workflow.

## Local Trace Browser

Launch a read-only provenance browser bound only to loopback:

```sh
belay browse
belay browse --port 8765 --open
```

The command prints the assigned `127.0.0.1` URL. Library search reuses the same
FTS5/BM25 and filter semantics as `belay search`; Reader shows sanitized
Markdown, typed inbound/outbound links, linked Plan Delivery Map tasks and Goal
items, and every Evidence verdict. Explore starts with Goals and expands one
neighborhood at a time from Entry to Evidence to its captured commit and files
changed in that commit. Reader deep links can still start the graph from any
Entry. Changed files are Git history context, not a claim that they are Entry
source files or the scope verified by Evidence.

Browse copies SQLite into a single in-memory generation. Reload builds a new
generation before swapping it in, and a failed reload leaves the prior snapshot
active. The server does not edit managed Markdown, SQLite, Evidence, revisions,
or sync baselines. It uses bundled assets and needs no runtime network access.

## Verify And Cover Goals

Record append-only Evidence for existing verification systems:

```sh
belay verify record \
  --kind test \
  --verdict pass \
  --source "cargo test" \
  --summary "unit and integration tests passed" \
  --verifies GOAL-20260607T085900-001-reliable-repository-sync

belay verify import --junit target/junit.xml --verifies WRK-20260607T091000-001-implement-sync
belay verify status GOAL-20260607T085900-001-reliable-repository-sync
```

Evidence is mirrored to `.belay/evidence/YYYY-MM.ndjson` and indexed in SQLite.
Success Criterion and Delivery Map task IDs use document-local `SC-NNN` and
`T-NNN` forms. Cross-entry references must be fully qualified, for example
`GOAL-...#sc-001` and `PLN-...#t-001`; see the
[ID and reference standard](docs/id-reference-standard.md).
Compute Goal Coverage with traceability and verified counts kept separate:

```sh
belay coverage
belay coverage --format json --fail-under verified=60
```

## Export Snapshots

Exports are external point-in-time artifacts. They are not managed mirrors and
are never sync or rebuild inputs.

```sh
belay export markdown --output ./artifacts/belay-export.md
belay export json \
  --type decision \
  --status accepted \
  --output ./artifacts/accepted-decisions.json
belay export ndjson --tag release --output ./artifacts/release.ndjson
belay export json \
  --id DEC-20260607T090000-001-use-sqlite \
  --output ./artifacts/decision.json
```

Filters are optional and combine with AND semantics. Export ordering is
deterministic. Normal exports contain display IDs and never expose internal
SQLite integer IDs. Destinations inside `.belay/entries/` are rejected.

## Diagnose And Rebuild

Run read-only repository health checks:

```sh
belay doctor
```

Doctor checks configuration, SQLite schema and foreign keys, FTS5/BM25,
managed Markdown validity, Goal sections, Evidence freshness, sync drift,
temporary files, and agent integration.

Agent integration checks describe repository files, not agent runtime internals:

| State | Repository observation | Recovery |
| --- | --- | --- |
| generated `present` | Canonical generated artifact exists; activation is not implied | None |
| generated `missing` or `stale` | Canonical artifact is absent or differs | `belay init` |
| integration `inactive` | Optional AGENTS/Skill target is absent | `belay init --update-agents` or `belay init --install-skill <target>` |
| integration `active` | Target exactly matches canonical generated content | None |
| integration `stale` | Target exists but differs from canonical content | Refresh with the matching explicit init option |
| integration `malformed` | The marker-managed AGENTS block has an unsafe marker structure | Repair to one ordered marker pair, then run `belay init --update-agents` |

Installed Skill content that differs from canonical content is reported as
`stale`; `malformed` currently applies to the marker-managed AGENTS block.
`active` does not prove that Codex or Claude loaded the Skill. Runtime
recognition must come from the agent surface, and successful command execution
is a third, separate observation. Report either runtime state as Unknown when
there is no direct evidence.

Rebuild SQLite and search indexes from all validated managed Markdown:

```sh
belay rebuild
```

Rebuild uses a temporary database and replaces active state only after the new
database is complete.

## Exit Status

| Code | Meaning |
|---:|---|
| `0` | success, help, or version |
| `2` | invalid invocation or arguments |
| `3` | repository not initialized or configuration unavailable |
| `4` | input, entry, schema, path, or not-found validation failure |
| `5` | sync conflict or repository drift requiring resolution |
| `6` | filesystem, SQLite, storage, or runtime capability failure |

Run `belay --help` or `belay <command> --help` for command-specific behavior,
side effects, examples, and related commands.

## Repository Layout

```text
.belay/
  config.toml
  entries/
    goals/
    plans/
    decisions/
    work/
    reviews/
    notes/
  evidence/
    YYYY-MM.ndjson
  state/
    belay.sqlite
  agent/
    AGENTS.md.snippet
    claude/SKILL.md
    codex/SKILL.md
```

The SQLite database is local operational state and ignored by Git by default.
The managed Markdown entries are the tracked review and recovery surface.
The root `AGENTS.md` is the recommended place for repository-specific agent
workflow policy; `.belay/agent/AGENTS.md.snippet` is the generated integration
snippet used by `belay init --update-agents`.

Use `docs/` for durable system documentation. Keep intent, approval, trade-off
history, execution notes, reviews, and evidence in belay entries, linking to
the durable document instead of maintaining two copies of the same truth.
