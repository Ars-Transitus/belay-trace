# OB-01 — Authority・lifecycle・保管の判断案

状態: **OB-01確定。** 2026-10-01。保存/offline方針は本チャット音声で人間が確定。その他の機械的設計は実装担当判断。
基準と再利用根拠は[inventory.md](inventory.md)。要件R03–06/R09/R12に対応する。
OB-02/03は合成fixtureに限定して進める。実source取込はbackup先・保持期間・復元手順の設定後。OB-04a/07bはOB-EVALの人間確定後。

## Authorityと状態

- Omniaは人間のIntent、固定Contractはその時点の実行境界、BelayはTask/Work/Evidence、repositoryはActual State。
- Contractの署名や独自権限基盤は導入しない。digestは内容束縛であり許可ではない。既存の具体的な人間の許可は同じ操作・対象に再利用し、native sandbox/approvalを上書きしない。
- 実行軸: not-started/running/paused/completed/cancelled。検証軸: unverified/partial/verified/failed。受入軸: pending/accepted/rejected。これはschema候補でありenumの最終確定ではない。
- Contract lifecycle（active/cancelled/expired/superseded）とsource freshness（observed-current/changed/unknown）は上記3軸とは別。技術verifiedでもacceptedにせず、Omnia completed変更の許可にもならない。
- 人間の受入は対象Contract版・payload digest・成果物/Evidence集合・具体的操作を参照する記録を必要とする。自由文kind=human-approvalだけで自動認定しない。

## ケースごとの操作・停止条件

| ケース | 推奨挙動と具体例 | 許可操作・停止条件 |
| --- | --- | --- |
| retry | C1/r1、同じpayload・issued_at・compiler・targetを再送。receiptから同じ投影IDsを返す | 完全一致ならno-op。書込途中ならreceipt/原本/preconditionsを照合して残りだけ回復。欠損・不一致を成功扱いしない |
| 同一版異内容 | C1/r1のACだけ変えたpayload | 衝突で拒否。元payload上書きも自動revision増分も行わない |
| 新revision | 同じ委譲系列C1でAC変更を確定しr2を発行 | 新Goal/Plan投影とAC対応を作る。r1成果物/Evidenceは保持。新発行だけでr1取消を推測せず、実行切替は明示supersedeに束縛 |
| 別委譲 | 同じOmnia Planでも異なるrepositoryや独立実行目的 | C2を発行。URL一致を重複判定キーにしない。targetが元previewと違えば拒否 |
| cancel | 明示取消イベントをC1/r1に記録 | 次のmutation/dispatch前に停止。読取・検証・安全な照合は可。進行中の外部操作を取り消せたと主張しない。取消解除で原本を書換えず、新委譲判断へ |
| expire | expires_atに到達（UTC instant比較、境界はnow >= expires_at） | mutation/dispatch停止。期限を再試行時に延長しない。時計が信頼できなければUnknownで停止し、期限変更は新版 |
| supersede | r2へ移る具体判断をr1とr2双方に結ぶ | r1の新規dispatch停止。既存Workを完了扱いしない。停止確認と未確定外部操作の照合後にr2をactive化。跨る成果物はレビューして採用判断 |
| offline再開 | 原本C1/r1とreceiptを検証できるがOmniaへ接続不能 | source freshness=unknown。読取/ローカル検証は可。mutation継続は下記D2で確定したoffline方針と既存許可・期限・取消観測に依存。方針なしなら停止 |
| source更新 | adapterが取得したページhashが変更 | source freshness=changed、差分を提示。旧snapshotを自動更新/無効化しない。境界に影響するか未判断なら新規dispatchを止め再compile判断へ |
| 投影手編集 | Task分解の追記と、Contract由来AC/Constraints改変を区別 | Task進捗・Work/Evidence追加は可。Intent由来領域やprovenanceのhash不一致はdriftで停止。手編集をOmniaへ逆流させず、新確定入力へ戻す |

取消/supersedeをcoreが知るのはローカルに観測記録がある場合だけ。offline中の遠隔取消を即時検知できる保証はしない。停止要求と実際のworker停止確認も区別する。

## 同一性・hash・決定論性

- IDは委譲系列、revisionは意味の版、payload digestは内容。targetは設定済repository identityと照合する。cloneのabsolute pathを恒久IDにしない。
- source bundle: 実際に利用した各page ID、URL、取得/編集時刻、必要部分snapshot、取得範囲・完全性、正規化内容hash。アクセス不能/未知block/欠落を成功扱いせず、必須source不完全なら発行不可。
- raw snapshot hash（保存bytes）とnormalized content hash（比較対象）、bundle hash（manifestと参照hash）、Contract payload hash（schema/compiler/固定発行情報/target/要件・AC/境界/source bundle digestを含む）を区別する。
- canonical JSON候補: key辞書順、UTF-8、整数のみの機械数値、未知field拒否、array順序維持、時刻UTC固定表現。本文の空白やUnicodeを意味判断なしに変更しない。digest自身をpayload hashから除外する。OB-02でgolden bytesを固定する。
- preview digestはContract digest、target、投影差分、前提revision/hash、操作IDを束縛。作成後に対象が変われば再preview。
- Capsuleは固定evaluation state/as-of、Contract digest、AC対応、Evidence IDと原本hash、3状態軸を入力とする。現在時刻を暗黙に差し込まず、再評価は別artifact。missing/stale/failedをverifiedへ丸めない。
- provenance候補はflatな `contract_id` / `contract_revision` / `contract_digest` / `source_bundle_digest` / `projection_digest`。AC→SC→Evidence対応表は別artifactに置き、metadataのnested化を先行しない。

## 保管・復元・秘密情報（D1の提案）

推奨: 機微な実source snapshotは既定でGit非追跡の原本領域、公開用fixtureだけ追跡。原本はcacheではなく、独立したbackup対象とする。**既存 `.belay/.gitignore` はstate等のみを除外しており、この提案の保護は現在未実装。** 今回はsourceを新たに取得・複製しない。

- 保存単位はContract payload、source bundle、確定入力、AC対応、lifecycle events、apply receipts。投影とEvidence/packを含めて復元セットを定義する。DBだけをbackupとは呼ばない。
- 書込は共有writer lock、同一filesystem内の一時書込→検証→原本公開→投影/receipt確定の各境界を定義。OB-04cで各境界へ中断注入。原本とreceiptを先に保証し、派生indexは再構築できる形を目指す。
- DBを削除した隔離fixtureから、原本・投影・receipt・Evidenceを照合して同じIDs/AC対応を再構築できることを受入条件にする。不足時は明示停止しreceiptを捏造しない。
- Entry exportの従来動作は維持。別の明示的backup/export操作でartifact inclusion manifestとhashを出す案。原本を含まないredacted exportは「非復元用」と明記し、同じdigestの完全backupを装わない。
- secret/token/credentialは取得・保存しない。必要部分の明示選択と確認を使う。自動secret検出だけで秘密情報なしを保証しない。保存後に発見した場合は停止・公開しない・別途削除/失効判断へ。immutableを理由に秘密保持問題を放置しない。
- Git追跡しない場合は機械喪失時の復元用backupが不可欠。暗号化/保管先/保持期間は利用者環境に依存し、未設定のまま本番sourceを取り込まない。
- root storage schema、artifact schema、compiler、Belay版を分離。0.7 writerが新Contract原本を無視して投影を変えるのを防ぐ読書き互換判定が必要。Contract対応schema移行はOB-02/04で設計する。今回のEvidence保存では既存0.7 CLIがroot storage schemaを1から2へ自動移行した。これはContract実装ではない。旧CLI writerは使用せず、downgradeを保証しない。

## 決めるべき点と代替案

| ID | 推奨案 | 有力な代替・trade-off | 判断時点 |
| --- | --- | --- | --- |
| D1 保存とGit | 実snapshotは既定非追跡＋明示backup。fixtureのみ追跡 | 承認済みsanitized snapshotをGit追跡すれば共有/復元は容易だが、機微情報と履歴残存リスクが増す | OB-01確定前。保管先・保持・backup責任を具体化して選ぶ |
| D2 offline | 既定は読取/検証のみ。実行を許す場合は発行時に範囲と最大継続時間を明示 | 常時再取得必須は失効検知を強めるがネットワーク依存。常時offline許可は遠隔取消の遅延を受け入れる | OB-01確定前。時間値は未決で勝手に置かない |
| D3 projection | Intent由来fieldはhash固定、Task分解/進捗は編集可能 | 全文固定は簡単だが通常のBelay作業を阻害する。自由編集はIntent境界が弱い | OB-01確定前。OB-02でprotected field一覧をfixture化 |
| D4 connector/配布 | 能力表を作り、小さいhelperをcore外へ | ntn/connectorの更新・pagination・冪等性の実能力はUnknown。配置repositoryも未決 | OB-07a。別repositoryや導入設定変更を現承認から推測しない |
| D5 OB-EVAL | 固定Code案件と数値上限を人間の運用責任者が確定 | 過去baselineが再現不能な指標は改善比較から除外 | OB-03後、OB-04a/07b前。0.7の例外は流用不可 |

D1/D2は保持・運用リスクの選択であり、今回の「開始」依頼だけで確定済みとみなさない。D3は実装可能性が高い推奨案だが、まだfixtureで検証していない。

## 次の検証と停止線

OB-01判断確定 → OB-02正常/衝突/取消/期限/不足/手編集のfixtures → OB-03読取だけの縦断 → OB-EVAL → OB-04a/07b。
各fixtureには期待する3状態軸、lifecycle/freshness、許可操作、停止理由、hashとreceiptを付ける。
DB喪失、原本欠損、同一版異payload、旧writer、source更新、offline取消未観測を必須ケースにする。
OB-07a未確定でもsynthetic fixtureによるcore境界検証は設計可能だが、実Notion取得や外部書込は行わない。

## 確定事項（2026-10-01、音声承認）

- D1: Gitには目的・全AC・制約・禁止事項・停止条件・出典・原本digestを含むBelay実行記録を残す。機密情報は含めない。原本snapshotはGit非追跡のローカル領域に保持する。要約は原本の完全backupの代わりではない。実sourceの取込はbackup先・保持期間・復元手順が明示設定されるまで停止。合成fixtureは追跡可能。
- D2: Notionのみの障害も含め、sourceを確認できない間は変更/新規dispatchを停止。読取と外部変更を伴わないローカル検証は可。復旧時に取消・変更・期限と既存Workを照合し、問題がない場合のみ同じContractで再開。実行中処理を即時取消できる保証はしない。
- D3（実装担当判断）: Outcome/AC/Constraints/Non-goals/delegation/verification/stop conditions/source/provenanceを固定。Task分解と進捗、Work/Evidence追加は可。固定領域の編集はdriftとして停止。
- D4はOB-07aで決定。D5はOB-03後の人間判断を維持。
- 上記は以前のD1/D2未決記載を置き換える。提案時の本文は比較用に保持。

## Confirmed trial storage follow-up

Human approved the concrete local trial configuration in
EVD-5149dedd040e9dce8e56c7ac0187c746: original `.belay/local-sources/`,
backup `.belay/local-backups/`, both Git-untracked, retain all trial generations.
This deliberately accepts same-disk recovery only; earlier disaster-recovery
recommendations are not an additional approval gate for this personal trial.
Recovery: stop dispatch, select the exact recorded generation, verify canonical
source hashes against its manifest/Contract, restore matching bytes from backup,
then refetch source validity before further changes. Never rewrite the immutable
execution snapshot to match a newly edited source. See local-storage.md.
