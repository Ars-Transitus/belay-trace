Here is the result of "fetch" for the Page with URL https://app.notion.com/p/3ebc2ad7b43181df94b6c2629c3ec3d2 as of 2026-09-30T12:46:14.454Z:
<page url="https://app.notion.com/p/3ebc2ad7b43181df94b6c2629c3ec3d2" icon="icons/compass_purple">
<ancestor-path>
<parent-data-source url="collection://3b6c2ad7-b431-80b4-bebe-000b0cd1c810" name="Scriptorium"/>
<ancestor-2-database url="https://app.notion.com/p/3b6c2ad7b4318018a774c7b0cf606c6a" title=""/>
</ancestor-path>
<properties>
{"Archived":"__NO__","Created time":"2026-09-30T11:50:09.706Z","Domain":"Ars Transitus","Last edited time":"2026-09-30T12:46:14.454Z","Name":"Belayデータ棚卸し・Context改善 Plan","Parent item":["https://app.notion.com/p/3b6c2ad7b43180e3ac61dda12843b98b"],"Priority":"High","Related Items":["https://app.notion.com/p/3c2c2ad7b43180409086cb0867586272","https://app.notion.com/p/3b8c2ad7b43180d7a0a8d78e58d2392b"],"Status":"draft","Sub-item":["https://app.notion.com/p/3ebc2ad7b43181f28f89d2d3d2ee78d6","https://app.notion.com/p/3ebc2ad7b431815b8d1cfb95ed1d6294"],"Subtype":"Code","Summary":"棚卸し・Context改善に加え、並列Evidence記録、出典付き要約、原本を保持したpack化・復元・保持方針を整備。BI-00〜07、BI-LC00〜06。","Type":"Plan","date:Due:is_datetime":0,"url":"https://app.notion.com/p/3ebc2ad7b43181df94b6c2629c3ec3d2","userDefined:Id":"","userDefined:URL":""}
</properties>
<iconMetadata>{"type":"icon","icon":{"name":"compass","color":"purple"}}</iconMetadata>
<content>
## Approach — データを失わず、現在の有用な情報へ到達する
2026-09-30作成。状態：実装計画案。原案：<mention-page url="https://app.notion.com/p/3c2c2ad7b43180409086cb0867586272"/>。
目的：蓄積したGoal/Plan/Decision/Work/Review/Evidenceの現在性・関係・有用性を整理し、人間とAgentが現在の仕事を根拠付きで把握できるようにする。並列記録、出典付き要約、原本を保持したpack化を接続し、通常取得する情報量と作業ディレクトリの個別ファイル数を抑える。
2026-09-30追記：Evidence並列記録とデータライフサイクル管理を追加。ファイル数、読む量、保存容量を別々に測定する。
**事実：**現行`src/cli.rs`のContextArgsではtask省略がlive working set、compileとfocusも存在する。`src/context.rs`にはlive entries、Goals、NextAction等を構成する処理がある。引数なしサマリーをゼロから新設する前に既存機能を評価する。
**Hypothesis：**探索トークンと時間を減らせる。現状の不便さの原因がデータ、検索/ranking、出力構造、Agentの利用手順のどれかはUnknown。比較評価で切り分ける。
## Scope
- 読み取り中心の棚卸しreport、整理候補とpreview、承認された可逆整理。
- 引数なしbelay contextの人間/Agent向け出力改善。
- context compileの関連性、鮮度、budget、focus、出典、矛盾の扱い。
- 利用手順と評価fixture。
- 独立ID・1件1ファイルを基本とする新規Evidence記録と、中断・DB再索引への対応。
- 完了した仕事の出典付きサマリー、原本を復元できるpack、保存場所に依存しないID解決。
- 集約対象の選定、競合検出、復元、再生成可能キャッシュの整理。
初期対象はbelay-trace内のデータとCLI。意味的な要約はSkill、原本の保存・検証・集約・参照解決はcoreが担当する。複数repository横断索引、Embedding/GraphRAG、core内のLLM呼び出し、原本/raw logの完全削除、Evidence内容の書き換え、Git履歴の書き換えは対象外。検証済みpackに保持した原本の個別ファイル整理は対象に含む。年齢だけで不要と判断しない。
## Key Design / Strategy
### 要件
- BI-R01：基準commit、対象repository、取得時点、件数、status、リンク、Evidence freshnessを含む棚卸しreportを再生成できる。
- BI-R02：coreは孤立参照、無効Task参照、stale/missing Evidence、明示的なsupersede関係を検査し、期間閾値や正規化一致による候補を抽出する。文章の矛盾・意味上の重複・完了妥当性はSkillレビューの候補とし、機械的事実と区別する。
- BI-R03：整理の前後で履歴、ID、Task→SC→Evidenceの対応と追跡可能性を保つ。削除やEvidence verdict変更で見かけの鮮度を上げない。
- BI-R04：引数なしcontextで進行中Goal/Plan、阻害要因、次の参照、検証状態、適用状態を確認できるDecisionへ到達できる。有効性の根拠がないDecisionは未確認と示す。Task statusがdoneでもGoal verifiedと誤表示しない。
- BI-R05：compileは関連性・現在性・scope・出典を使って選び、除外/欠落理由を表示する。新しいものが常に正しいとは判断しない。
- BI-R06：budgetの単位と推定方法を明記し、切断を知らせる。Constraint、Non-goal、停止条件、Task boundaryの必須情報を黙って落とさない。
- BI-R07：同一state・query・budgetで選択順序が安定し、human/AI両formatの参照IDが一致する。
- BI-R08：探索コスト・必要情報への到達・誤選択を比較し、削減率だけで品質低下を隠さない。
- BI-R09：同一checkoutの同時記録と別worktreeからのEvidence統合で、原本の欠損・上書き・ID衝突による混同を防ぐ。既存IDと月次NDJSONの読取互換を維持する。
- BI-R10：サマリーは成果・Decisionの理由/適用範囲・未解決事項・学び・成果物/Evidence参照と対象ID/版/hashを持つ派生物。原本変更時にstaleを示し、CoverageやEvidence判定の根拠を置き換えない。
- BI-R11：通常ファイルとpackのどちらでもID/版で原本を取得でき、索引なしから再構築できる。集約後もTask→SC→Evidence参照が解決する。
- BI-R12：対象確定→pack作成→検証→公開→個別ファイル整理を再開可能な操作として行う。並列追加・対象変更・中断・競合する集約でレコードを失わない。
- BI-R13：保持方針をデータ種別ごとに定める。原本の完全削除、意味的要約、可逆pack化、キャッシュ削除を別操作にし、個別ファイル数と総容量/Git履歴を別指標で報告する。
### 判定の担当と根拠
core：リンク存在、status、明示的な後続決定、scope、Evidence freshnessを既存データから検査する。期間閾値による長期activeや正規化一致は候補として理由を出力する。意味的に同じ/矛盾するとは断定しない。
Skill：候補の原文と必要な依存を読み、意味的重複・矛盾・完了妥当性について根拠付き提案を作る。coreの検出結果がないことを「矛盾なし」と解釈しない。
人間：archive/status変更、canonical参照、Decision採否を判断する。レビュー結果は出典ID・対象版・判断理由とともに記録する。
Decisionの表示は「明示根拠あり／置換済み／競合候補／未確認」。明示根拠ありは採用状態と適用scopeが確認でき、後続の取消/置換と矛盾しない場合に限る。記録不足を有効と推定しない。具体的な既存field/linkへの対応はBI-01でfixtureとして確定する。
### 情報の保持と整理方針
原本Entry/Evidenceは通常ファイルまたは検証済みpackに保持し、完全削除しない。索引は原本から再構築できる派生物。Skill生成サマリーも派生物だが再生成の文章一致は保証せず、生成版と対象snapshotを記録する。state fingerprintで原本変更を検知し、古い要約を現行と表示しない。
### データライフサイクル
- **実行中**：Goal/Plan/有効Decisionは通常の編集対象。新規Evidenceは独立IDの不変ファイルを基本とし、完成後に公開する。Work/Reviewは終了・安定後に集約候補にする。
- **知識整理**：Skillが完了した仕事の成果・判断理由・適用範囲・未解決事項を出典付きで要約する。通常contextは有効な要約を入口にし、詳細はIDから取得する。進行中Taskの境界や有効Decisionを要約によって隠さない。
- **保存集約**：まず不変Evidenceをpack化し、次に終了したWork/Reviewへ拡張する。Goal/Plan/有効Decisionのpack化は初期対象外。Work/Review再開時は対象版を通常ファイルへ復元し、更新後の版とpack内の旧版を区別する。
- **保持管理**：再生成可能キャッシュは生成方法と削除条件を持つ。原本の完全削除・外部退避・Git履歴削減は測定後に別計画とする。
### 保存と並列実行の契約
ファイル分割とID採番を一緒に変更する。新ID形式、既存ID解決、prefix検索、schema version、旧CLIとの互換境界をBI-LC00で固定する。旧CLIが新形式を扱えない場合は混在writerを禁止して明示エラーにし、静かにレコードを無視させない。
Evidenceは一時ファイルへ書き、上書きしない原子的な公開で完成品だけを見せる。原本公開後のSQLite反映失敗は「保存済み・索引未反映」と区別し、同じ記録を照合・再索引できる。成功/未確定を混同して新IDで盲目的に再送しない。
packはformat version、収録ID/版、原本hash、pack hash、取得位置を持つmanifestと原本payloadを保持する。元の内容を復元できることを検証し、安定順序で列挙する。mutable Entryの版は集約対象snapshotのdigestで識別する。immutable Evidenceでは同一ID・異内容を競合とする。
移行中の同一ID/版/hashの二重配置は一つの記録として扱い、同一ID/版で内容が違えば停止する。集約後の更新版を古いpackで上書きしない。旧月次ファイルは全収録レコードの保管確認とファイル全体の変更検出に成功した場合だけ整理し、一部だけpack化した月次ファイルは残す。
集約対象をID/版/hashで固定し、別writerの新規記録は対象外にする。複数compactorの公開/整理は保守lock等で直列化する。更新可能Entryの最終比較と個別ファイル整理は、writerと共有する排他または同等の条件付き操作で守り、確認直後の変更を消さない。
各段階のmanifest/receiptで再開・復元する。pack公開前には元ファイルを消さない。公開後でも対象が変わっていればそのファイルを残す。壊れたpackや索引欠損を検出し、原本不明のまま整理を続けない。
archived/canceled/supersededは別の意味。重複らしい文章は自動mergeせず、canonical参照と履歴の扱いを決める。Technical Decisionの有効性は更新日時だけで決めず、明示的な後続決定・適用scopeを確認する。activeの放置は完了とみなさず整理候補にする。
元Ideaの「最新」は、新着順ではなく、現在の仕事に適用でき、出典と不確実性が確認できる情報と定義する。
### コマンド案
既存belay contextを標準入口として評価する。読み取りreport用のbelay inventory等はBI-01で不足を確認してからCLI名を決める。belay summaryの追加は既存contextでは満たせないUXが測定で確認された場合だけ別Taskにする。
## Major Steps — 工程と実行Task
BI-00〜07とBI-LC00〜06は工程ID。枝番がある工程は枝番単位でTaskを作り、工程全体を重複した実装Taskにしない。Taskには基準commit、確定変更paths、入力/出力、要件ID、依存、検証、停止条件を転記する。工程依存は全必須枝番の完了を意味する。CX-01は両Plan共通の仕様Taskで、このPlanが一度だけ所有する。
### BI-00 現状とbaselineを記録
依存：なし。要件：R01、R04、R08。対象：現在のEntry/Evidence、context/compile/focus、archive、doctor、coverageの読取調査。
出力：基準commit、件数と状態分布、既存コマンドで可能/不足、固定queryと期待参照、現在の出力。データ種別ごとの個別ファイル数・原本容量・キャッシュ容量・参照関係を記録し、履歴から測定できる範囲で増加傾向を調べる。実測できない増加率はUnknownとする。
AC：直近の仕事を把握、特定Taskへ再開、過去Decisionを確認、失敗を参照、Goalの証拠を確認する最低5ケースを作り、所要時間・出力token推定・手動探索回数・到達可否を記録する。測定未実施の値を埋めない。
### BI-01 棚卸し分類と出力契約を定義
依存：BI-00。要件：R01–03。
出力：分類規則、report schema、正常/欠損/矛盾fixture、整理操作の許可範囲。
AC：各指摘にID、理由、判定主体、事実/heuristic/意味レビュー提案、対象版、参照、推奨操作を付ける。coreで判定可能な規則とSkillへ渡す候補をfixtureで区別する。Entry不足・DB drift・Evidence staleを混同せず、freshnessは既存意味を再利用する。自動完了/merge/deleteは対象外。
### BI-02 読み取り棚卸しreportを実装
依存：BI-01、BI-EVAL。要件：R01–02、R07。
対象候補：read-only inventory moduleとCLI。具体的な変更pathsはTask発行時に現在コードを確認して固定。
AC：原本を変更せず、JSON/human出力が同じ機械的指摘と候補を示す。欠損参照、期間閾値、stale証拠、正規化一致候補を固定fixtureで検出する。意味的な矛盾の自動検出を合格条件に含めない。大規模fixtureで時間/出力量を測定する。
### BI-03 整理候補previewと実データ整理
工程の要件：R02–03。製品変更とデータ変更を別Task/別diffにする。
- **BI-03a 候補レビュー手順と変更preview** — 依存：BI-02。出力：Skillによる意味レビュー案、対象ID/版、変更前後、参照影響、復元方法。AC：機械指摘と意味提案を区別し、人間が操作単位で選択できる。自動merge/deleteを含めない。
- **BI-03b 適用・復元経路のfixture検証** — 依存：03a。出力：既存mutation経路の再利用、必要なら最小追加。AC：preview後の変更を拒否し、中断/再試行で重複せず、fixtureを元へ復元できる。実データを変更する前に合格する。
- **BI-03c 選択済み実データの整理** — 依存：03b、人間による対象操作の選択。出力：適用receipt、before/after snapshot、整合性report。AC：選択した操作だけを適用し、履歴・ID・Evidence対応を保持。doctor/coverage/rebuild/exportの既知状態を悪化させない。対象なしなら理由付きno-opで完了できる。
### CX-01 共通context/budget仕様を固定
依存：BI-01。所有：棚卸しPlan担当。入力：現行context/compile/focusと統合PlanのOB-R07。Contractの内部schema確定を待たず、必須sectionを渡せる境界を定義する。
出力：sectionのrequired/optional、出典ID、表示順、token推定単位、予算超過時の結果型、truncation表示、同順位の安定順序とfixture。
AC：必須sectionを途中切断しない。収容不能なら不足量/理由を示して実行packet生成を失敗させる。通常のlive summaryは省略情報と詳細参照を表示する。Contractを持たない既存利用も動く。統合Plan担当がContract境界をこのinterfaceへ渡せることをfixtureレビューで確認する。
### BI-04 引数なしcontextのlive summaryを改善
工程の要件：R04、R06–07。
- **BI-04a 共通section renderer/budgetを実装** — 依存：CX-01、BI-EVAL。対象：既存contextの表示・予算処理。出力：required/optional section、安定順序、明示的budget不足。AC：Contractを模した必須sectionと通常contextの両fixtureを満たし、出典IDを失わない。OB-05がこの経路を再利用する。
- **BI-04b live summary内容を接続** — 依存：04a。出力：現在のGoal/Plan、blocked/Unknown、次Task参照、coverage/freshness、Decisionの適用状態、詳細取得コマンド。AC：BI-00の把握/再開caseで必要参照を取得でき、空repository/複数active/全archive/競合候補で誤った完了・有効判断をしない。next actionを実行承認済みと表示しない。
### BI-05 context compileの選択とbudgetを改善
依存：BI-01、BI-04。要件：R05–07。CX-01/BI-04aのrendererを再利用し、独自のbudget処理を追加しない。
対象：既存compile、seed、focus、ranking、切断処理。
AC：固定fixtureで関係あるDecision/失敗/Evidenceを選び、無関係な新規Entryが押し出さない。scope外とarchiveを既定で抑え、必要な歴史は明示取得できる。必須境界と出典を保ち、予算に収まらない場合は不足を明示。選択理由・truncationを確認可能にする。
### BI-06 手順とSkillの利用案内を更新
依存：BI-02、BI-04–05。要件：R03–06。
出力：まずcontextで把握→必要Taskへfocus→足りないものだけshow/search→Evidenceを確認、という短い手順と棚卸し手順。
AC：固定ケースで過剰な全件読取を誘発しない。製品の生成元を変更し、installed consumerコピーを直接編集しない。実装時に生成元を確認する。
### BI-EVAL 評価手順と採否基準を固定
依存：BI-00、BI-01。所有：棚卸しPlan担当。BI-02/04aの実装前、BI-03cのデータ変更前に完了させる。
出力：baselineデータsnapshot、query、期待参照、モデル/手順（Agent評価時）、budget、token推定方法、反復回数、時間の集計方法を固定した評価仕様。
AC：主要改善指標を探索回数・総取得token・時間から事前に選び、数値目標と許容運用負担を人間の運用責任者が確定する。目標は実装結果を見る前に保存し、変更するなら理由と版を残して旧基準でも報告する。固定budgetの出力量だけでtoken削減を測らず、必要情報に到達するまでの全取得量を数える。
必須品質条件：固定caseで必須参照の漏れゼロ、誤った完了/Decision有効判断ゼロ、sourceへの到達可能、履歴/Evidence対応喪失ゼロ。固定caseでの合格を未知の全データへの保証に拡張しない。
ライフサイクル拡張：LC00完了後、LC01〜04の実装前に、新旧形式混在・並列writer・worktree統合・中断・DB障害・pack破損・重複配置・更新版・復元の評価matrixを固定する。データ規模、writer数、反復数、記録/再索引時間の許容値、file数目標、容量/Git履歴の測定方法を記録する。数値はbaselineから確定し、既存context改善の評価仕様とは版を分けて管理する。
### BI-07 比較評価と導入判断
依存：BI-03–06、BI-EVAL。要件：R01–08。
AC：保存したraw snapshotを用い旧/新compilerを同じquery/budgetで比較する。整理済みsnapshotに対しても両compilerを実行し、データ整理とcompilerの効果を分ける。手順の効果はcompiler/dataを固定して比較する。実行不能な比較があれば因果を断定せず制約として残す。
導入判定：必須品質と事前の改善目標/運用上限を満たせば導入可。品質のみ満たす場合は限定試行とし、効率改善を達成済みとしない。品質未達なら導入不可として該当Taskを修正する。結果は誤参照/漏れ、総取得token、探索回数、時間、維持負担を含める。
### BI-LC00 保存・版・保持方針を固定する
依存：BI-00、BI-01。要件：R09–13。所有：棚卸しPlan担当。
出力：新旧Evidence ID仕様、record/Entry revisionの意味、pack/manifest仕様、参照解決、SQLite再構築、再開/保持規則、CLI互換表。
AC：通常ファイル、旧月次NDJSON、packの混在・重複・異内容・更新版の扱いをfixture化。Evidenceを最初のpack対象にし、Work/Reviewの終了判定と再開手順を別に定義する。SQLiteとファイルを一つの原子transactionと仮定しない。publish/lock/fsync等の保証範囲とクラッシュ時の復旧規則を記録する。
### BI-LC01 並列Evidence記録
要件：R09、R11。
- **BI-LC01a 分散IDと互換reader** — 依存：LC00、BI-EVAL。出力：別worktreeでも独立発行できるID、旧ID/新IDの検証・参照解決。AC：固定時刻の複数processと独立worktreeで記録を統合し、ID混同を起こさない。prefix曖昧性と旧CLI非対応を明示する。
- **BI-LC01b 原本公開と索引復旧** — 依存：01a。出力：1件1ファイルの完成後公開、保存状態と索引状態の報告、再試行/再索引。AC：同時writer/reader、公開前後・DB更新前後の中断で壊れた完成ファイルを露出せず、記録を欠損・二重計上しない。索引未反映を成功済み索引と表示しない。
### BI-LC02 出典付きサマリー
要件：R10。
- **BI-LC02a 要約artifactと更新検知** — 依存：LC00、BI-EVAL。出力：対象ID/版/hash、生成版、成果/判断/未解決/出典の形式、stale判定。AC：対象変更・参照欠損を検知し、原本へ遡れる。学びと検証済み事実を区別する。
- **BI-LC02b 要約Skillと品質fixture** — 依存：02a。出力：完了仕事の要約手順と評価例。AC：重要な制約、Decision理由、未解決事項を落とさず、根拠なしの完了/一般化を追加しない。Evidence判定は原本経由で取得する。要約の文章一致を決定論的合格条件にしない。
### BI-LC03 pack・参照解決・再構築
要件：R11。
- **BI-LC03a pack writer/readerと検証** — 依存：LC00、LC01、BI-EVAL。出力：原本payload、manifest、hash検証、ID/版取得。AC：全原本が復元可能でhash一致し、破損・欠損・形式不明を拒否する。旧月次NDJSONとの混在を読む。
- **BI-LC03b 全参照経路の接続** — 依存：03a。対象：show、Evidence status、Coverage、context、sync/rebuild、export、doctor。AC：同一記録の二重配置を二重計上せず、異内容は競合として報告する。SQLiteを破棄したfixtureから全参照と検証結果を再構築できる。
### BI-LC04 安全な集約・復元
要件：R12–13。
- **BI-LC04a Evidence集約のpreview/apply/recover** — 依存：LC03。出力：対象snapshot、操作receipt、検証済みpack公開、条件付き個別ファイル整理。AC：新規追加、月次追記、複数compactor、各段階の中断・再試行で欠損なし。Git統合後のpack/通常ファイル重複も照合できる。対象外ファイルを削除しない。
- **BI-LC04b 終了Work/Reviewの集約と再開** — 依存：04a。出力：終了・安定・依存確認に基づく対象選定、復元→更新の手順。AC：進行中または変更されたEntryは整理しない。再開後の更新版が旧packに置き換わらず、Task/SC/Review参照が解決する。
- **BI-LC04c 再生成可能キャッシュの整理** — 依存：LC00、BI-02。出力：削除候補・再生成手順。AC：原本/pack/manifest/receiptをキャッシュ扱いせず、削除後に必要状態を再生成できる。
### BI-LC05 context・利用手順へ接続
依存：BI-05、BI-06、LC02、LC03。要件：R04–07、R10–11。
出力：有効サマリーを入口とする取得、原本詳細への展開、stale時の原本fallback、集約/復元の利用手順。
AC：CX-01の必須境界を維持し、要約がContract/Task制約を押し出さない。要約なし・古い要約・pack破損時に誤ったverifiedを示さない。通常取得量と必要情報への到達を固定caseで比較する。
### BI-LC06 ライフサイクル全体の評価・導入判定
依存：LC01〜05、BI-EVAL。要件：R09–13。
AC：並列記録→要約→pack→個別ファイル整理→索引再構築→原本取得→Work再開を固定fixtureと隔離した実データcopyで検証する。元データへの適用前に復元を確認する。
測定：個別ファイル数、packを含む総容量、Git履歴容量、走査/再索引時間、総取得token、要約品質、記録成功/競合/復旧件数。file数減少を容量減少や性能改善と同一視しない。
必須条件：対象原本の復元/hash一致100%、固定試験の欠損/誤上書き/参照喪失/二重計上ゼロ、要約経由の誤った検証判定ゼロ。性能・運用負担はBI-EVALで事前固定した目標を用いる。品質失敗は導入不可、品質のみ合格なら限定試行。初期導入は原本完全削除を伴わない。
### リリース目標：0.7.0
2026-09-30 checkout確認時の現行版は0.6.3（Cargo.toml/Cargo.lock自package）。このPlanのContext改善、並列Evidence、要約、pack/復元をまとめる目標版は**0.7.0**とする。機能と保存互換性の境界が広がるため0.6.xの修正版から一段進める。Omnia–Belay Integration全体の完成や1.0の安定契約はこの版の条件に含めない。
BI-07とBI-LC06の導入可判定後に正式版番号を更新する。先行試行が必要ならpre-releaseとして区別し、未完了を0.7.0完成と表示しない。実装時に別リリースとの番号競合を再確認する。アプリ版と保存schema versionは別管理。
### BI-DEV01 make deployを追加
登録Task：<mention-page url="https://app.notion.com/p/3ebc2ad7b43181f28f89d2d3d2ee78d6"/>
依存：なし。先行可能な小規模Task。Makefileのbuild/deploy targetとREADMEを追加し、`make deploy`でrelease build成功後に`~/.local/bin/belay`へインストールする。BINDIRを変更可能にし、失敗時は既存binaryを保持する。完了条件：隔離先への配置・version一致・再実行・build失敗時の非破壊確認。
### BI-REL01 0.7.0へのバージョン更新
登録Task：<mention-page url="https://app.notion.com/p/3ebc2ad7b431815b8d1cfb95ed1d6294"/>
依存：BI-DEV01、BI-07、BI-LC06。Cargo.tomlとlockfile自package版を0.7.0へ整合させ、release notesと旧データ/新形式の互換・復元手順を更新する。完了条件：必須チェック、移行/復元試験、隔離先でmake deployしたbinaryの0.7.0表示。tag pushや公開はこのTaskの範囲外。
### 順序と着手
BI-00 → 01 → BI-EVAL → 02 → 03a → 03b → 03c。
CX-01はBI-01後。CX-01/BI-EVAL → 04a → 04b → 05。02/04/05後に06、03/06後に07。
ライフサイクル系はBI-00/01 → LC00 → BI-EVAL拡張 → LC01とLC02。LC01 → LC03 → LC04、BI-05/06とLC02/03 → LC05、LC01〜05 → LC06。LC04cはLC00/BI-02後に独立して進められる。
BI-07は既存context改善の導入判定、LC06は保存/要約機能を含む追加の導入判定。CX-01/BI-04aやOmnia統合をpack完成待ちにしない。
BI-DEV01は独立して先行し、BI-DEV01/BI-07/LC06の完了後にBI-REL01で0.7.0へ更新する。
最初の依頼はBI-00「現状棚卸し＋5ケースbaseline＋種別別ファイル数/容量」。評価基準と分類規則を固定してから改善を実装する。
## Risks / Open Questions
- **Unknown：**不便さの主要因と削減量。BI-00/07、LC06で確認する。
- **設計で確定する事項：**新Evidence ID形式、pack形式/サイズ、読取索引、旧CLIの拒否方法、共有lockと耐久性、終了Work/Reviewの版・再開規則。LC00の担当がfixture付きで確定し、未決部分の実装を開始しない。
- **保存上の限界：**pack化は主に個別ファイル数を減らす。Gitに既に入った原本の履歴は残り、pack追加で総容量が増える場合もある。最初から容量削減を保証しない。
- **外部参照：**Evidenceが外部log/成果物を参照する場合、recordのpack化だけでは外部内容を保存したことにならない。参照切れを検出し、添付原本の保存対象をLC00で明示する。
- **未決：**長期activeの閾値、現在のDecisionを表す既存linkの使い方、inventoryのCLI surface。BI-01でfixtureに基づいて決定する。
- 古いが有効なDecisionや失敗履歴を消すと検索は短くても判断が悪くなる。履歴保持・出典・欠落検証を優先する。
- 機械的CoverageはIntent達成や人間の受け入れと同義ではない。Evidenceがmissing/staleなら不確実性を残す。
## Omnia–Belay Integrationとの境界
<mention-page url="https://app.notion.com/p/3b8c2ad7b43180d7a0a8d78e58d2392b"/>と独立に棚卸し調査・分類・共通rendererを進められる。
共通仕様の正本はこのページのCX-01。棚卸しPlan担当が仕様とBI-04aの実装を所有し、統合Plan担当がContract要件への適合を確認する。順序はCX-01 → BI-04a → OB-05。BI-05はranking/候補選択、OB-05はContract/provenance/AC mappingを所有する。
共通renderer変更が必要ならCX-01の版とfixtureを先に更新し、両側の合格条件を確認する。同じ共通処理を両Planで実装しない。BI-03cの実データ整理はContract開発の前提にしない。
このPlanの人間向けサマリーとOBのOutcome Capsuleは別用途。前者は現在作業の把握、後者は特定Contractの結果還元であり、混ぜて新しい正本を作らない。
## 出典・確認範囲
- <mention-page url="https://app.notion.com/p/3c2c2ad7b43180409086cb0867586272"/>：データ整理、context compile見直し、引数なしサマリーの要求。
- <mention-page url="https://app.notion.com/p/3b6c2ad7b43180e3ac61dda12843b98b"/>：local-first、Trace/Evidence、Outcome Consolidationの方向。
現行コードはcontext入口とlive出力の存在を限定確認した。実データの健全性・性能・全CLI挙動はこのPlan作成では未評価であり、BI-00で検証する。
</content>
</page>
