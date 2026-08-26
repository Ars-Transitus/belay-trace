---
schema_version: 1
id: DEC-20260727T192305-001-close-phase-6-mvp-and-reopen-only-on-observed-ne
type: decision
title: Close Phase 6 MVP and reopen only on observed need
status: accepted
created_at: 2026-07-27T19:23:05+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: PLN-20260712T164000-001-deliver-phase-6-assurance-incrementally
- relation: references
  id: REV-20260727T184743-001-evaluate-phase-6-agent-first-mvp-dogfooding
- relation: fulfills
  id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
metadata: {}
---

## Decision

- Phase 6 Agent-first MVPを現状のAGENTS / Skill規約、Intent Brief、Delivery Map、Evidence、review、fixed reconciliation reportとして維持する。
- first-class deterministic Plan lint、`belay reconcile` command、core completion gateは現時点では実装しない。
- PLN-20260712T164000-001のT-004からT-006は、必要性未立証を理由に承認source付きで`dropped`とする。
- Phase 6 Goal、Plan、既存Workは、実装済みMVPと5事例dogfoodingの評価を記録したうえで完了する。
- 将来、明確な反復的失敗または運用コストが観測された場合は、このDecisionを暗黙に再開せず、新しいGoal / Plan / Decisionとして再評価する。

## Evidence Basis

- REV-20260727T184743-001は、5/5事例でIntent BriefとDelivery Mapが使われ、4/5事例で実装開始または完了前に意味のある欠落や不整合が表面化したと評価した。
- Browse PlanとPhase 6 Plan自身ではstale-map overheadが観測され、現在の運用が自動整合ではなく手動reconciliationに依存することも確認された。
- 人間はBelayを本repository以外でも利用しており、現時点で大きな問題は観測していない。
- Unknown. 気づかれていない問題が存在する可能性は残る。

## Rationale

- 観測されていない問題に備えて未開始taskを無期限に残すと、現在地を不正確にし、Phase 6が扱うstale-map問題を自ら増やす。
- 現状で重大な不足が反復観測されていない以上、追加core機能の実装・保守コストを正当化できない。
- terminal statusと再開条件を記録して一度閉じる方が、未開始taskを曖昧に保持するよりresumabilityとtraceabilityが高い。

## Reopen Signals

新しいGoalを作る候補は、次のいずれかが複数回または重大な形で観測された場合とする。

- Delivery Mapのstale化により、誤った実装開始、完了判定、release判断が発生する。
- Goal Success CriterionとPlan taskの欠落対応をdoctor / reviewで発見できない。
- fixed reconciliation reportだけではfresh contextが現在地を再構築できない。
- Human GateまたはEvidence要件が繰り返し迂回され、process guidanceでは防止できない。
- 複数repositoryで同じ手動reconciliation負荷が反復し、専用commandの便益を測定できる。

## Consequences

- 現在のAgent-first workflowは維持する。
- T-004からT-006の実装は行わない。
- 将来の再開は新しい観測Evidence、Goal、Plan、人間承認を必要とする。
- This is a hypothesis. 明示的な再開signalを設けることで、未知の問題を否定せずに現在のtraceを閉じられる。
