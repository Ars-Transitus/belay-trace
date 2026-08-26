---
schema_version: 1
id: GOAL-20260820T214155-001-retrieval-hygiene-archiv
type: goal
title: retrieval-hygiene-archive
status: active
created_at: 2026-08-20T21:41:55+09:00
updated_at: 2026-08-20T21:57:17+09:00
revision: 3
tags: []
links: []
metadata: {}
---

## Summary

- Default retrieval hides archived entries. Unique prefix and slug IDs resolve. Working-set compile and a task packet cut token cost. Semantic archive judgment and Frame/Map quality stay in the Skill.

## Desired outcome

- Agents retrieve live context without a query string, resolve incomplete IDs, and archive stale history without deleting it.

## Success criteria

- [SC-001] `belay show`, `status`, `link`, `plan lint`, `goal lint`, and context `--seed`/`--focus` resolve a unique display-id prefix or slug; 0 or 2+ matches fail with candidates and never fall back to FTS.
- [SC-002] Every entry type allows `archived`. Search and context exclude archived by default; `--include-archived` or `--status archived` returns them. `show` by resolved ID still prints archived entries.
- [SC-003] `belay archive candidates` lists deterministic candidates (terminal status, no live inbound link, superseded old side) with reason codes and does not apply status changes.
- [SC-004] `belay context` and `belay context compile` with no task compile the live working set and a Next index from Delivery Map rows.
- [SC-005] `belay context compile --focus <plan>#t-nnn` prints Intent Brief constraints/non-goals/assumptions, that task, the mapped SC, and recent Evidence — not the whole Plan.
- [SC-006] Generated belay-trace Skill teaches working-set compile, `--focus`, unique IDs, archive candidates, and a Frame/Map quality bar that stops on open Unknowns.

## Constraints

- Belay core stays deterministic and does not call an LLM.
- Do not change the fragment standard, Route contracts, or FTS ranking.
- Incomplete IDs must not silently FTS-fallback.
- `.agent-safety/` and installed consumer skills are not edited by this change; propose safe-autonomy text instead.
- Unique resolution applies to CLI-facing reads and mutations; stored Route/Evidence IDs stay canonical.

## Non-goals

- Auto-applying archive. Embeddings. A `belay task` command. Model names or Difficulty inside belay. Mixing doctor skill-stale with entry archive. `plan improve` in this increment.

## Assumptions / Unknowns

- Assumption: the approved design plan is the Intent Brief the human accepted.
- Assumption: `completed` remains visible in default compile; only `archived` is hidden.
- Unknown: none identified for this increment.

## Verification

- `cargo test` covering unique ID resolution, archived default exclude, archive candidates, working-set compile, and `--focus` packets.
- `belay init` generated skill contains the new retrieval forms.

## Risks

- Default-excluding archived from search is a CLI contract change; `--include-archived` must stay discoverable in help.
- Unique prefix resolution on mutations could hit the wrong entry if the uniqueness check is wrong; fail closed on 0 or 2+ matches.
