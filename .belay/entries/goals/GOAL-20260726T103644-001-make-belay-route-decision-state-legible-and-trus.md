---
schema_version: 1
id: GOAL-20260726T103644-001-make-belay-route-decision-state-legible-and-trus
type: goal
title: Make Belay Route decision state legible and trustworthy in Browse
status: draft
created_at: 2026-07-26T10:36:44+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Summary

- Belay Routeが構成した暫定的な分析、経路候補、根拠、Unknown、人間の応答、再統合結果を、Browseから誤解なく確認できる状態にする。
- Browse対応はRouteの中核推論や有効性仮説とは分離し、Routeの固定出力契約が成立した後に、その判断面を人間が理解、検証、追跡するためのUXとして提供する。

## Success Criteria

- [SC-001] Browseは、versionedなRoute出力契約から、対象、参照したBelay entryとEvidence、Fact、Assumption、Hypothesis、Unknown、Conflict、経路候補、停止または追加検証提案を表示し、各情報の分類と参照元を辿れる。
- [SC-002] Browseは、Routeの暫定Proposal、保留中の人間判断、人間が採用、修正、却下または保留した内容、Belayへ正式記録された内容を視覚的かつ意味的に区別し、Proposalを承認済みの事実または実装開始許可として誤認させない。
- [SC-003] 人間は、重要なConstraint、主要Risk、未検証事項、excludedまたはdeferred scope、保証されていない事項、必要なHuman Decisionを、全Route出力を逐語的に読まなくても発見し、関連する原文またはEvidenceへ移動できる。
- [SC-004] Browseは、Route実行後のReconciliation Resultから、GoalとSuccess Criterionへの対応、実装済みと検証済みの差、新しいDecision、変更されたAssumption、新しいUnknown、未承認または未解決の範囲を確認できる。
- [SC-005] Route情報が存在しない、失効している、入力Belay stateよりstaleである、schema versionが未対応である、参照先が欠落している場合、Browseは推測で補完せず、その状態と制約を明示する。
- [SC-006] Route対応画面は通常リンクとkeyboardで主要情報と参照先へ到達でき、canvas、色、hover、pointer操作だけへ依存しない。代表的なRoute fixtureを用いたアクセシビリティ、レスポンシブ表示、情報優先順位の人間レビューを通過する。

## Constraints

- このGoalはRoute protocol、推論品質、候補生成、評価方法を定義しない。GOAL-20260726T014020-001またはその後継Goalが定めるversioned inputとoutput contractを利用する。
- Belayを正式な事実、判断、履歴、EvidenceのSource of Truthとして維持し、Browse内のRoute表示を競合するSource of Truthにしない。
- 初期のBrowse Route対応はread-onlyとする。承認、修正、materializationその他の書き込み操作をBrowseへ追加する場合は、別のDecision、Goal、Plan、人間承認を必要とする。
- BrowseまたはBelay coreからLLMを呼び出さない。Routeが生成済みの構造化出力を決定的に検証、表示、関連付けする。
- Route出力のschemaをBrowse固有の都合で決定せず、Route contractとBrowse presentationを分離する。
- 重要事項を量の中へ埋めない。全文表示より、判断に必要なConstraint、Risk、Unknown、非保証範囲、Human Decisionを優先する。

## Non-goals

- Browse内でRoute Proposalを生成すること。
- BrowseからRoute Proposalを採用、承認、修正、却下すること。
- Routeの有効性またはモデル能力を評価すること。
- Route ProposalをBelayの正式なtrace entryとして扱うこと。
- Product Map、Requirement、Route Proposalその他のfirst-class persistence modelをこのGoalだけで決定すること。
- 法人向けホスティング、RBAC、組織横断dashboardを実装すること。
- 既存Browseの全UXまたはExplore graphを再設計すること。

## Verification

- Fact、Assumption、Hypothesis、Unknown、Conflict、複数候補、stop、insufficient-context、stale、unsupported-schema、missing-referenceを含む固定Route fixtureを用意する。
- fixtureから生成したBrowse表示について、分類ラベル、provenance link、暫定状態と正式記録の区別、重要事項の優先順位を検証する。
- 通常リンク、keyboard navigation、screen reader向け名称、色以外の状態表現、狭いviewportでの表示を自動検査と人間レビューで確認する。
- Route ProposalからBelayへ採用済み内容が記録されたfixtureで、Proposal、Human Response、正式entry、Reconciliation Resultの関係をfresh contextのreviewerが再構築できるか確認する。
- Route出力が存在しない通常repositoryで、既存BrowseのReader、Explore、Evidence、Git provenance、read-only動作が回帰しないことを確認する。

## Risks

- Route情報を追加することでBrowseの情報量が増え、重要判断がかえって見つけにくくなる。
- 洗練された表示が暫定Proposalへ不当な確実性または権威を与える。
- Route schemaが未確定の段階でBrowseを設計し、表示都合がcore contractを歪める。
- read-only表示と将来の承認UIの境界が曖昧になり、既存Human Gateを誤解させる。
- Route表示のためにBrowseがRoute固有persistenceへ直接結合し、Belayとの分離可能性を失う。

## Assumptions

- **This is a hypothesis.** 構造化されたRoute出力が存在しても、CLIまたはraw dataだけでは人間が重要判断とprovenanceを効率よく把握できず、Browseによる情報設計に追加価値がある。
- **This is a hypothesis.** 初期検証ではread-only表示で十分に価値を観測でき、承認操作をBrowseへ入れなくてもRouteの判断面を評価できる。

## Unknowns / Decisions Needed

- Route出力が永続化される場所と、Browseが安全に読み取る公開interface。
- Route Proposal、Human Response、Reconciliation Resultを同一画面、時系列、または別画面のどれで表現するか。
- stale判定に使用するBelay state fingerprint、entry revision、commitまたは時刻の契約。
- 人間の判断操作を将来Browseへ追加するか、Route固有UIへ分離したままにするか。
- Route情報をReader、Explore、専用Route viewのどこへ配置するか。
- 判断可能性を改善したとみなすUX評価方法と合格基準。
