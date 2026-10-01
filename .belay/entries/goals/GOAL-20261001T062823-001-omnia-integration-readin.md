---
schema_version: 1
id: GOAL-20261001T062823-001-omnia-integration-readin
type: goal
title: omnia-integration-readiness
status: draft
created_at: 2026-10-01T06:28:23+09:00
updated_at: 2026-10-01T20:20:36+09:00
revision: 12
tags: []
links: []
metadata: {}
---

## Summary

- Omnia–Belay 0.8.0着手のため、OB-00棚卸しとOB-01設計判断案を証拠付きで整える。段階的に同じGoalを拡張し、OB-EVAL承認済み。OB-04aから依存順に進める。

## Success Criteria

- [SC-001] 基準commit付き再利用・不足・移行影響表を作り、コードと既存Evidenceを照合する。
- [SC-002] lifecycle・authority・storageの具体案と重要未決事項、後続の停止条件を提示する。提案の作成と設計承認を区別する。

- [SC-003] 確定設計に従う入力/Contract/Capsule schemaと正常・不正fixtureを備える。
- [SC-004] 合成確定入力からContractとGoal/Plan previewを決定論的に生成し、repository/Notionを変更しない。

- [SC-005] OB-04a: RouteとContractに共通するpreview digest/前提条件の境界を最小抽出し、既存Routeを回帰させない。

- [SC-006] OB-04b: Contract原本・投影・receiptを保存し、target/preview/版衝突を拒否する。
- [SC-007] OB-04c: 各書込境界の中断後、同じ原本と投影IDから重複なく復旧または明示停止する。

- [SC-008] OB-07a/b: 設定境界と取得完全性を検査し、不完全sourceからの発行を拒否する。

- [SC-009] OB-05: Contract境界とprovenance/AC対応をcontext必須先頭に保持し、予算不足・改変・欠損を拒否する。Task進捗は固定Intentと分離する。

- [SC-010] OB-08a/b: IdeaからGoal選択/作成、Plan更新/作成を区別し、要件ID/出典/AC/境界を確定入力へ結ぶSkillを提供する。

- [SC-011] OB-07c: repository/source/Contract/Belay投影/receiptの対応台帳を照合し、既存外部Id/URLを上書きしない。

- [SC-012] OB-06: 固定as-ofで全ACのEvidence状態を決定論的に集計し、execution/verification/human acceptanceを分離したCapsuleを返す。

- [SC-013] OB-09: 同じContract/Work/Evidenceから再開し、変更/取消/期限/offlineを確認して既存deliveryへ接続するSkill。
- [SC-014] OB-10: Capsuleに束縛した操作台帳と専用結果領域への追記/照合手順を持ち、結果不明を成功扱いせず盲目的に再送しない。
- [SC-015] OB-11: 承認済み実Code案件と固定異常試験を完走し、品質条件と運用上限で導入判定する。
- [SC-016] OB-REL01: OB-11導入可後に版/互換/一時BINDIRを検証し、事実に一致するrelease notesを提供する。公開は含めない。

## Constraints

- 基準457136c4c1861742d195c8e881823630ad7ebd00。既存ハーネス変更を保持。0.7速度例外はOB-EVALへ継承しない。

## Non-goals

- push/tag/公開、ユーザーローカルinstall、Erwin/導入済み設定の変更、Notion書込。

## Verification

- source/hash照合、参照箇所確認、Goal/Plan lint、文書受入項目の検査をEvidenceに記録する。

## Risks

- 正本Planを再取得し編集時刻2026-09-30T12:51:10.865Zを確認。将来のfreshnessは別確認。保存/offline、ローカルbackup全世代保持、OB-EVALは承認済み。実際の外部更新先は未確定。
