---
schema_version: 1
id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
type: goal
title: Maintain intent-to-evidence alignment during agent delivery
status: completed
created_at: 2026-07-12T16:39:56+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Human intent, implementation state, and verification evidence remain aligned throughout Tier 2 and Tier 3 agent coding work.

## Success Criteria

- [SC-001] Tier 2 and Tier 3 work starts with an Intent Brief that exposes assumptions, unknowns, and human decisions.
- [SC-002] Every Goal success criterion maps to observable delivery tasks and a verification method.
- [SC-003] AI and humans can distinguish not started, in progress, blocked, implemented, verified, and dropped work.
- [SC-004] Reconciliation exposes scope changes, unimplemented criteria, and unverified work before completion.
- [SC-005] Completion requires relevant Evidence and human acceptance rather than implementation alone.

## Constraints

- Belay core remains deterministic and does not call an LLM.
- The workflow must not replace an issue tracker or require detailed task management for Tier 1 work.
- Productization follows dogfooding evidence rather than preceding it.

## Non-goals

- Eliminating all ambiguity from human requests.
- Encoding a comprehensive software-design knowledge base in the Belay skill.
- Treating task completion percentage as a quality score.

## Verification

- Dogfood the Agent-first MVP on five Tier 2 or Tier 3 tasks.
- Record whether Intent correction, missing implementation, missing verification, or specification drift is found before completion.
- Compare operational value with the time, token, and stale-map overhead.

## Risks

- A consistent but incorrect Intent Brief can create an orderly misunderstanding.
- Delivery Maps can become stale or ceremonial.
- Task modeling can expand Belay into project management.

## Closure

- REV-20260727T184743-001で5件のTier 2 / Tier 3 dogfoodingを評価し、MVP規約の実用的価値とstale-map overheadの両方を確認した。
- EVD-20260727T192335-001およびEVD-20260727T192348-001から004がSC-001からSC-005を検証する。
- DEC-20260727T192305-001により、first-class Plan lint、reconcile command、core completion gateは現時点で実装せず、具体的なreopen signalが観測された場合に新しいGoalで再評価する。
- EVD-20260727T192321-001は、評価結果、T-004からT-006のdrop、現在のGoal closureに対する人間承認を記録する。
- Unknown. repository外の利用で気づかれていない問題が存在する可能性は残る。
