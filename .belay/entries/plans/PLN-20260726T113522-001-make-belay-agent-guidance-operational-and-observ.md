---
schema_version: 1
id: PLN-20260726T113522-001-make-belay-agent-guidance-operational-and-observ
type: plan
title: Make Belay agent guidance operational and observable
status: completed
created_at: 2026-07-26T11:35:22+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: DEC-20260726T011127-001-separate-agent-guidance-concerns-and-improve-bel
- relation: references
  id: NOTE-20260712T134440-001-workflow-feedback-from-first-external-adoption-m
- relation: references
  id: PLN-20260712T164000-001-deliver-phase-6-assurance-incrementally
- relation: fulfills
  id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
metadata: {}
---

## Tier Classification

- Tier 2。再利用されるagent integration、生成物、doctor診断、評価fixtureを変更するfeature級の作業である。
- CLIの状態遷移、永続化schema、Human-Gated Workflowを変更する場合は本Planの範囲を超えるため、実装前に再計画する。

## Intent Brief

### Problem

- AIエージェントはBelayの方針を参照できても、日常的なtrace操作の具体的な手順を保持せず、操作のたびにCLI helpを探索することがある。
- help参照という観測だけでは、Skill内容不足、Skill未インストール、repository上のinactiveまたはstale、agent runtimeでの未認識、未記載の例外操作を区別できない。
- 現行`belay doctor`の`active`は、repository内の所定pathにあるSkillが期待内容と一致することを示す。agent runtimeがそのSkillを読み込み、triggerしたことまでは証明しない。
- AGENTS.md、Skill、README、CLI helpへ同じ情報を重複させると、説明の長文化とCLI変更時のdriftを招く。

### Desired Outcome

- エージェントがrepository固有の方針、再利用可能なBelay実行手順、完全なCLI構文の参照先を区別できる。
- context取得からEvidenceとcoverage確認までの代表的な操作を、無効な呼び出しや誤った状態遷移なしで完了できる。
- 人間とAIが、生成済み、未導入、repository上でactive、stale、runtimeでの認識不明を混同せず、次の診断または復旧操作を選べる。
- Skill改善の効果を、同じfixtureとrubricによるbaseline/post-change比較で説明できる。

### Success Signals

- routine recipeに含まれる操作では、post-change評価中の無効なCLI呼び出しが0件である。
- Human-Gated Workflow違反が0件で、期待するentry、relation、status、Evidenceがすべて作成される。
- help参照は、Skillに完全構文を複製していない未記載または例外的な操作に限定され、同じroutine commandを反復探索しない。
- Codex/Claude向け生成Skillが共有ソースと一致し、導入、inactive、stale、malformedの各状態をfixtureで識別できる。
- Skillの追加量、help呼び出し数、無効呼び出し数、完了結果をbaselineと比較できるEvidenceが残る。

### Constraints

- DEC-20260726T011127-001の責務分離を維持する。AGENTS.mdはrepository固有の方針と承認ゲート、Skillは再利用可能な手順と代表recipe、CLI helpは完全で正確な構文を担当する。
- Belay coreへLLM呼び出しを追加しない。
- CLI helpの利用自体を失敗として扱わない。
- Human-Gated Workflow、既存entry lifecycle、Evidence freshness、sync conflict safetyを弱めない。
- 既存の共有Skillを正典としてCodex/Claude生成物を導出し、手動三重管理を導入しない。
- baselineが内容不足を示さない場合、説明を増やすことを目的化せず、activationと診断の明確化に変更を限定する。

### Non-goals

- CLIの全commandとoptionをAGENTS.mdまたはSkillへ複製すること。
- AIにBelayの全構文を暗記させること。
- Belay Route、Route Proposal、製品化機能を実装すること。
- Phase 6のIntent Brief、Delivery Map、completion assurance契約を再設計すること。
- agent runtimeがSkillを内部的に読んだことをBelayだけで証明すること。
- 統計的に全model、全version、全実行環境へ一般化すること。

### Assumptions

- This is a hypothesis. 代表的なroutine recipeを共有Skillへ追加すれば、無効な呼び出しと反復的なhelp探索を減らせる。
- This is a hypothesis. repository上のintegration状態、fresh contextでのSkill認識、操作結果を分けて記録すれば、説明不足とactivation不足の誤診を減らせる。
- This is a hypothesis. 4つの固定シナリオをbaseline/post-changeで比較すれば、初期改善の採否を判断するのに十分な定性的Evidenceを得られる。

### Unknowns / Decisions Needed

- Unknown. 現在のhelp参照のうち、Skill内容不足、runtime未認識、慎重な構文確認がそれぞれどの程度を占めるか。
- Unknown. CodexとClaudeの各runtimeで、Skillが利用可能またはtrigger済みであることをどこまで観測可能か。
- Unknown. 文脈costを増やさずに維持できるSkillの適切な長さ。
- Unknown. 既存`doctor`表示とREADMEだけで状態診断が十分か、追加のCLI表示が必要か。
- 人間の判断が必要: baselineが既にSuccess Signalsを満たす場合、recipe追加を最小限にしてGoalを診断・activation改善中心へ縮小するか。
- 人間の判断が必要: CLIの新しい診断commandまたは状態語の意味変更が必要になった場合、本Planを改訂して別Decisionを作るか。

## Scope and Approach

1. source変更前に評価scenario、rubric、現行Skillとintegration状態を固定し、baselineをEvidenceとして残す。
2. baseline findingを、`content-gap`、`not-installed`、`repository-inactive`、`stale`、`runtime-recognition-unknown`、`expected-help`、`invalid-invocation`、`workflow-error`へ分類する。
3. AGENTS.md、共有Skill、CLI helpの責務契約をdurable documentationと生成ソースで一致させる。
4. 共有Skillへroutine workflowの短いrecipeと復旧判断を追加し、完全なoption referenceはCLI helpへ委ねる。
5. install、inactive、active、stale、malformedの意味と復旧手順をREADME、help、doctor出力の既存境界内で明確にする。
6. 生成物とfixtureを更新し、同じscenarioをfresh contextで再実行する。
7. Goal、Plan、Decision、diff、scenario結果、Evidenceをfresh contextで照合し、人間がGoal達成を受け入れる。

## Pre-registered Evaluation Scenarios

| ID | Scenario | Required operations | Expected observable outcome |
| --- | --- | --- | --- |
| S-001 | Context and planning orientation | `context compile` fallback、targeted search/show、Tier判定、Goal lint | broad scanなしで関連Goal/Decisionを取得し、実装ゲートを越えない |
| S-002 | Trace lifecycle | entry作成、`fulfills`/`supports`/`references` link、status更新 | 指定type、relation、statusを持つtraceが作成される |
| S-003 | Sync and conflict recovery | managed Markdown編集後のsync、drift確認、競合時の両側確認と明示的prefer | 未解決競合を上書きせず、意図したSource of Truthへ復旧する |
| S-004 | Assurance and integration diagnosis | Evidence記録、verify status、coverage、doctor、inactive/stale fixture | EvidenceとGoal coverageが対応し、integration状態と復旧方法を正しく説明する |

### Evaluation Method

- source変更前後で同じprompt、fixture、期待結果、採点rubricを使う。
- 各scenarioをbaselineとpost-changeで1回ずつ、合計8回、毎回fresh contextと隔離fixture repositoryで実行する。
- CodexとClaudeを2scenarioずつ担当させ、同じscenarioのbaseline/post-changeは同じagent系統で比較する。
- 各runで、help呼び出し、無効なinvocation、exit category、作成artifact、relation、status、gate違反、復旧結果を記録する。
- runtime内部でSkillが読まれたという主張は、agent surfaceが明示的に提供する場合だけFactとして記録する。それ以外はUnknownとし、行動結果から内部状態を断定しない。
- 8回は初期の定性的比較であり、model一般化または統計的有意性の根拠として使用しない。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-003, SC-004 | S-001からS-004、分類語彙、rubric、現行Skill sizeとbaseline 4 runをsource変更前に固定する | AI | verified | EVD-20260726T132738-001 (corrective provenance)、versioned fixture/rubric、baseline run log |
| T-002 | SC-001 | AGENTS.md、共有Skill、CLI help、README/doctorの責務と状態語の意味を明文化し、重複と矛盾を除く | AI + Human | verified | EVD-20260726T132738-002 (corrective provenance)、focused documentation review、DEC-20260726T011127-001との照合 |
| T-003 | SC-002 | context、entry、link、status、sync、Evidence、coverage、conflict recoveryの代表recipeと判断基準を共有Skillへ追加する | AI | verified | EVD-20260726T132739-001 (corrective provenance)、Rust 1.87 CLI integration tests、deterministic S-003/S-004 replay |
| T-004 | SC-003 | generated、installed/inactive、active、stale、malformed、runtime recognition Unknownを区別し、既存commandでの診断と復旧導線を整える | AI | verified | EVD-20260726T132739-002 (corrective provenance)、doctor fixture、README/help review |
| T-005 | SC-005 | 共有ソースからAGENTS snippetとCodex/Claude Skillを再生成し、内容一致とdrift検出を自動検証する | AI | verified | EVD-20260726T132739-003 (corrective provenance)、generated artifact equality、stale/missing/malformed fixture tests |
| T-006 | SC-004 | post-change 4 runをfresh contextで実行し、baselineとhelp利用、invalid invocation、artifact、gate結果を比較する | AI | verified | EVD-20260726T141140-002。独立Claude再実行を含む4/4完遂、invalid 0、gate違反0。helpは全件non-repetitive expected-helpだが、baselineとの完遂数が異なるため因果的な削減はUnknown |
| T-007 | SC-001, SC-002, SC-003, SC-004, SC-005 | Rust 1.87 fmt、clippy、all-target tests、rebuild、doctorを実行し、結果をCriterion単位のEvidenceとして記録する | AI | verified | EVD-20260726T132739-006 (corrective provenance)、SC-001/002/003/005のpassing Evidence、SC-004はEVD-20260726T141140-001でpass。Goal-level test/metricとWork EvidenceはEVD-20260726T141307-001から003 |
| T-008 | SC-001, SC-002, SC-003, SC-004, SC-005 | fresh contextでGoal、Plan、Decision、diff、生成物、scenario結果、Evidenceを独立reviewし、人間が最終結果を判断する | Independent reviewer + Human | verified | REV-20260726T135148-001-review-operational-and-observable-belay-agent-gu completed。High provenance finding解消、behavioral acceptanceとraw transcript limitationを明示し、EVD-20260727T183606-001で人間の最終受入れを記録 |
| T-009 | SC-003, SC-005 | completion時のrepository integrationを正典と一致させ、最新状態で`belay doctor`がpassすることを再確認する | AI + Human environment | verified | EVD-20260727T183937-001 records the earlier stale diagnosis。EVD-20260727T191548-001 records passing `belay doctor` after Codex Skill refresh; AGENTS、Codex Skill、Claude Skillはすべてactiveでgenerated / installed Codex SHA-256も一致 |

## Decision Gates

- T-001後: 主因がcontent gapかactivation/diagnostic gapかを分類し、T-003とT-004の相対scopeを確定する。
- T-004中: 新しいCLI contractが必要と判明した場合は実装を止め、本Plan改訂とDecisionの要否を人間へ提示する。
- T-006後: post-changeがSuccess Signalsを満たさない場合、文書量をさらに増やす前にruntime認識、prompt、scenario設計のどこが原因かを再診断する。

## Risks and Mitigations

- Skill長文化: routine recipeと判断基準だけを残し、完全な構文はCLI helpへlinkする。変更前後のsizeを記録する。
- duplicated reference drift: 共有Skillを正典とし、生成物一致をtestする。
- activationの過大解釈: repository state、runtime recognition、behaviorを別項目で報告する。
- evaluation overfitting: source変更前にfixtureとrubricを固定し、post-changeで期待結果を変更しない。
- model variance: fresh contextと同一scenarioで比較し、8 runを一般化根拠にしない。
- Phase 6との重複: 既存workflow policyを再設計せず、操作recipeとintegration診断にscopeを限定する。

## Acceptance and Completion

- T-001からT-009がすべて`verified`である。
- 各Success Criterionにfresh Evidenceがあり、`implemented`だけのtaskが残っていない。
- baselineとpost-changeの差、残存Unknown、Skill size差、help利用の内訳を説明できる。
- `belay sync`、`belay doctor`、`belay coverage`がpassingである。
- fresh-context reviewのfindingが解消または人間承認つきでdeferされている。
- 人間が、routine操作の自己案内性が改善し、Human-Gated WorkflowとBelay coreの決定性が維持されたことを受け入れる。
