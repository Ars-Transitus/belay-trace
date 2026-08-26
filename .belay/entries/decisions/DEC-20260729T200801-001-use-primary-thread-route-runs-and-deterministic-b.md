---
schema_version: 1
id: DEC-20260729T200801-001-use-primary-thread-route-runs-and-deterministic-b
type: decision
title: Use primary-thread Route runs and deterministic Belay core boundaries
status: accepted
created_at: 2026-07-29T20:08:01+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: DEC-20260728T214035-001-prioritize-route-functional-design-and-defer-mod
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Decision

- Route MVPは、1 runにつき一つのprimary threadを扱う。primary threadはGoal、Plan、Decisionなどの明示seedで指定し、必要な関連threadだけを追加参照する。
- repository-wideな複数タスクの委任、優先順位、WIP、通知、統合順序の管理は、Route後の上位frameworkまたはtoolへ分離する。Routeにはprimary threadを選ぶための限定的なactive-thread overviewだけを許容する。
- Routeの決定的処理はBelay coreへ置く。対象はRoute Input snapshot、fingerprint、schema validation、Materialization Preview、Reconciliation Resultである。
- LLMによる意味解釈とRoute Draft生成はBelay coreへ組み込まず、Agent Skillまたは外部agent layerが担う。
- このDecisionはRoute Goal/Planの承認または実装開始承認ではない。

## Rationale

- 一つのprimary threadへfocusすることで、Route MVPがtask managerやmulti-agent orchestratorへ拡散することを防ぐ。
- 自然言語queryだけでは関連Goalを安定して取得できない事例が2026-07-29の再開時に観察され、明示seedを機能契約へ含める必要がある。
- Belay coreを決定的かつlocal-firstに維持しながら、snapshot、provenance、authority boundary、preview、reconciliationを機械的に検証可能にする。
- 意味推論を外部へ分離すれば、モデル変更とBelayの正式状態・検証契約を独立して進化させられる。

## Consequences

- 現在のstandalone evaluation harness中心のPlanは、実際に利用できるdeterministic `belay route` surfaceを含む機能Planへ改訂する。
- Route Inputは明示primary seedを必須または既定経路とし、自然言語queryだけにfocus解決を委ねない。
- Routeの上位に位置するmulti-agent delegation/orchestrationは本GoalのNon-goalとし、後続構想として保持する。

## Source

- Human approval in the 2026-07-29 conversation: 「方向性はOK。推奨の2点もOK。」
