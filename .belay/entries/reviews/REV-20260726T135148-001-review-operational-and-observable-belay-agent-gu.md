---
schema_version: 1
id: REV-20260726T135148-001-review-operational-and-observable-belay-agent-gu
type: review
title: Review operational and observable Belay agent guidance
status: completed
created_at: 2026-07-26T13:51:48+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: reviews
  id: WRK-20260726T123916-001-implement-operational-and-observable-belay-agent
metadata: {}
---

## Review Method

- Method: subagent-review in a fresh context that did not implement the change.
- Scope: approved Goal, Plan, Decision, Work, relevant jj diff, generated artifacts, Rust 1.87 validation, evaluation records, and Evidence.
- Planning / implementation / review budget: high reasoning for planning and implementation; fresh high-reasoning Codex sub-agent for review.
- Outcome: changes requested for verification/completion; no blocking code correctness defect found.
- requires_human_review: true

## Findings

### High — Addressed: Evidence provenance did not identify the tested jj snapshot

- Initial implementation Evidence inherited Git HEAD `517b7975` rather than the tested jj working snapshot.
- Append-only corrective Evidence EVD-20260726T132738-001 through EVD-20260726T132739-006 now explicitly records tested commit `c1fc67fefc5c219dde65feddcd996eee40783e80`.
- Earlier Evidence remains as history and is no longer the cited Delivery Map basis.

### Medium — Open: behavioral improvement is Unknown

- Skill source grew from 4,937 bytes / 89 source lines to 7,429 bytes / 172 lines.
- Routine help remained 2 to 2; completed scenarios remained 2/4 to 2/4.
- Claude recognized the Skill but could not execute Bash in baseline or post-change runs.
- Codex runtime recognition was Unknown.
- SC-004 and T-006 correctly remain warn / implemented-unverified. Human acceptance or a valid recognition-and-execution runner is required.

### Medium — Open: raw transcript evidence is unavailable

- Versioned run records preserve final reports and available command inventories, not raw Codex sub-agent transcripts.
- Exact Codex runtime version and transcript-level independent rescoring remain Unknown.
- The protocol deviation is now documented in `evaluation/agent-guidance/README.md`.

## Validation Reviewed

- Rust 1.87 `cargo fmt --all -- --check`: pass.
- Rust 1.87 `cargo clippy --all-targets --locked -- -D warnings`: pass.
- Rust 1.87 `cargo test --all-targets --locked`: pass, 124 tests.
- `belay rebuild`: pass for 36 managed Markdown entries.
- `belay doctor`: exit 0.
- `belay coverage`: partial, consistent with warn SC-004.
- Generated Codex and Claude Skill files are byte-identical.
- CLI recipes match current help syntax.

## Positive Findings

- Doctor output limits claims to deterministic repository state and explicitly disclaims runtime recognition and execution.
- Generated, inactive, active, stale, missing, and malformed states have tested recovery guidance.
- Plan and Work do not treat SC-004 or T-006 as complete.
- Human gates, conflict safety, deterministic core behavior, and existing lifecycle contracts remain unchanged.

## Follow-up Actions

- Agent: keep SC-004/T-006 unverified and present the behavioral limitation to the human.
- Human: decide whether the approximately 50% Skill byte increase is acceptable as a provisional deterministic guidance improvement, or require reevaluation in a runner with observable Skill recognition and working command execution.
- Future evaluator: retain raw transcripts or stable external references/digests when the runtime surface exposes them.

## Post-review Evidence Update

- On 2026-07-26 the human arranged independent fresh Claude Code 2.1.220 reruns using isolated fixtures and the working-copy Belay 0.3.2 binary.
- S-003 completed at 10/10 and S-004 at 9/10, with zero invalid Belay invocations and zero Human-Gated Workflow violations.
- EVD-20260726T141140-001 and EVD-20260726T141140-002 address the behavioral execution finding. SC-004 and T-006 can now be verified for the pre-registered qualitative outcomes.
- The causal claim remains Unknown: baseline Claude execution was blocked, completed-run denominators differ, and the larger Skill was not recognized in S-004.
- The Claude transcripts were inspected by the independent scorer but remain external and unversioned. The Codex transcript limitation is unchanged.

## Pull Request Follow-up

- Copilot reviewed all 53 files in PR #17 and reported one Low-severity grammar issue in the user-facing Skill text: singular `state` was paired with plural `do not prove`.
- The finding was addressed consistently in the canonical Skill source, generated and installed Codex / Claude Skills, and the CLI test assertion in commit `533b16e048e662cc05949acbb9c6aa12e8c60afd`.
- Rust 1.87 fmt, clippy, and all 124 tests passed locally after the correction. The final PR head then passed all nine GitHub checks, recorded as EVD-20260728T201944-001.
- No additional Copilot review thread or blocking review finding was present.
- PR #17 was merged into `main` as `d03ae146edf74156d62b00e6848c1bf1962054c6`.
