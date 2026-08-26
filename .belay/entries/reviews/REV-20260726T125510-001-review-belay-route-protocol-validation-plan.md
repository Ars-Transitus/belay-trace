---
schema_version: 1
id: REV-20260726T125510-001-review-belay-route-protocol-validation-plan
type: review
title: Review Belay Route protocol validation plan
status: completed
created_at: 2026-07-26T12:55:10+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
- relation: references
  id: GOAL-20260726T103644-001-make-belay-route-decision-state-legible-and-trus
- relation: reviews
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: follows-up
  id: REV-20260726T101157-001-review-belay-route-goal-draft
metadata: {}
---

## Review method

- 対象: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product (revision 4, draft)
- 併読: GOAL-20260726T014020-001 (revision 4、改訂後)、GOAL-20260726T103644-001 (Browse)、REV-20260726T101157-001 (先行レビュー)、`docs/design/phase6.md`
- 確認: `belay goal lint` (改訂Goal 10/10 passed)、`belay export --help`、`belay coverage --help` の出力契約、既存entryのMarkdown構造
- 観点: 改訂GoalのSC充足性、評価設計の反証可能性、先行レビューのfindings継承状況、撤退条件の実行可能性
- 本レビューは Plan draft に対する意味論的レビューであり、実装、fixture作成、Goal/Plan本文の改訂を含まない。

## Findings

### Blocking

- High: Planのbaselineが、Goalの撤退条件より弱い。Goalは「良いAGENTS.md、一般的なSkill、強いモデルとBelayの組み合わせが同等の検出率、provenance、承認境界、再構築性を示す場合、Routeを独立機能化しない」と定める。しかしPlanのbaselineは「通常のAIとBelayだけ」+「一般的な高品質分析依頼」であり、AGENTS.mdもSkillも含まない。この評価ではGoalの撤退条件を発火させられない。Goalが持つ最も重要な判断ゲートが、Planでは検証不能になっている。
- High: fixtureが構造的に循環している。seedするblocking条件5種 (Constraint conflict、contradictory Decision、implemented but unverified、unsupported Assumption、unapproved/deferred scope) は、Route schemaが型として持つslotと1対1で対応する。「slot Xを持つschemaがXを検出できるか」を測っており、protocolの一般的な検出能力を測っていない。fixture作成 (T-004) はprotocol設計と同じ AI + Human であり、hidden holdout 1件では緩和にならない。
- High: Route Inputの決定性境界が未定義。Belayの型付けはentry type、link、status、Evidenceの水準にあり、Constraint、Assumption、Unknown、Conflictはentry本文のMarkdown見出しと箇条書きであって型付きfieldではない (`belay goal lint` はsection存在のchecklistであり項目の型を検査しない)。したがってRoute Input構成は次の二択になり、Planはどちらも選んでいない。(a) LLMがprose解釈する場合、決定性がRoute Inputの時点で崩れ、かつ抽出段階が検出作業を先取りするためbaseline比較が汚染される。(b) 決定的に構成する場合、Route Inputはentry水準の構造とraw proseしか運べず、検出はすべてLLM段階に残り、protocolの寄与はprompt + schemaに縮む。これはGoalの撤退条件が想定する「良いSkill」との差が消える領域である。T-002とT-003の両方がこの未決定に依存している。
- High: reviewerの独立性が自己矛盾している。Review Requirementsは「protocol authorとは別のfresh-context reviewerが...レビューする」と定めるが、Unknownsは「2名のreviewerのblind scoringを、構想者とfresh-context Claudeで十分とするか」と構想者をreviewer候補に置く。Risks and Responsesの「protocol authorが採点しないrunを設ける」は、逆にauthorが他のrunを採点する前提を露呈している。加えて、Route conditionを実行するのがClaudeであるとき、reviewerの一方が fresh-context Claude であることは独立性を与えない (同一model familyの誤りは相関する)。SC-007の「最低2名の独立reviewer」は現状の候補では満たせない。
- High: SC-006の2閾値が相互作用して、条件次第で原理的に達成不能になる。SC-006は「90%以上を表面化」と「baselineを20 percentage points以上上回る」をANDで要求する。baselineが70%を超えると、Route側は90%上限に頭を抑えられて+20ppを満たせず、Route protocolの品質に関係なくSC-006がfailする。PlanのUnknownsは閾値の妥当性を問うているが、この上限との干渉には触れていない。

### Non-blocking

- Medium: 統計的検出力が不足し、cluster構造が無視されている。「最低12回」は総runか条件あたりかが不明で、総12回なら1条件6runとなる。同一fixtureを再利用するためrun間の観測は独立ではなく、fixture caseがclusterになる。この規模で20ppの差はノイズと区別できない。Planは検出力、解析単位、区間推定の方針を事前登録していない。
- Medium: SC-004 (Reconciliation Result) と Materialization Preview に実行経路がない。T-002が定義し、T-003がvalidatorで検査するが、実際のBelay stateに対してReconciliation Resultを生成するtaskがDelivery Mapに存在しない。T-006が採取するのは「Human Responseを含むraw結果」までであり、T-008の reconcile はGoal水準のmeta作業であってprotocolのReconciliation Result documentの行使ではない。SC-004はpassing Evidenceを得る経路を持たない。
- Medium: PlanがSC-005を暗黙に弱めている。SC-005は「提示内容の復唱では得られないblocking帰結を特定」を要求するが、Scope §5は「seedしたblocking帰結が人間の選択前に表面化し、具体的応答へ反映されたか」を採点対象としており、これは復唱で満たせる。さらに構造的緊張がある。Routeが優秀であるほど (SC-002/SC-006が高得点であるほど)、Route outputに含まれない帰結は減り、SC-005が満たしにくくなる。SC-005とSC-002/SC-006は反対方向へ引いており、Planはこの緊張を認識していない。
- Medium: 「未承認内容の正式記録は0件」がcapabilityの不在によって自明に満たされる。本Planは production `belay route` command を追加せず、自動writeを除外し、materializationは手動である。書き込み経路が存在しないため0件は設計の成果ではなく構成上の帰結であり、SC-006のこの部分は空虚になる。
- Medium: Route Input実現可能性 (Assumption 4「既存の`belay context compile`またはexport出力から、内部storageへ依存せずRoute Inputを構成できる」) が最大の依存でありながら、T-003で初めて判明する。偽だった場合、Belay側の追加contractがDecision候補として人間へ回され、T-001とT-002への投資が無駄になる。前倒しのspikeが必要。
- Medium: Gate 3 (fixture、prompt、metric、閾値、run matrixのfreeze承認) を承認する人間が、T-004とT-005のactorに含まれる。自身が作成した成果物を自身がfreezeする構成は、overfittingに対するgateとして機能しない。
- Medium: baseline promptがseeded条件を知る側によって書かれる。悪意がなくても、baseline promptが「矛盾を探せ」と促さない形になれば差が誇張される。独立した第三者がfixtureを見る前にbaseline promptを承認する手順がPlanにない。
- Medium: SC-007の一致度に指標と閾値がない。「一致して再構築できる」の合格線が未定義であり、加えて「相違を調停する」は事後の合意形成であるため、調停前のagreementを別途記録しないと独立性の測定値が失われる。Planは相違の保存には言及するが、調停前後の区別と閾値は定めていない。
- Medium: Phase 6との弁別テストが採用されていない (先行レビューP-008)。T-001はdurable designで境界を文書化するが、両者を区別する唯一の運用差である「人間の依頼がまだ存在しない状態で有用な出力が出るか」に対応するfixture caseがない。Phase 6のIntent Briefも Fact/Assumption/Unknown/Human decision の分離と人間による訂正を行うため、この差を測らないとPhase 6のdogfoodingで代替可能という疑いが残る。
- Medium: schema bloat と「Goal自身が解こうとする認知負荷の再生産」に測定手段がない。Risks and Responsesは「required fieldを最小化」という設計方針を置くが、これは応答であって観測ではない。Route outputの分量とrequired field数を記録しないと、両Riskは評価後も判定できない。

### Low

- Low: 「12回以上のrun matrix」が総run数か条件あたりかで解釈が割れる。事前登録前に確定が必要。
- Low: 「blocking条件検出率」が条件単位か run単位かが未定義。分母が変わると90%の意味が変わる。
- Low: Delivery Map上、T-003 (validator) がT-004 (fixture) より前に置かれ、validatorとfixtureが相互適応しうる順序になっている。
- Low: Unknownsの「standalone packageの実装言語と配置」が未決定のままT-002とT-003が依存しており、対応するgateがない。
- Low: Delivery Mapに依存関係列がなく、spike、freeze、blocking関係が本文からしか読み取れない。

## Improvement proposals

### P-101 baselineを撤退条件と同じ強さにする (Blocking 1)

- baselineを3条件へ拡張する。(a) Belayのみ + 一般的な高品質分析依頼、(b) Belay + 良いAGENTS.md + 一般的なSkill + 強いモデル、(c) Route protocol。
- 撤退判断は (b) と (c) の比較で行う。(a) は下限確認として保持する。
- 3条件が run 数の制約で困難なら、(a) を落として (b) をbaselineとする。Goalの撤退条件が発火可能であることを優先する。
- Goal側で撤退条件を弱めて整合させる選択もあるが、前回レビューでP-002として追加した条項であり、弱めるなら意図的な判断として記録すべき。

### P-102 fixtureの循環を外す (Blocking 2)

- fixture caseの半数以上を、本repositoryの実履歴から構成する。候補: explore spacingのrevert→再適用サイクル、0.3.2 release candidateのdraft PR承認とmerge/tag/publish未承認の境界、Phase 6 T-003が未着手のままT-002がimplementedである状態、Playwrightがローカルで実行できずCI待ちだった未検証状態。正解ラベルは既存のReviewとWorkから復元できる。
- schema slotに対応しないblocking条件を最低1種含める。候補: 参照先entryが存在するが status が逆行している、Evidenceのsourceが古いcommitを指す、GoalのNon-goalsとPlanのScopeが衝突する。
- hidden holdout を protocol author 以外が作成し、authorへ内容を開示しない。1件では不足であり、最低2件かつ全体の15%以上を推奨。

### P-103 T-000としてRoute Input spikeを追加する (Blocking 3、Medium: 実現可能性)

- `belay export json`、`belay coverage --format json`、link graph、statusから、決定的にどこまでRoute Inputを構成できるかを実測する。
- prose解釈なしに得られないものを列挙する (Constraint、Assumption、Unknown、Conflictのentry内型付けは現状存在しない)。
- 結果をDecisionとして記録し、二択を明示的に選ぶ。(a) 抽出段階もLLMに任せる場合、baselineへ同じ抽出段階を与えて比較の公平性を保つ。(b) 決定的に留める場合、protocolの寄与がprompt + schemaに縮むことをGoalのAssumptionへ明記し、撤退条件の判定基準に反映する。
- このspikeはT-001とT-002より前に置く。契約設計への投資前に答えが必要。

### P-104 reviewerの独立性を確定する (Blocking 4)

- protocol authorは一切採点しない。Risks and Responsesの「protocol authorが採点しないrunを設ける」を「protocol authorはいかなるrunも採点しない」へ改める。
- Unknownsの「構想者とfresh-context Claudeで十分とするか」を、Review Requirementsと矛盾する候補として削除するか、構想者をreviewerではなくGate 4の判断者として位置づけ直す。
- Route conditionをClaudeで実行する場合、reviewerの一方に別model familyまたは人間を充てる。同一model familyしか用意できない場合は、それを限界としてGoalとEvidenceへ明記し、SC-007の主張範囲を縮小する。

### P-105 SC-006の閾値を上限干渉のない形へ書き換える (Blocking 5)

- primary metricを1つに固定する。推奨は「見逃したblocking条件の削減率」。例: baselineの見逃し件数に対しRouteが50%以上削減する。上限90%に干渉せず、baselineが強い場合も意味を保つ。
- 絶対値90%はsecondary指標へ降格する。
- 「baselineを20 percentage points以上上回る」を残す場合、「ただしbaselineが70%を超える場合は削減率基準を適用する」という但し書きを加える。
- Goal側SC-006の本文も同時に改訂が必要。Plan側だけの修正では整合しない。

### P-106 検出力と解析単位を事前登録する (Medium: 統計)

- 条件あたりのrun数を明示する。総12回ではなく、条件あたり12回以上を推奨。
- 解析単位をfixture caseとし、run間の相関を無視しない。case単位の検出率と正確区間を報告する。
- 到達可能なnで有意差が示せない場合、それを事前に認め、結果を推測統計ではなく記述統計として扱うことをGoalとPlanの両方へ明記する。事後に基準を緩めるより誠実で、Gate 3のfreezeとも整合する。

### P-107 SC-004とMaterialization Previewへ実行経路を与える (Medium)

- T-006の後にtaskを追加する。採用されたHuman Responseに対してMaterialization Previewを生成し、手動materialization後にReconciliation Resultを実際のBelay state (ID、link、status、`belay coverage --format json`、accepted/deferred scope) に対して生成し、決定的照合が成立するか確認する。
- これを行わない場合、SC-004とMaterialization Previewを本Goalから外し、後続Goalへ移す。定義とvalidatorだけでpassing Evidenceを主張しない。

### P-108 SC-005とSC-002/SC-006の緊張を明示的に解く (Medium)

二択のいずれかをPlanで選ぶ。

- 案A: SC-005を「復唱では得られない帰結の特定」から「判断の具体性」へ再定義する。採点対象を、却下または保留に付された理由が特定のConstraintまたはEvidenceを名指ししているか、承認範囲がProposalより狭く限定されているか、とする。Routeが優秀なほど不利になる構造を解消できる。
- 案B: fixture caseの一部で、Route outputからseeded条件を1件意図的に脱落または誤記させ、人間が捕捉するかを採点する。これは評価専用の注入であり、production利用と両立しないこと、虚偽をBelay Evidenceへ残さないことをPlanへ明記する。先行レビューでUnknownとして提起した論点であり、未解決のまま残っている。

### P-109 「未承認内容の正式記録0件」を検証可能な形へ置換する (Medium)

- 書き込み経路が存在しない以上、この0件は成果ではない。T-003のnegative testsが既に扱う「validatorが未承認materializationを拒否する」件数へ置き換える。
- SC-006のprimary metricからは外し、決定的テストの合格条件として扱う。

### P-110 Gate 3の独立性を確保する (Medium)

- Gate 3のfreeze承認者を、T-004とT-005のactorから分離する。分離できない場合、freeze後の変更履歴をEvidenceとして残し、結果を見た後の変更が皆無であることを検証可能にする。
- baseline promptは、fixtureを見ていない独立reviewerがGate 3で承認する。

### P-111 Phase 6弁別caseをfixtureへ追加する (Medium、先行レビューP-108の継承)

- 人間の依頼がまだ存在しない状態のBelay stateを1 case以上含め、Route conditionとPhase 6相当のIntent Brief conditionを比較する。
- 追加しない場合、Phase 6弁別を本Goalの範囲外とする旨をNon-goalsへ明記し、後続Goalへ送る。現状は「分離する」と書きながら測っていない。

### P-112 出力分量を計測する (Medium)

- run毎にRoute outputの分量 (語数またはtoken数)、required field数、人間が選択に要した提示項目数を記録する。閾値は設けなくてよい。
- 記録がないと、Goal Risksの「認知負荷を再生産」とPlan Risksの「schema bloat」は評価後も判定できない。

### P-113 Delivery Mapへ依存関係と順序を入れる (Low)

- 依存列を追加し、T-000 spike → T-001/T-002 contract → T-004 fixture (holdoutは独立author) → T-003 validator → Gate 3 freeze → T-006 の順序を明示する。
- 現順序ではT-003がT-004より前にあり、validatorとfixtureが相互適応しうる。
- 「12回以上」と「検出率」の単位を事前登録前に確定する。
- standalone packageの言語と配置の決定をT-002の入口gateとして明示する。

## Positive findings

- 先行レビューREV-20260726T101157-001の主要提案が実際に反映されている。P-001 (Goal分割とVisionのdocs移動、Scope §1)、P-002 (完了/見直し/撤退条件、Goalへ追加済み)、P-004 (accountabilityと認知的関与の分離、Goal Constraintsと Problem Hypothesis へ明記)、P-007 (既知解fixtureへの置換)、P-009 (SC-004を決定的照合へ限定、Scope §3)、P-010 (materialize表現の修正、Slice用語の削除)。
- Scope §5の「Evidenceは`decision-response`または同等の限定された事実を記録し、`human understood`とは表現しない」は、先行レビューが指摘した「留保がEvidenceの下流利用へ伝播しない」問題への直接的な対処であり、Goal本文の但し書きより強い。
- 「`stop`、`insufficient-context`、`no-safe-route`を正規の結果として扱う」は、常時候補生成を強制しない設計として適切であり、Goal Constraintsとも整合する。
- 「sourceなしFactをvalidatorで拒否する」は、structured wrongnessに対する決定的で強制可能な防御であり、方針表明にとどまらない数少ない項目。
- Human Gatesが4段階に分離され、「Gate 1: 承認しても実装は開始しない」が明記されている。Proposal採用と実装開始承認の混同を防いでいる。
- 「初期reference implementationはBelay本体から依存されないrepository-localな独立packageまたはharnessとし、production `belay route` commandを追加しない」は、accidental productizationに対する構造的な防御。
- Validation Strategyの「Belay core、CLI、Browse、storage schemaが変更されていないことのdiff review」は、責務分離を検査可能にしている。
- Risks and Responsesが各Riskへ具体的な対処を対で置いており、列挙のみのRisksセクションより実効性がある。
- GOAL-20260726T103644-001 (Browse) がConstraintsで「このGoalはRoute protocol、推論品質、候補生成、評価方法を定義しない」と明示し、本PlanもBrowseを除外しているため、両者の責務分離は双方向に成立している。

## Follow-up

- Owner: 人間 (構想者)。P-101のbaseline強化は、Goalの撤退条件を有効にするか、撤退条件を弱めるかの選択であり、価値判断のためAIが代理決定しない。
- Owner: 人間 (構想者)。P-104のreviewer構成は、構想者自身の役割を決める問題であり、本人の判断を要する。
- Owner: 人間 (構想者)。P-105とP-106は閾値と検出力の受容範囲の決定であり、Gate 3のfreeze前に確定する必要がある。P-105はGoal SC-006本文の改訂を伴う。
- Owner: AI + 人間。P-103のT-000 spikeは、T-001着手前に実施する。結果はDecisionとして記録する。
- Owner: AI + 人間。P-102のfixture再構成は、実履歴からのcase復元をAIが起案し、holdoutは構想者以外が作成する。
- Owner: AI。P-107、P-109、P-112、P-113はPlan改訂案として起案可能。
- Owner: 人間。P-108のA案とB案の選択は評価設計の方針判断であり、B案を選ぶ場合は虚偽注入の範囲を明示的に承認する。
- 本レビューの時点でDelivery Mapは全taskがnot-startedであり、実装は開始されていない。Gate 1 (Plan承認) 前の状態としてこの順序は正しい。

requires_human_review: true

## Outcome

Plan は先行レビューの主要提案を実際に反映しており、Goal からの縮約、Human Gate の分離、Belay core との責務分離、structured wrongness への決定的防御は成立している。実装計画としての骨格は妥当。

ただし評価設計に blocking が5点残る。(1) baseline が Goal の撤退条件より弱く、最も重要な判断ゲートが発火しない。(2) fixture が schema slot と1対1で循環しており、protocol の一般的検出能力を測っていない。(3) Route Input の決定性境界が未定義で、T-002 と T-003 の両方がこの未決定に依存する。(4) reviewer の独立性が Review Requirements と Unknowns の間で自己矛盾し、SC-007 を満たせない。(5) SC-006 の2閾値が干渉し、baseline が70%を超えると Route の品質に関係なく fail する。

このうち (3) は契約設計への投資前に答えが必要であり、P-103 の T-000 spike を Gate 1 と Gate 2 の間へ挿入することを推奨する。(1)(2)(4)(5) は Gate 3 の freeze 前に解決すれば足りるが、(1) と (5) は Goal 本文の改訂を伴うため Gate 1 の承認内容に含める必要がある。

Gate 1 の承認は、上記5点の扱いを決めた上で行うことを推奨する。現状の Plan のまま Gate 3 まで進むと、freeze 後に評価設計を変更できず、結果の解釈が定まらない。
