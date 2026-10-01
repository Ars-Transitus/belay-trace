---
schema_version: 1
id: PLN-20261001T062923-001-omnia-readiness-delivery
type: plan
title: omnia-readiness-delivery
status: draft
created_at: 2026-10-01T06:29:23+09:00
updated_at: 2026-10-01T21:55:38+09:00
revision: 25
tags: []
links:
- relation: fulfills
  id: GOAL-20261001T062823-001-omnia-integration-readin
metadata: {}
---

## Intent Brief

### Problem
- 0.7完了後の0.8計画はあるが、現在実装との差分と重要設計判断が未固定。
### Desired Outcome
- 確定した判断と評価基準に従い、Omnia–Belay 0.8.0を依存順に実装・検証する。
### Success Signals
- コード・Evidenceへの参照、全lifecycleケース、重要未決事項と停止条件が揃う。
### Constraints
- 基準457136c4c1861742d195c8e881823630ad7ebd00。ユーザーの既存変更を保持。0.7の速度例外をOB-EVAL承認としない。
### Non-goals
- 公開、install、他repository/設定改変、Notion書込。
### Unknowns / Decisions Needed
- 保存/offlineとOB-EVALは承認済み。connector/配布はconnector-capabilities.md。原本 .belay/local-sources/、backup .belay/local-backups/、試行中全世代保持を人間が承認。実際の外部更新先は未確定。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | OB-00 基準付き棚卸し | root | verified | EVD-bf7198c449fa81048640ba8cca761277; evaluation/omnia-080/inventory.md |
| T-002 | SC-002 | OB-01 設計判断案の提示 | root | verified | EVD-bf7198c449fa81048640ba8cca761277; evaluation/omnia-080/design-proposal.md（提案のみ） |

| T-003 | SC-003 | OB-02 schemaとfixture | root | verified | EVD-a9ac3ae6020798e05f124999b94ac3be; EVD-4816022746b6de1bf476934c31a61db4 |
| T-004 | SC-004 | OB-03 read-only縦断 | root | verified | EVD-a9ac3ae6020798e05f124999b94ac3be; EVD-4816022746b6de1bf476934c31a61db4 |

| T-005 | SC-005 | OB-04a common mutation boundary | root | verified | EVD-d91a4e191f36d33a2fac3688d8570813 |

| T-006 | SC-006 | OB-04b Contract apply | root | verified | EVD-87374f59e45aa96674c0c394c3455e1f |
| T-007 | SC-007 | OB-04c interruption recovery | root | verified | EVD-87374f59e45aa96674c0c394c3455e1f |

| T-008 | SC-008 | OB-07a connector設定能力 | root | verified | EVD-875d97b6c9675ea353d28a135608730b |
| T-009 | SC-008 | OB-07b source bundle fixture | root | verified | EVD-f03753eeaf11b46eee95eae1f27ee55f |

| T-010 | SC-009 | OB-05 context/provenance | root | verified | EVD-bfda39aa83a840091d6198f89efa3f96 |

| T-011 | SC-010 | OB-08a requirements intake | root | verified | EVD-16cdb9cd12288ec3a742ffaddc505aff; isolated scenarios only |
| T-012 | SC-010 | OB-08b confirmed input | root | verified | EVD-16cdb9cd12288ec3a742ffaddc505aff; no live Notion mutation |

| T-013 | SC-011 | OB-07c ID ledger | root | verified | EVD-bc1a8966383f00c08c8f40156c913e0d |

| T-014 | SC-012 | OB-06 Outcome Capsule | root | verified | EVD-0e3c6483634deebdc7d11abe1bce54ea |

| T-015 | SC-013 | OB-09 execution/resume skill | root | verified | EVD-a1f3bd9d65a9a4047334f23a78db0b56 |
| T-016 | SC-014 | OB-10a operation ledger | root | verified | EVD-99e35c75c5a7aaa0a863520fd86e5506 |
| T-017 | SC-014 | OB-10b reconcile recovery | root | verified | EVD-99e35c75c5a7aaa0a863520fd86e5506 |
| T-018 | SC-014 | OB-10c consolidate skill | root | verified | EVD-eb5979e597cf2a415297e4cd9087f45d |
| T-019 | SC-015 | OB-11 real dogfood | root | in-progress | local code all4 AC verified, EVD-d3491482aa70a77f9d29d990a94e000d; Notion append applied; human time unmeasured; explicit user delivery authorization recorded in evaluation/omnia-080/delivery-decision.md |
| T-020 | SC-016 | OB-REL01 release preparation | root | in-progress | user authorized delivery with unmeasured human time; final checks and PR pending |

| T-021 | SC-013 | Dogfood Work link compatibility | root | verified | EVD-5e0fcd70d42f50d0597364b2f93b1cd4; actual Contract Work created |

## T-001
- Objective: OB-00の再利用・不足・移行影響を固定する。
- Scope: src/tests/evaluationの読取、evaluation/omnia-080と新規Belay記録の作成。基準457136c4c1861742d195c8e881823630ad7ebd00。
- Steps: 引継ぎ・snapshot・release Evidenceを読む。CLI/Route/context/Evidence/metadata/sync/rebuild/export/Skill生成元を照合する。
- Acceptance: 各主張にコード/検証参照、再実行と過去記録の区別、移行不足がある。
- Verification: hash比較、diff、参照検査。依存BI-REL01は既存Evidenceで確認。
- Stop: 基準の不一致または既存変更との衝突は記録して再判断。

## T-002
- Objective: OB-01の具体的な設計案と判断待ちを提示する。OB-01全体の設計確定とは区別する。
- Scope: R03–06/R09/R12、文書のみ。同じ基準commitを使用。
- Steps: T-001後、authority/lifecycle/hash/freshness/storageの案と代替案を比較する。
- Acceptance: retry/new revision/別委譲/cancel/expire/supersede/offline/手編集、backup/rebuild/export/secretの挙動と停止条件を具体化。
- Verification: ケースと要件の文書チェック。提案を承認Evidenceとして扱わない。
- Stop: 重要未決に依存するOB-02以降の実装へ進まない。

## T-003
- Objective: OB-02: strict schema、未知field/必須不足/不正状態を拒否する合成fixture。
- Scope: scripts/omnia_contract.py、tests/omnia_contract、evaluation/omnia-080。基準457136c4c1861742d195c8e881823630ad7ebd00。
- Steps: design-proposal.mdの確定事項を実装し、正常/不正/意味変更/取消/期限fixtureを検証。
- Acceptance: 未確定入力を発行せず、未知fieldを拒否、同一入力は同一bytes。外部/Belay mutationなし。
- Verification: Python unittest、入力/出力golden比較。
- Stop: OB-EVAL未承認のままapply/source取得を実装しない。

## T-004
- Objective: OB-03: 確定入力から同一canonical Contract/hashとGoal/Plan previewを生成するread-only helper。
- Scope: scripts/omnia_contract.py、tests/omnia_contract、evaluation/omnia-080。基準457136c4c1861742d195c8e881823630ad7ebd00。
- Steps: design-proposal.mdの確定事項を実装し、正常/不正/意味変更/取消/期限fixtureを検証。
- Acceptance: 未確定入力を発行せず、未知fieldを拒否、同一入力は同一bytes。外部/Belay mutationなし。
- Verification: Python unittest、入力/出力golden比較。
- Stop: OB-EVAL未承認のままapply/source取得を実装しない。

## Reconciliation
- OB-02/03 synthetic read-only slice verified; full release/real source/complete evaluator not verified.
- Next: human OB-EVAL case and ceilings in evaluation/omnia-080/evaluation-proposal.md. OB-04a/07b numerical gate approved by EVD-ae60bd4fda1b275020c5298a54405199.
- Local snapshot/backup configuration approved in this chat; actual external-write destination remains unapproved.

## T-005
- Objective: OB-04aの最小共通mutation境界を抽出。OB-EVALはEVD-ae60bd4fda1b275020c5298a54405199で承認済み。
- Scope: src/mutation.rs、src/route.rs、src/lib.rs、必要なfocused tests。
- Steps: preview digestと前提検査、receipt identityの共通性を調べ、実際に共通な機械的処理だけ抽出。
- Acceptance: Route固有Assessment/Proposalを持ち込まない。既存preview hash互換とRoute retry/precondition回帰を維持。Contract永続化を先走らない。
- Verification: 既存Routeテストと共通関数の反例検証。
- Stop: 実データmutation、別repository変更、公開は行わない。

## T-006
- Objective: OB-04b Contract validate/preview/apply/showと原本・投影・receipt保存。
- Scope: src/contract.rs、CLI/lib接続、必要なstore/config互換処理、tests/contract。
- Steps: apply-design.mdと確定Contract schemaを基にRust coreで検証し隔離repositoryで実装・検証。
- Acceptance: 同一適用no-op、target/preview/同一版異内容拒否。原本/投影/receipt中断復旧、改変は停止、新版は別投影。
- Verification: Rust tests with synthetic fixtures and fault injection; Python goldenとのcanonical一致。
- Stop: 実Notion取得/書込、production store移行、公開は行わない。

## T-007
- Objective: OB-04c 各書込境界への中断注入と重複なし復旧。
- Scope: src/contract.rs、CLI/lib接続、必要なstore/config互換処理、tests/contract。
- Steps: apply-design.mdと確定Contract schemaを基にRust coreで検証し隔離repositoryで実装・検証。
- Acceptance: 同一適用no-op、target/preview/同一版異内容拒否。原本/投影/receipt中断復旧、改変は停止、新版は別投影。
- Verification: Rust tests with synthetic fixtures and fault injection; Python goldenとのcanonical一致。
- Stop: 実Notion取得/書込、production store移行、公開は行わない。

## T-008
- Objective: OB-07a connector能力と配置/配布判断。
- Scope: scripts/omnia_source.py、tests/omnia_source、evaluation/omnia-080/connector-capabilities.md。
- Steps: exact Area/data-source/repository/connector設定と取得envelopeを検証。
- Acceptance: 権限不足/未知block/削除/archive/relation循環/pagination不全を拒否。実source取得はbackup設定と明示対象の条件を維持。
- Verification: 合成fixtureとtool schemaの能力検査。
- Stop: 外部書込/実snapshot複製/consumer変更は不可。

## T-009
- Objective: OB-07b 明示選択source bundleの境界/完全性/原本hash検証。
- Scope: scripts/omnia_source.py、tests/omnia_source、evaluation/omnia-080/connector-capabilities.md。
- Steps: exact Area/data-source/repository/connector設定と取得envelopeを検証。
- Acceptance: 権限不足/未知block/削除/archive/relation循環/pagination不全を拒否。実source取得はbackup設定と明示対象の条件を維持。
- Verification: 合成fixtureとtool schemaの能力検査。
- Stop: 外部書込/実snapshot複製/consumer変更は不可。

## T-010
- Objective: Contract context/provenance/AC mappingとeditable projectionを接続する。
- Scope: src/contract.rs context.rs context_sections.rs必要最小CLI接続とtests。
- Steps: ContextSection required rendererを再利用し、固定IntentとTask進捗を分離。sync/rebuild/export保持を隔離fixtureで確認。
- Acceptance: Contract境界が先頭で必須。予算不足は明示失敗。欠損/改変/期限/取消/supersedeを報告。Task進捗変更後も同じreceiptから再読取可能。
- Verification: focused Contract/context tests plus legacy regression.
- Stop: 外部書込/実データ移行/公開は不可。

## T-011
- Objective: OB-08a Idea受付とGoal選択/作成。
- Scope: skills/omnia-requirements/SKILL.mdと必要reference。
- Steps: exact source選択と既存ID照合、候補が曖昧なら選択依頼。draft/confirmedと作成/更新を区別。
- Acceptance: 無条件Goal重複作成なし、必須未決ならdraft停止。既存具体指示は再承認不要。
- Verification: skill validatorと隔離read-only scenario review。
- Stop: 外部実更新、consumer installは行わない。

## T-012
- Objective: OB-08b Plan要件化と確定入力への接続。
- Scope: skills/omnia-requirements/SKILL.mdと必要reference。
- Steps: exact source選択と既存ID照合、候補が曖昧なら選択依頼。draft/confirmedと作成/更新を区別。
- Acceptance: 無条件Goal重複作成なし、必須未決ならdraft停止。既存具体指示は再承認不要。
- Verification: skill validatorと隔離read-only scenario review。
- Stop: 外部実更新、consumer installは行わない。

## T-013
- Objective: receiptと原本からID台帳を決定論的に再構築・照合する。
- Scope: scripts/omnia_ledger.py tests/omnia_ledger。
- Steps: Contract/receipt/source IDsと期待repositoryを束縛し既存台帳との差分を生成。
- Acceptance: 欠損・異版・同一版異内容を検出し、外部Id/URLを書き換えない。
- Verification: 合成receipt/Contract fixtureによる再試行・欠損・衝突試験。
- Stop: Notion書込/実store変更は不可。


## T-014
- Objective: OB-06 全ACの証拠状態と独立した3状態軸を返す。
- Scope: src/capsule.rs、src/cli.rs/lib.rs接続、tests/contract_capsule.rs。既存Evidenceを再利用。
- Steps: T-010レビュー合格後、capsule-design.mdに従い固定as-of、厳密SC参照、canonical出力を実装。
- Acceptance: 不足/古い/失敗証拠からverifiedを作らず、人間承認を推測しない。全ACを列挙。外部呼出なし。
- Verification: 隔離storeの欠損/混在/失敗/未来時刻/期限/反復一致とPython schema互換。
- Stop: 実store移行、外部更新、根拠のないacceptedは禁止。

## T-015
- Objective: OB-09 execution/resume skill。
- Scope: skills/omnia-execution/。
- Steps: T-010/T-013/T-011/T-012完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: 既存deliveryへのcompact pointer、固定Contract再開、変更/取消/期限/offline停止。
- Verification: 隔離resume scenario review。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-016
- Objective: OB-10a operation ledger。
- Scope: scripts/omnia_consolidate.py tests/omnia_consolidate/。
- Steps: T-014/T-015/T-013完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: 対象/差分/Capsule digest/操作ID/許可根拠を束縛しpending/applied/conflict/unknownを区別。
- Verification: 決定論的operation fixture。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-017
- Objective: OB-10b reconcile recovery。
- Scope: scripts/omnia_consolidate.py tests/omnia_consolidate/。
- Steps: T-016完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: 専用結果領域追記とreadback照合。応答喪失はunknown、盲目的再送なし。単一writer限界を記載。
- Verification: 応答喪失/部分成功/競合/retry fixture。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-018
- Objective: OB-10c consolidate skill。
- Scope: skills/omnia-consolidate/。
- Steps: T-017完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: Decision昇格/completedは具体的人間判断。許可/手動反映待ち/未確認を区別。
- Verification: 隔離scenario review。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-019
- Objective: OB-11 real dogfood。
- Scope: evaluation/omnia-080/; isolated dogfood checkout; approved local source storage。
- Steps: T-018/OB-EVAL完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: 実Code案件1件の全経路と品質条件/運用上限を測定。具体的外部更新先は別途承認が必要。
- Verification: evaluation-v1.md と dogfood-case.json。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-020
- Objective: OB-REL01 release preparation。
- Scope: Cargo.toml Cargo.lock version tests release documentation。
- Steps: T-019 adoption pass完了後、正本Planと既存設計に従い実装・検証。
- Acceptance: 0.8.0版一致、0.7既存機能互換、一時BINDIR deploy、Evidenceに一致する文書。
- Verification: 既存回帰suite、metadata/version/temp deploy。
- Stop: native authorityを維持。tag/push/公開/install/Erwin変更禁止。未承認外部書込は禁止。

## T-021
- Objective: Contract projectionの既存implements Goal linkをWork作成へ接続。
- Scope: src/store.rs、focused testsのみ。immutable projection変更なし。
- Steps: Plan Goal解決でfulfillsとimplementsを一貫して認識し、曖昧性/fragment制約は維持。
- Acceptance: Contract apply後に既存work create --taskが成功しGoal SCへ束縛される。重複同一Goal linksは正規化し、異なるGoalの曖昧性は拒否。
- Verification: 実Contract CLIからWork作成する回帰試験と既存Work tests。
- Stop: 保存済みContract/receiptの書換、他repository/外部変更禁止。
