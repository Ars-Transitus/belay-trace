---
schema_version: 1
id: REV-20260727T184743-001-evaluate-phase-6-agent-first-mvp-dogfooding
type: review
title: Evaluate Phase 6 Agent-first MVP dogfooding
status: completed
created_at: 2026-07-27T18:47:43+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: reviews
  id: PLN-20260712T164000-001-deliver-phase-6-assurance-incrementally
- relation: supports
  id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
metadata: {}
---

## Review Method

- Method: retrospective-evidence-review。
- 対象: Phase 6 Agent-first MVPを使用した5件のTier 2 / Tier 3事例。
- 参照: 各Goal、Plan、Work、Review、Evidence、現在のstatus、`docs/design/phase6.md`。
- このReviewは既存記録から観測可能な結果を再構成する。実行時間、token量、人間の内面的理解は記録がないため評価しない。
- requires_human_review: true

## Cases

### C-001 Local trace provenance browser

- Intent BriefとDelivery Mapは実装範囲、read-only制約、security、Git provenanceの非保証範囲を明確にした。
- 実装Workは完了したが、PlanのT-001からT-006が`implemented`のまま残り、Playwright Linux CI pendingも後続CI後に更新されなかった。
- 2026-07-27のreconciliationで後続Evidenceへ接続し、全taskを`verified`、Planを`completed`へ修正した。
- Finding class: stale-map overhead、missing reconciliation。

### C-002 Explore node activation

- fresh-context reviewが、初期Goal loading前にclick可能になる問題と、同一nodeへの重複requestを検出し、実装中に修正した。
- local Playwrightが起動できない状態を`verified`と扱わず、browser-capable CIと人間受入れまでGoal completionを保留した。
- Finding class: missing implementation、missing verification、completion assurance。

### C-003 Version 0.3.2 release candidate

- Intent BriefとDelivery Mapが、version metadata、draft PR、merge、tag、GitHub Release、publicationの別々のHuman Gateを明確にした。
- reviewが、T-005の外部action表現と「変更全体がmetadataのみ」と誤読できるWork表現を検出し、記録を修正した。
- Playwright CI完了前のrelease-readiness claimを防ぎ、mergeも別承認まで保留した。
- Finding class: intent correction、scope clarification、missing verification。

### C-004 Operational and observable agent guidance

- independent reviewが、Evidenceのcommit provenanceがtested jj snapshotを指していないHigh findingを検出し、append-only corrective Evidenceで修正した。
- behavioral improvementとraw transcript availabilityをUnknownとして残し、後続の独立runと人間受入れまでT-006/T-008を完了扱いしなかった。
- completion reconciliationでinstalled Codex Skillのstaleを検出し、人間受入れ後もT-009をblockedとしてGoal completionを停止した。
- Finding class: evidence defect、unverified behavior、completion drift。

### C-005 Belay Route protocol validation plan

- Tier 3の実装開始前レビューで、弱いbaseline、schemaとfixtureの循環、Route Inputの決定性境界、reviewer独立性、相互干渉する成功閾値という5件のblocking findingを検出した。
- Goal/Planはdraft、全実装taskはnot-startedのまま維持され、Human Gateを越えていない。
- Finding class: intent correction、specification defect、premature implementation prevention。

## Findings

### Operational value

- 5/5事例でIntent BriefとDelivery Mapが使用された。
- 4/5事例で、完了または実装開始前に、実装欠落、未検証、Evidence欠陥、scope/gateの曖昧さ、または仕様欠陥が発見された。
- Explore、release、agent guidance、Routeでは、`implemented`と`verified`、または計画と実装承認を分けることが実際の停止条件として機能した。
- Browseでは、Planがstaleになり、後続Evidenceが自動的にはDelivery Mapへ反映されなかった。現在のAgent-first規約はstale-mapを発見できるが、防止または自動解消はしない。

### Cost and limitations

- Fact: stale-map事例はBrowse PlanとPhase 6 Plan自身で観測された。
- Fact: current workflowはPlan、Work、Review、Evidence間の手動reconciliationを必要とする。
- Unknown. 5事例の作成・更新に要した時間、token、人間の確認時間は一貫して記録されていない。
- Unknown. 専用Plan lintまたはreconcile commandが、実装・保守コストを上回る追加価値を持つか。
- This is a hypothesis. 現時点では、AGENTS/Skillの規約、`belay doctor`の構造検査、`belay coverage`、fixed reconciliation reportを組み合わせれば、専用commandなしで主要なリスクを管理できる。

## Recommendation

- T-001とT-002は実装済みのAgent-first MVPとして維持し、本Reviewと人間判断を用いてverificationを再構成する。
- T-003は5事例のretrospective evaluationが存在するため`implemented`とする。人間が評価の十分性と結論を受け入れるまで`verified`にしない。
- T-004 deterministic Plan lintは現時点で追加実装しない。既存doctor検査で不足する反復的なfalse-negativeが観測された場合だけ再提案する。
- T-005 reconciliation commandは現時点で追加実装しない。まずfixed report運用でstale-mapの再発率を観測する。
- T-006 core completion gateは現時点で追加実装しない。現在のHuman Gate、Evidence、review、doctorを使うprocess gateを維持する。
- T-004からT-006を`dropped`にするには、理由と承認sourceをPlanへ残す人間判断が必要である。

## Outcome

- Phase 6 Agent-first MVPには、実装前または完了前に重要な欠落を表面化する実用上の価値が観測された。
- 追加のfirst-class Plan lint、reconcile command、core completion gateの必要性は現時点では立証されていない。
- 推奨結果は「MVP規約を維持し、追加core機能は実装せず、T-004からT-006を承認付きでdropしてGoal completionを準備する」。
