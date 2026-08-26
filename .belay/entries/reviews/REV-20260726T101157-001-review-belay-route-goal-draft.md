---
schema_version: 1
id: REV-20260726T101157-001-review-belay-route-goal-draft
type: review
title: Review Belay Route goal draft
status: completed
created_at: 2026-07-26T10:11:57+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
- relation: references
  id: PLN-20260712T164000-001-deliver-phase-6-assurance-incrementally
- relation: reviews
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Review method

- 対象: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo (revision 3, draft)
- 入力: `belay goal improve <id> --budget 3000` の出力 (Goal本文、Deterministic Lint、Related Context bundle)
- 併読: GOAL-20260726T011059-001 (Belay agent guidance)、PLN-20260712T164000-001 および `docs/design/phase6.md` (Phase 6 Delivery Assurance)、GOAL-20260712T163956-001
- 観点: 人間が事前指定した7点 (Vision/MVP混同、SCの実装先取り、人間の責任という価値観の固定、SC-005の形式化、SC-006の実行可能性、Phase 6および一般的Agent Skillとの弁別、モデル能力で代替できない価値の定義)
- Deterministic Lint: checklist 10/10 passed、findings なし。以下はすべて意味論的指摘。

## Findings

- High: Vision と初期Goalが同一entryに同居している。Non-goals は「Routeの**初期Goal**で独立サービス、課金、法人向け運用を確定しない」と書き手の意図を示すが、SC-001..SC-007 は Route の全機能セットを網羅する。決定的なのは SC 間に時間的前後関係が埋まっていること。SC-006 (baseline比較) と SC-007 (モデル/ベンダー非依存) は SC-001..004 が実装され複数環境で運用された後にしか評価できない。ひとつの Goal に「作る」「効いていることを示す」「一般化する」の3フェーズが同居している。
- High: SC-006 の「いずれかを改善し」という OR 条件により、6指標のどれか1つが改善すれば通過する。多重比較により偶然でほぼ必ず通るため、テストとして機能しない。Verification 側に「primary metrics を Plan で固定」とあるが、SC 本文が OR のままなら緩い方が効く。
- High: SC-005 は「説明**または**具体的に訂正できる」を要件とするが、「説明できる」は Route が直前に提示した内容の復唱で満たせる。再認 (recognition) を再生 (recall) と取り違えている。加えて Route が提示物を作り、その提示物への応答を Route が Evidence 化するため、出題者・教材・採点者が同一で独立性がない。
- High: SC-005 に閾値がない。「人間は説明または訂正できる」は能力の記述であり、全員が毎回できれば pass なのか1回でも失敗すれば fail なのか判定不能。
- Medium: 「Routeは人間の内面的理解を証明したと主張しない」という留保は Goal 本文にしか効かず、Evidence の下流利用へ伝播しない。Evidence として残る以上、後続セッションからは「人間は確認済み」として参照される。Risks の「human gateがチェックボックス化」の具体的発生経路だが、Risks には抽象的にしか記載がない。
- Medium: SC が実装方法を先取りしている。(a) SC-002 の「2から3件以下」は仕様値。Unknowns で応答粒度を未解決と自認しながら候補数だけ確定しており非対称。(b) SC-002 の12項目列挙はテンプレート仕様であり達成条件ではない。項目数の多さ自体が Risk「認知負荷を増やす」と衝突しており、SC が自らの Risk を生産している。(c) SC-003 の materialize は機構の先取り。
- Medium: 「人間が目的、権限、リスク受容、採否に責任を持つ」が Summary に公理として置かれ、Assumptions に入っていないため反証不可能。一方 Why This Goal 末尾の「モデル能力が向上しても統制の必要性は残り得る」は明白な仮説だが正当化セクションに置かれている。仮説が根拠の位置に座っている。
- Medium: Constraints の「実装開始、Issue作成、PR作成、mergeは既存の個別Human Gateを維持する」がゲート粒度まで固定する一方、Unknowns は「モデル能力の向上に応じて人間承認、自動判断、監督、停止条件の境界をどう変えるか」を未解決とする。Constraint が Unknown の探索空間を先に閉じている。
- Medium: 「責任」が accountability (結果の帰属先、ほぼ固定でよい) と認知的関与 (程度問題であり Route が動かそうとしている変数そのもの) の2義で未分離。分離しないと SC-005 が「帰属を守るために関与を要求する」のか「関与自体が目的」なのか読めない。
- Medium: SC-006 のブラインド化が不可能。baseline は Route protocol を知らない状態だが、評価者は Route の考え方を内面化した構想者本人であり、同一人物の before/after は学習効果で汚染される。「評価者間一致条件は Plan で定める」は評価者が実質1人なら成立しない。評価者数の下限は Goal レベルの制約に属する。
- Medium: SC-006 の時間指標は符号が定義依存。「現在地把握時間」「次の判断までの時間」は提示物の生成時間を計測に含めるかで反転する。「重大なConstraint/Requirement漏れ」は事後発覚しないと数えられず観測期間が未定義で、実開発では母数が数件で統計的に何も言えない。「形式的承認を許容範囲内に保つ」は測定手段未定義のものを guardrail に置いている。
- Medium: Route と Phase 6 の弁別条件がない。時間軸 (during / between) の分離は言えているが、`docs/design/phase6.md` 設計原則1 の Intent Brief も Fact/Assumption/Unknown/Human decision の分離と人間による解釈案訂正を行う。SC-001 と SC-005 は Phase 6 の Intent Brief とほぼ同じ操作を対象だけ変えて記述しており、現状の SC では Phase 6 の T-003 dogfooding で代替可能。
- Medium: 一般的な Agent Skill との弁別が、初期形態を Skill にすると消える。Unknowns は「Route protocol を Skill から検証するか」と「良いSkillや強いモデルで再現できない部分は何か」を同時に持つ。Skill で実装すると比較対象と実装形態が同一になり、差は内容の差に還元される。その場合に残る弁別軸は入力の決定性 (Belayの型付きread interface) と出力の扱い (承認境界とprovenanceの強制) のみ。
- Medium: 非代替価値の重心がずれている。モデル能力に耐性があるのは SC-003 (承認境界)、SC-004 (再統合の照合)、SC-007 (セッション/ベンダー非依存)。SC-001 (再構成の質)、SC-002 (候補の質)、SC-005 (問いの質) はモデル能力そのもので、世代交代で追い越される。現在の Goal は後者を前面に置いており Risk「コモディティ化」に直撃する。
- Medium: SC-004 の「照合する」が意味照合 (モデル依存) と決定的照合 (coverage/linkのような) のどちらとも読め、モデル能力への耐性が反転する。決定的側に寄せられれば既存の Goal Coverage に接続でき、最も堅い差別化点になり得る。
- Medium: Goal に撤退条件がない。`docs/design/phase6.md` は完了条件・見直し条件・撤退条件を持つ。Risks は「独立した価値が残らない」を認識しているが、判定基準は Verification 内で「Plan へ固定する」と後送りされ、Goal レベルの応答がない。
- Low: Assumptions の「異議 (dissent)」が SC にも Constraints にも現れない。Risks に「minority viewが失われる」とあるのみで、Assumption が SC に接続していない。
- Low: SC-004 の「Slice」が無定義。Non-goals は「すべての作業をVertical Sliceへ変換すること」を否定しており、未定義の実装語彙が SC 本文に入り込んでいる。
- Low: SC-001 が列挙する Product Intent と Constraint は現行 Belay の first-class entry ではない。Non-goals が「Product Map、Requirementを直ちにfirst-class objectとして追加しない」と述べる以上、SC-001 は既存 entry からの導出で満たす前提のはずだが明記がなく、SC-001 が Non-goals を破る読みが成立する。
- Low: SC-006 と Verification が重複しており、どちらが権威か不明。
- Low: Human observations の「AIに読解を委任した人が重要事項を把握しないまま説明を自分の理解として扱う」が問題の中核証拠だが、n 不明の伝聞。Unknown と正直に記載されている点は適切だが、Goal 全体で最も弱い接合点であることが本文から読み取れない。
- Low: Non-goals 11項目、Risks 10項目、Constraints 8項目、Unknowns 10項目に優先順位がなく、「これが崩れたらGoal自体が無効」という中核が識別できない。Goal 自身が解こうとしている問題 (重要事項が量に埋もれる) を Goal 自身が起こしている。

## Improvement proposals

### P-001 Goal を分割し Vision を Goal 外へ出す (High findings 1 に対応)

- Goal A (このentryを縮約): Route protocol が反証可能な形で成立するかの検証。SC-001/002/003 と SC-005 の一部のみを残す。撤退条件を Goal 本文に持たせる。
- Goal B (後続entry): 有効性と一般化の主張。SC-006/SC-007 を移す。
- Summary 第3段落の「追跡可能な進化的開発」の定義は `docs/design/route.md` または roadmap へ移す。Vision が Goal 本文にあると Goal の成否判定に混入する。

### P-002 撤退条件を Goal 本文へ追加 (Medium: 撤退条件欠落 に対応)

`docs/design/phase6.md` の完了条件/見直し条件/撤退条件の構造を採用する。撤退条件の候補:

- 良い AGENTS.md と Skill と強いモデルの組み合わせが、固定シナリオで Route と同等の検出結果を出す場合、Route を独立機能化しない。
- 人間の訂正が連続してゼロになり、承認が提示内容の追認のみになる場合、SC-005 の設計を破棄する。

### P-003 SC-002 を観測可能な帰結へ書き換える (Medium: 実装先取り に対応)

- 候補数「2から3件以下」と12項目列挙を SC から Plan の設計変数へ降格する。
- 代替案: 候補集合から選択した後に「重要な代替案が欠けていた」と事後判明した割合が baseline 以下であること。提示情報量の上限は Plan が定める。
- これにより SC-002 と Risk「認知負荷を増やす」の衝突が解消する。

### P-004 責任概念を2層に分離する (Medium: 価値観の固定 に対応)

- Summary の「人間が目的、権限、リスク受容、採否に責任を持つ」を accountability (固定してよい前提) として明示し、Constraints へ移す。
- 「人間が実際に理解して判断しているか」を認知的関与として分離し、Assumptions へ明示的な仮説として置く。
- Why This Goal 末尾の「モデル能力が向上しても統制の必要性は残り得る」を Assumptions へ移す。正当化セクションに仮説を置かない。
- 効果: accountability に立脚することで、Non-goals で棚上げした「AIの判断が人間より正しいか」に依存せず主張が成立する。

### P-005 Constraint と Unknown の衝突を解く (Medium: ゲート粒度の固定 に対応)

Constraints の Human Gate 維持条項に「本Goalの検証期間中は」と時限を付す。または Unknowns の「モデル能力の向上に応じた境界変更」を本Goalの範囲外と Non-goals へ移し、どちらか一方に寄せる。現状は同一対象について Constraint と Unknown が反対のことを述べている。

### P-006 SC-005 を復唱で満たせない形に再設計する (High findings 3,4 に対応)

- 提示に含まれていない帰結を問う形式へ変更する。例: 「この候補を採ると、次に何が未検証のまま残るか」。復唱では答えられない。
- 訂正ゼロの連続を形骸化のシグナルとして事前定義し、負の指標として Plan へ固定する。
- 閾値を SC に書く。何件中何件で pass とするか、どの粒度の誤りを fail とするかを定義しないと判定不能。
- 「内面的理解を証明しない」という留保を Evidence 側の記録形式へ埋め込む。Goal 本文の但し書きは下流へ伝播しない。
- Unknowns へ追加: 意図的に誤った Assumption を混ぜる検出テストは有効だが「Belay を Source of Truth とする」Constraint と衝突する。評価シナリオ限定で虚偽を注入できるかは未解決。

### P-007 SC-006 を既知解のある固定シナリオ検出テストへ置き換える (High findings 2、Medium: ブラインド化/指標定義 に対応)

- 実開発での効果測定を Goal A から外し、Goal B へ送る。
- Goal A に残す形: 同一の Belay リポジトリ状態に既知の Constraint 衝突、未検証項目、矛盾する Decision を事前に埋め込み、Route あり/なしで検出できるかを測る。正解が既知なので採点でき、複数モデルと fresh session で反復でき、母数を作れる。
- Verification の2番目 (今回の構想議論を基準事例として保存) は既にこの方向を向いており、SC をそちらへ寄せると整合する。
- OR 条件を廃し primary metric を1つに固定する。
- 評価者数の下限を Goal レベルの制約として記述する。

### P-008 Phase 6 との弁別テストを SC 化する (Medium: Phase 6 弁別 に対応)

- 弁別条件: 人間の依頼がまだ存在しない状態で有用な出力が出るか。Phase 6 は人間の発言を起点とするため依頼が無ければ動かないが、Route は動くはず。
- これを SC へ書けば差が観測可能になる。現状の SC-001/SC-005 は Phase 6 の Intent Brief とほぼ同一操作であり、Phase 6 の T-003 dogfooding で代替されうる。

### P-009 非代替価値の重心を SC-003/004/007 へ移す (Medium: 価値定義 に対応)

- Goal の主張を「良い候補を出すこと」から「何が承認され、何が承認されずに残り、それが誰の権限で、どの記録に基づいたかが、モデルとセッションをまたいで再構築できること」へ移す。
- SC-004 の「照合する」を決定的照合側へ限定する。既存の Goal Coverage に接続でき、モデル能力への耐性が最も高い差別化点になる。
- SC-001/002/005 は達成条件ではなく、Route の入力条件または前提として位置づけ直す。

### P-010 落ちている概念と用語を補う (Low findings に対応)

- Assumptions の「異議 (dissent)」を SC または Constraints へ接続する。現状 Assumption が SC に届いていない。
- SC-004 の「Slice」を定義するか削除する。Non-goals が Vertical Slice への一律変換を否定している以上、無定義の使用は矛盾に読める。
- SC-001 に「既存 entry からの導出で満たし、新規 first-class object を追加しない」と明記し、Non-goals との整合を明示する。
- SC-003 の「materialize」を「人間が採用した内容だけが出典を保って記録される」に置換し、機構の先取りを外す。
- SC-006 と Verification の重複を解消し、どちらが権威かを決める。

### P-011 Goal 自体の量を削る (Low: 量と優先順位 に対応)

- Non-goals 11、Risks 10、Constraints 8、Unknowns 10 に優先順位を付ける。または「これが崩れたら Goal 自体が無効」となる中核を各セクション先頭に分離する。
- Human observations の伝聞根拠が Goal 全体の最も弱い接合点であることを本文へ明記する。

## Positive findings

- Deterministic lint は 10/10 pass、findings なし。構造要件は満たしている。
- Problem Hypothesis と Assumptions に "This is a hypothesis." が明示され、確認済み事実と仮説の分離が entry 内で実践されている。
- Human observations が「発生率、一般性、主要因はUnknown」と自認しており、証拠強度を過大に主張していない。
- Confirmed context が「この対話はRoute候補ワークフローの一事例だが、一般的な有効性の証明ではない」と自己の n=1 性を明示している。
- Risks が automation bias、形式的承認、二重管理、コモディティ化を既に列挙しており、本レビューの High/Medium findings の多くは新規発見ではなく「Risk を認識しながら SC がそれに応答していない」という接続の欠落を指す。
- Unknowns に「良いAGENTS.md、Skill、強いモデルだけでは再現できない部分は何か」という最も重要な問いが既に立っている。
- Non-goals が「人間が実際に理解したことを機械的に証明すること」「組織における責任放棄や形式的承認を完全に防止すること」を明示的に除外しており、過大な主張を先に封じている。

## Follow-up

- Owner: 人間 (構想者)。P-001 の Goal 分割を採否判断する。分割しない場合、SC-006/007 の評価時期をどう扱うかを決める。
- Owner: 人間 (構想者)。P-004 の責任概念の2層分離は Goal の価値主張の土台を変えるため、AI が代理決定しない。
- Owner: 人間 (構想者)。P-005 の Constraint/Unknown 衝突は、どちらへ寄せるかが権限設計の方針判断であり人間が決める。
- Owner: AI + 人間。P-006 と P-007 の具体化は後続 Plan で行う。primary metric、閾値、評価者数の下限、失敗条件、撤退条件を実装前に固定する。
- Owner: AI。P-010 の用語整理と Non-goals 整合は、人間が P-001 を判断した後に Goal 改訂案として提示する。
- 本レビューは Goal draft に対する意味論的レビューであり、実装、Plan 作成、Goal 本文の改訂は含まない。Goal の改訂は人間の採否判断後に別途行う。

requires_human_review: true

## Outcome

Goal は draft として構造的に健全 (lint 10/10) だが、現状のままでは検証可能な Goal として成立しない。ブロッキングは3点。(1) Vision と初期Goalの同居により SC 間に評価不能な時間的前後関係が埋まっている。(2) SC-006 の OR 条件によりテストとして機能しない。(3) SC-005 が復唱で満たせるうえ閾値がない。

P-001、P-006、P-007 を適用するまで Plan 着手は推奨しない。P-004 と P-005 は Goal の価値主張と権限設計の土台に関わるため人間の判断を要する。P-002、P-003、P-008 から P-011 は改訂時にまとめて適用可能。
