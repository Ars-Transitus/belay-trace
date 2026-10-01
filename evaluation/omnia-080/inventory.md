# OB-00 — 0.7.0基盤の棚卸し

調査日: 2026-10-01。製品基準: `457136c4c1861742d195c8e881823630ad7ebd00`。
開始HEAD: `df48ef171f1c3f8e602546607af6029037b9097a`。
原本Planは[Omnia–Belay Integration](https://app.notion.com/p/3b8c2ad7b43180d7a0a8d78e58d2392b)。
使用資料: `evaluation/inventory-lifecycle/omnia-plan-source.json`、取得版の編集時刻2026-09-30T12:51:10.865Z。現在のNotion freshnessはUnknown。8月Proposalの記載を現行実装の証拠には使わない。

## 確認方法と境界

- 現在のsrc/tests/Cargo.toml/Cargo.lockと基準commit間のdiffは空。
- release-final.jsonの全product_file_hashesとbinary_sha256を現物のSHA-256と照合し、不一致なし。
- `belay show EVD-20261001T062236-001`と`EVD-20261001T062341-001`で0.7完了記録を確認。240テスト、fmt、Clippy、ブラウザ、隔離deploy等は過去記録。今回再実行したという意味ではない。
- 0.7ストレージv1速度基準は未達。今回限りの人間受入を0.8 OB-EVALへ転用しない。
- local tagに0.8を含むものはなかった。remote番号の未使用は未確認。リリース時に再確認する。
- 既存ハーネス変更は調査対象外として保持。製品/生成元/導入済み設定は編集しない。
- 調査後のEvidence保存で0.7 CLIがroot storage schemaを1から2へ自動移行し、configにlint/verify既定値も直列化した。新Evidenceは独立JSON。初期状態と作業後状態を区別し、以後もtarget/release/belayを使用する。旧writer停止の事前確認は今回できていなかったため、競合なしを保証しない。

## 再利用・不足・移行影響

参照は基準commitと一致する現行ファイル。テスト参照は既存試験の場所であり、新機能の合格Evidenceではない。

| 領域 | 現行の事実とコード参照 | 再利用／追加が必要な点 | 検証参照と移行影響 |
| --- | --- | --- | --- |
| CLI | `src/cli.rs:593` CommandはRoute/context/verify等を持つ。Contract/Capsuleコマンドはない | CLIのvalidation/error出力構造を利用。OB-03はfixtureからvalidate/preview/showの最小縦断 | `tests/cli.rs:4945` Route CLI。コマンド追加は既存引数の互換を保つ。0.8への版変更はOB-REL01 |
| Route | `src/route.rs:481` preview、`:602` applyはpreview hash、入力fingerprint、entry preconditions、receipt replayを検査。`:826`はRoute操作に固有 | hash/前提条件/receiptの設計経験を利用。汎用engineを先に抽出しない。ContractはAssessment/Proposalを必要としない | `tests/cli.rs:4945,5332`、`src/route.rs:2069,2132`。既存Route回帰をOB-04aで維持。hashは許可そのものではない |
| context | `src/context_sections.rs:57` render_sectionsはRequiredを予算内に保持できなければ失敗。`src/context.rs:988` focus | OB-05でContract境界をRequired sectionへ接続。rendererを複製しない。現状にContractの有効性判定はない | `tests/context_sections.rs:80,129,330`。今回4000予算が必要5060に不足して明示失敗。これは新Contractの試験ではない |
| Evidence | `src/evidence.rs:19` schema1、`:296`原本show、`:833` freshnessはcommit/時刻評価。detailはJSON、Entry metadataとは別 | 原本・verifiesリンクを利用。Contract digest/AC対応、固定as-of、取消や変更時の適用性を追加 | `tests/cli.rs:5490,5646,5728`。現在のFreshはsource freshnessや人間の受入を証明しない |
| Coverage | `src/coverage.rs:20,65`はGoalのdecision/implementation/test/monitoring集計、原本index検査 | OB-06は全ACのfresh passing/failed/missing/staleを列挙し、execution/verification/acceptanceを分離。既存集計だけでは不足 | `tests/cli.rs:4809` freshness。古いEvidenceを新revisionへ自動継承しない |
| metadata | `src/entry.rs:303`はscalarのみ。Entry/LinkともBTreeMap | flat provenance keysを使い、詳細AC対応はimmutable artifact。nested metadataは不要 | `src/export.rs:41`出力にmetadataあり。既存scalar読取互換を保つ。キー衝突・手編集検知は追加 |
| inventory/summary | `src/inventory.rs:894,923,957` preview/apply/restore、`src/summary.rs` derived-only source binding | source revision/hash束縛の考え方を利用。summaryをContract原本やverificationの代わりにしない | `tests/inventory_summary.rs`、`tests/cli_inventory_lifecycle.rs`。0.7のarchiveとContract cancelは別概念 |
| 原本保管 | `src/lifecycle.rs:18,121,135`共有writer lock/原本読取/schema移行。`src/pack.rs:734,888,948` apply/recover/restore | 耐久化・path安全性を利用。ただしpackは既存Entry/Evidence向けでContract対応済みではない | `tests/lifecycle_storage.rs:86,183,226,348,433`。新artifactの保管/復元/旧writer拒否を別途検証 |
| sync/rebuild | `src/reconcile.rs:91,754`同期と再構築。`:832`付近は既存DBのRoute receiptsを読む | projection同期とContract整合検査を追加。Contract receiptsはDB喪失後にも復元可能にする案 | `tests/cli.rs:3902,3984,4037`。既存DBからreceipt保持できることと、DBなしから完全復元できることを混同しない |
| export | `src/export.rs:35,59`はEntry snapshotとmetadataを出力し、Evidence indexを検査 | 既存Entry exportは利用可。Contract/source/receipt/Evidenceの完全backupは別形式または別操作が必要 | `tests/cli.rs:4412,4583`。export成功だけでContractの再現可能性を宣言しない |
| Skill生成元 | `src/agent.rs:128,258,542,551`が共通guidanceとinstallを持つ | core機能追加に伴う生成元調整候補。Omnia helper/Skillの配置・配布はOB-07a判断 | `tests/cli.rs:951`等。`.agents`等のconsumer copiesやErwinは今回編集しない |
| deploy | `Makefile`、`scripts/build-local.py`は既存の一時BINDIR配置 | OB-REL01で再利用。作り直し不要 | release-final.jsonのisolated deploy記録。今回installなし |

## OB-01へ渡す結論

1. Contractはimmutable artifact、Goal/Planは投影という案に適合する基礎はある。ただし保管・同期・exportが自動的に対応するわけではない。
2. Routeの機械的保証を参考にできるが、artifact原本とreceiptを失わない復旧契約は追加が必要。
3. Evidence freshness、source freshness、Contract lifecycle、人間の受入は異なる。ひとつのstatusに統合しない。
4. 再利用可能という判断はコード読取に基づく設計判断。新機能の実動保証はOB-02/03のfixtureと後続試験で初めて得る。

## 今回観測した摩擦

- 広いcontext queryが旧Planを含め予算超過した。範囲を絞ったsearch/showと新Planのfocusで進める。無関係な履歴のarchiveやハーネス変更はしない。
- Plan作成はbody必須だったため明示body-fileを使用。製品やSkill修正が必要という証拠ではない。
