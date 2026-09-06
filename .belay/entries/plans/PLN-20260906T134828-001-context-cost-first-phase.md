---
schema_version: 1
id: PLN-20260906T134828-001-context-cost-first-phase
type: plan
title: context-cost-first-phase
status: active
created_at: 2026-09-06T13:48:28+09:00
updated_at: 2026-09-06T19:44:22+09:00
revision: 9
tags: []
links:
- relation: fulfills
  id: GOAL-20260906T134724-001-context-cost-reduction
metadata: {}
---

## Intent Brief
### Problem
- compileはGoal説明とranked contextを重複出力し、組立後の末尾切捨てで情報を失う。focusは独立したUnknowns節を抽出しない。
### Desired Outcome
- 必要な情報が一度で取得でき、比較可能な根拠付きでcontext出力量を減らす。
### Success Signals
- 固定fixtureで重複削減、必須情報欠落ゼロ、追加取得回数非増加。計測結果をEvidence化する。
### Constraints
- Tier 3。2026-09-06の人間による本Plan指定の実装依頼を承認根拠とする。
- coreは決定的。schema、canonical ID、show契約、FTS rankingを維持する。
- 各Taskは専用worktreeで1 Work、独立review、最大1回のfresh fixer。rootが状態と統合を担当。
- T-001/T-004: implement_medium (gpt-5.6-terra, medium), review_medium (gpt-5.6-sol, low)。
- T-002/T-003: implement_high (gpt-5.6-sol, low), review_high (gpt-5.6-sol, medium)。dispatch時にroutingを再確認。
### Non-goals
- Skill分割、表示オプション追加、検索関連性変更、archive、installed設定更新、push/deploy。
### Assumptions
- 第1段階をcompilerに限定。Skill分割等は比較結果を見て別Planで扱う。
- 重複fixtureの推定tokens減少を要求するが、未計測の削減率を約束しない。
### Unknowns / Decisions Needed
- Human decision: 2026-09-06、本Planを実装する。
- 実課金効果は後続実セッション計測までUnknownとする。
- 出力不足時は既存Validation分類の非成功応答とし、required estimateと再取得コマンドを示す設計を提案する。

## Delivery Map
| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | 固定fixtureとbaseline計測 | implement_medium | blocked | implementer rounds 2/2 timed out before settlement |
| T-002 | SC-002 | compile重複排除と配分 | implement_high | not-started | regression testsと出力量比較 |
| T-003 | SC-003 | focus必須情報保持 | implement_high | not-started | Unknowns/overflow CLI tests |
| T-004 | SC-004 | 統合比較と評価記録 | implement_medium | not-started | before/after reportとfresh Evidence |

## T-001
- Objective: 改善前の出力と情報充足度を再現可能に測る。
- Scope: evaluation/context-cost/ のfixture、測定script、README、baseline report。runtime変更なし。
- Steps: 重複Goal、working set、多Task Plan、Unknowns、長い制約、Evidence多数、日英混在、低budgetを含むfixtureを作る。変更前revisionとbinaryを特定し固定する。出力bytes、src/markdown.rsと同じ推定tokens、必須項目の在否、事前定義した取得手順の追加コマンド回数を保存する。
- Acceptance: 各fixtureの入力、期待必須情報、budget、取得手順、baseline revisionが明示され、再実行で同じ結果になる。実モデルtokenや料金と混同しない。
- Verification: fixture再実行と結果一致、独立review。将来比較用baselineを固定する。
- Prior failure: Herdr runnerがagent status待ちでtimeout。保存lane w5:p5、worktree /private/var/folders/lp/cf8008053v12_7knfvy17jy40000gn/T/erwin-routed/17f2e2fc15626fd2-0604552903a9。WRK-20260906T135430-001はsettlement metadataなしでabandoned。
- Recovery: 2026-09-06、人間が旧pane終了、旧Work abandoned、clean baseからのfresh reimplementationを承認。
- Recovery result: fresh workerはGit stage承認UIで停止し、親runnerがstatus timeout。pane w5:p6とworktree /private/var/folders/lp/cf8008053v12_7knfvy17jy40000gn/T/erwin-routed/17f2e2fc15626fd2-f7e83d36c9a3を検査用に保持。implementer round budget 2/2を消費し、settlement metadataとcommitは未生成。

## T-002
- Objective: task指定compileとworking-set compileの同一entry再説明を除く。
- Scope: src/context.rs、tests/cli.rs、必要な出力契約記述src/cli.rs。T-001 fixtureを入力として読む。
- Steps: T-001後に実施。canonical entry identityでGoalとranked候補を重複排除し、必要な追加情報は一つの表現へ統合する。headerを含む全体budgetで選択し、必須部分を先に確保する。Next参照やcanonical参照の再使用は説明重複と区別する。任意候補の省略は件数と追加取得先を示す。必須部分が収まらなければValidation応答を返し、末尾切捨てで成功扱いしない。
- Acceptance: 同一entryの説明が二重掲載されない。保持対象は採用Goalの制約/Non-goals/未解決事項、working setのNext参照。ランキング自体は変えない。重複fixtureの推定tokensはbaselineより少なく、非重複fixtureの必要情報は欠落しない。
- Verification: task/working-set双方、empty、重複、低budgetのfocused regression testsとbaseline比較。独立high review。

## T-003
- Objective: focusを安全に渡せる完結したTask packetにする。
- Scope: src/context.rs、tests/cli.rs、src/cli.rsのbudget/失敗契約記述。T-002の共通budget処理を再利用する。
- Steps: T-002後に実施。PlanのUnknowns / Decisions Neededと既存の統合Assumptions / Unknowns表記を扱う。Constraints、Non-goals、Assumptions、Unknowns、Task definition/section、Goal item、現在選択されるEvidence参照を必須として計測する。budget未満でも内容を切らず、収まらない場合はValidation非成功応答にrequired estimateと増額したfocusコマンドを含める。推定値であることを明示する。
- Acceptance: 独立Unknowns節が保持される。Task Acceptance/VerificationとGoal itemが欠落した成功packetを返さない。境界budget、zero、小budget、日本語で挙動が決定的。unknown fragmentや曖昧Goal mappingは既存どおり失敗する。
- Verification: exact-fit/不足/Unknownsありなし/多数EvidenceのCLI tests、既存focus regression tests、独立high review。

## T-004
- Objective: 効果と制限を同条件で評価し、根拠を残す。
- Scope: evaluation/context-cost/ の比較scriptとreport。runtimeやskillの追加変更なし。
- Steps: T-002/T-003後に固定baselineと変更後revisionでT-001手順を実行する。fixture別のbytes、推定tokens、追加取得回数、必須情報欠落を比較する。overflowの非成功応答と推奨再取得を取得回数に含める。通常取得と意図的overflowを別記し、失敗や悪化を隠さない。
- Acceptance: 全fixtureの必要情報欠落ゼロ、同じ情報取得完了条件で追加取得回数非増加、重複fixtureの推定tokens減少。満たさなければ未達として返し、基準を後から緩めない。料金はUnknownと明記し、取得できないcached/input/output tokenを捏造しない。
- Verification: focused checks、cargo test、独立review、Belay Evidence。rootが全SCを照合し、最終human acceptanceを別途記録する。

## Deferred improvements
- Skillの用途別分割: src/agent.rsの生成と導入契約を別途設計し、入口短縮と追加読込回数を比較する。
- Metadata簡素化: canonical参照を維持した詳細表示契約を別途設計する。
- 関連性改善: 過去Goal/失敗の必要性を評価してから検索・選択契約を別途変更する。
