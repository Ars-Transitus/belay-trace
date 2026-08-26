---
schema_version: 1
id: WRK-20260726T123916-001-implement-operational-and-observable-belay-agent
type: work
title: Implement operational and observable Belay agent guidance
status: completed
created_at: 2026-07-26T12:39:16+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
metadata: {}
---

## Scope

- Implement approved Plan PLN-20260726T113522-001.
- Preserve the responsibility split established by DEC-20260726T011127-001.
- Do not change CLI state-transition or persistence contracts without a revised Plan and Decision.

## Progress

- T-001: verified — EVD-20260726T132738-001 records corrective provenance for the fixed rubric, Skill size, four baseline runs, and classification.
- T-002: verified — EVD-20260726T132738-002.
- T-003: verified — EVD-20260726T132739-001 and deterministic replay.
- T-004: verified — EVD-20260726T132739-002.
- T-005: verified — EVD-20260726T132739-003.
- T-006: verified — EVD-20260726T141140-002. Independent fresh Claude reruns completed S-003 at 10/10 and S-004 at 9/10; all four post-change scenarios completed with zero invalid invocations and zero gate violations.
- T-007: verified — EVD-20260726T132739-006; Rust 1.87 validation, rebuild, and doctor pass. SC-004 is passing under EVD-20260726T141140-001.
- T-008: verified — REV-20260726T135148-001-review-operational-and-observable-belay-agent-gu completed; High provenance finding addressed, and EVD-20260727T183606-001 records final human acceptance.
- T-009: verified — EVD-20260727T183937-001 records the earlier stale diagnosis; EVD-20260727T191548-001 records the subsequent passing `belay doctor`, with AGENTS and both installed Skills active and the generated / installed Codex SHA-256 values identical.

## Validation

- Rust 1.87 `cargo fmt --all -- --check`: pass.
- Rust 1.87 `cargo clippy --all-targets --locked -- -D warnings`: pass.
- Rust 1.87 `cargo test --all-targets --locked`: pass (124 tests total).
- `belay rebuild`: pass (36 managed Markdown entries).
- `belay doctor`: pass; deterministic repository state is healthy, while Goal-level Evidence freshness is reported separately as drift detail.
- `belay coverage`: decision, implementation, test, and monitoring are all 100% verified after Goal-level Work, test, and metric Evidence were recorded.
- A newer local toolchain raised an unrelated pre-existing `clippy::obfuscated-if-else` finding in `src/browse.rs`; the Plan requires Rust 1.87, where clippy passes.

## Observations

- The Plan was approved by the explicit user request on 2026-07-26.
- Existing working-copy changes were preserved in the parent jj change; implementation starts in a new child change.
- Baseline Codex runs completed with zero invalid invocations and zero gate violations.
- Baseline Claude runs explicitly recognized the Skill but could not start Bash because the runner failed to create its session environment. This is a runtime execution failure, not a Belay CLI failure.
- The observed content gap is narrow: the lifecycle run used one-time help for body syntax and link direction. The larger gap is distinguishing repository state, runtime recognition, and runtime execution.
- Post-change Codex completed S-001/S-002 with zero invalid invocations and zero gate violations, but routine help remained 2.
- Post-change Claude again recognized the Skill but could not execute Bash. Deterministic replay independently confirmed S-003/S-004 command syntax and recovery outcomes.
- A subsequent human-arranged independent run used Claude Code 2.1.220, separate fresh sessions, isolated fixtures, and the working-copy Belay 0.3.2 binary. S-003 and S-004 both completed.
- S-003 explicitly registered and loaded the repository Skill. S-004 did not register or invoke the stale Skill, so runtime recognition is correctly Unknown despite the child agent initially claiming recognition.
- All post-change help calls were non-repetitive and classified expected-help. The raw total rose from 2 across two completed baseline runs to 9 across four completed post-change runs; differing denominators prevent a causal help-reduction claim.
- Shared Skill size changed from 4,937 bytes / 89 source lines to 7,429 bytes / 172 source lines.
- Criterion-scoped Evidence is queried with `belay verify status <goal>#sc-001`; the initial recipe used the Goal only and was corrected during deterministic replay.
- Independent review found that the first validation Evidence records inherited Git HEAD instead of the tested jj working snapshot. Append-only corrective Evidence EVD-20260726T132738-001 through EVD-20260726T132739-006 explicitly records tested jj commit c1fc67fefc5c219dde65feddcd996eee40783e80; the earlier records remain as historical observations.

## Assumptions and Hypotheses

- This is a hypothesis. The pre-registered scenarios can be implemented as deterministic repository fixtures plus isolated agent-run logs without adding an LLM dependency to Belay core.
- This is a hypothesis. The unchanged Codex help count is primarily an activation/recognition observability gap, because runtime recognition was Unknown; the current evaluation does not prove the added content was loaded before command selection.

## Blockers

- No implementation or scenario-execution blocker remains.
- Final repository integration validation passed after the Codex Skill refresh.
- Raw Codex transcripts were not exposed. Claude raw transcripts were inspected by the independent scorer but remain external to the repository, so checkout-local transcript reproduction remains unavailable.
- Final human acceptance was recorded as EVD-20260727T183606-001.

## Pull Request Delivery

- [PR #17](https://github.com/Ars-Transitus/belay-trace/pull/17) carried the completed agent-guidance implementation, Phase 6 closure records, and trace reconciliation.
- Copilot reviewed all 53 changed files and raised one actionable grammar finding. Commit `533b16e048e662cc05949acbb9c6aa12e8c60afd` changed `repository-active state do not prove` to `repository-active states do not prove` in the canonical source, test assertion, and generated Codex / Claude Skills.
- The final PR head passed all nine GitHub checks, including Rust 1.87, Playwright, documentation, links, typos, and PR title validation. EVD-20260728T201944-001 records the CI result.
- PR #17 was merged into `main` on 2026-07-28 at 09:32:03 UTC as commit `d03ae146edf74156d62b00e6848c1bf1962054c6`.
- Tagging, GitHub Release creation, and registry publication were not part of PR #17.
