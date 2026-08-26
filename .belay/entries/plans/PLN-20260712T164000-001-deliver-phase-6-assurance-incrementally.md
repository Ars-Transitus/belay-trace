---
schema_version: 1
id: PLN-20260712T164000-001-deliver-phase-6-assurance-incrementally
type: plan
title: Deliver Phase 6 assurance incrementally
status: completed
created_at: 2026-07-12T16:40:00+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
metadata: {}
---

## Intent Brief

### Problem

- Agent coding work can lose its current position, omit requested behavior, or confuse implementation with verified Goal achievement.
- Human requests may not initially contain enough specificity for a correct implementation plan.

### Desired Outcome

- AI and humans share a live map from interpreted intent through Goal criteria, delivery tasks, and Evidence.

### Success Signals

- Intent assumptions and unknowns are visible before implementation.
- Every Goal criterion has an observable task and verification path.
- Checkpoints report the same current state to AI and humans.

### Constraints

- Start with the agent skill and Plan body convention before adding core commands or schema.
- Keep Tier 1 lightweight.

### Non-goals

- Implementing Phase 6 features as part of this planning change.
- Replacing issue tracking.

### Assumptions

- Intent Brief and Delivery Map can be validated through five dogfooding tasks before first-class Task modeling is needed.

### Unknowns / Decisions Needed

- Whether reconciliation becomes a new command or an extension of detailed coverage.
- Whether Task should ever become a first-class object.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | Specify Intent Brief and Delivery Map conventions | AI + Human | verified | `docs/design/phase6.md`、現行AGENTS / Skill、EVD-20260727T192335-001、人間によるclosure acceptance EVD-20260727T192321-001 |
| T-002 | SC-001..SC-004 | Build Agent-first MVP integration | AI | verified | WRK-20260712T165103-001、WRK-20260712T172930-001、後続agent-guidance validation、EVD-20260727T192335-001、EVD-20260727T192348-001から003 |
| T-003 | SC-001..SC-004 | Dogfood five Tier 2 or Tier 3 tasks | AI + Human | verified | REV-20260727T184743-001、EVD-20260727T192335-001、EVD-20260727T192348-001から003、人間による評価受入れEVD-20260727T192321-001 |
| T-004 | SC-002..SC-004 | Add deterministic Plan lint if justified | AI | dropped | 5事例とrepository外運用で反復的な不足が観測されず、追加core機能の必要性が未立証。DEC-20260727T192305-001、承認source EVD-20260727T192321-001。将来のreopen signal発生時は新Goalで再評価 |
| T-005 | SC-003..SC-004 | Add reconciliation report if justified | AI | dropped | fixed reconciliation report、doctor、coverageで現時点の主要リスクを管理可能。専用commandの便益はUnknown。DEC-20260727T192305-001、承認source EVD-20260727T192321-001 |
| T-006 | SC-005 | Add opt-in completion gate if justified | AI + Human | dropped | Evidence、fresh review、Human Gate、doctorによるprocess gateが実運用で機能し、core gateの必要性は未立証。DEC-20260727T192305-001、承認source EVD-20260727T192321-001 |

## Retrospective Dogfooding Result

- 5/5事例でIntent BriefとDelivery Mapが使用された。
- 4/5事例で、実装開始または完了前に、実装欠落、未検証、Evidence欠陥、scope / gateの曖昧さ、または仕様欠陥が発見された。
- Browse Planでは後続Evidence反映後もDelivery Mapが`implemented`のまま残り、stale-map overheadが実際に発生した。Phase 6 Plan自身も同様に現状とのreconciliationが遅れた。
- Unknown. 時間、token、人間の確認時間は5事例で一貫して計測されておらず、定量的な費用対効果は主張できない。
- This is a hypothesis. 現行のAGENTS / Skill規約、`belay doctor`、`belay coverage`、fixed reconciliation reportで主要リスクを管理でき、専用CLI機能の追加価値はまだ立証されていない。
- 詳細と出典はREV-20260727T184743-001-evaluate-phase-6-agent-first-mvp-dogfoodingを参照する。

## Proposed Disposition

- DEC-20260727T192305-001によりDispositionを確定した。
- T-001からT-003はpassing Evidenceと人間受入れにより`verified`とした。
- T-004からT-006は必要性未立証を理由に、承認source EVD-20260727T192321-001を保持して`dropped`とした。
- 将来の再開は、同DecisionのReopen Signalsに対応する具体的Evidence、新しいGoal / Plan、人間承認を必要とする。

## Completion Reconciliation

- Goal Success Criteria SC-001からSC-005にはEVD-20260727T192335-001およびEVD-20260727T192348-001から004が対応する。
- Unknown. repository外の利用で気づかれていない問題が存在する可能性は残る。このUnknownは未開始taskを保持する理由とはせず、観測時のreopen signalとして扱う。
- 現在のdiffはPhase 6の追加core command、schema、first-class Task modelを導入していない。
- Human acceptance and drop approval: EVD-20260727T192321-001。
- Closure decision: DEC-20260727T192305-001-close-phase-6-mvp-and-reopen-only-on-observed-ne。

## Roadmap

- The durable design and phase gates are defined in `docs/design/phase6.md`.
