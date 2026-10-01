## Intent Brief

### Problem
- 0.7完了後の0.8計画はあるが、現在実装との差分と重要設計判断が未固定。
### Desired Outcome
- OB-00の事実表とOB-01のレビュー可能な判断案を作る。
### Success Signals
- コード・Evidenceへの参照、全lifecycleケース、重要未決事項と停止条件が揃う。
### Constraints
- 基準457136c4c1861742d195c8e881823630ad7ebd00。ユーザーの既存変更を保持。0.7の速度例外をOB-EVAL承認としない。
### Non-goals
- 公開、install、他repository/設定改変、Notion書込。
### Unknowns / Decisions Needed
- 保存/offlineは音声承認で確定。実source backup設定、connector/配布、OB-EVAL数値は未確定。OB-04a/07b前に人間が評価対象と数値を確定する。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | OB-00 基準付き棚卸し | root | verified | EVD-bf7198c449fa81048640ba8cca761277; evaluation/omnia-080/inventory.md |
| T-002 | SC-002 | OB-01 設計判断案の提示 | root | verified | EVD-bf7198c449fa81048640ba8cca761277; evaluation/omnia-080/design-proposal.md（提案のみ） |

| T-003 | SC-003 | OB-02 schemaとfixture | root | verified | EVD-a9ac3ae6020798e05f124999b94ac3be; EVD-4816022746b6de1bf476934c31a61db4 |
| T-004 | SC-004 | OB-03 read-only縦断 | root | verified | EVD-a9ac3ae6020798e05f124999b94ac3be; EVD-4816022746b6de1bf476934c31a61db4 |

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
- Next: human OB-EVAL case and ceilings in evaluation/omnia-080/evaluation-proposal.md. OB-04a/07b blocked until explicit decision.
- Full source backup configuration and actual external-write destination remain unapproved.
