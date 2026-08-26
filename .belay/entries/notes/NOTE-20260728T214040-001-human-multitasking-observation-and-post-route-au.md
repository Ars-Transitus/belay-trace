---
schema_version: 1
id: NOTE-20260728T214040-001-human-multitasking-observation-and-post-route-au
type: note
title: Human multitasking observation and post-Route autonomy direction
status: active
created_at: 2026-07-28T21:40:40+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: DEC-20260728T214035-001-prioritize-route-functional-design-and-defer-mod
- relation: supports
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Human Observation

- Belayへ記録が存在していても、人間とAIが数日分の議論をすぐに再構成できず、主題へ戻るまでに追加の探索と会話が必要になる。
- AIへ複数タスクを委任すると、依頼、待機、別タスクの準備、返答処理、追加作業がinterleaveする。各タスクの現在地、前提、判断待ち、次のactionをツール支援なしで維持することは難しい。
- 2026-07-28のRoute議論自体がこの問題を示した。Routeの機能設計を確認したかったが、既存GoalとPlanに価値評価が混在していたため、モデル進化後の独立価値評価へ議論が引かれ、以前の「価値評価は後続へ回す」という主題へ戻るのに時間を要した。

## Human Judgment

- モデルへ完全な継続記憶を期待できない現段階では、外部記憶、provenance、Goal、Plan、Decision、Work、Evidenceを保持するBelayが必要である。
- Belayだけでは人間の理解と判断準備が記録量に追いつかないため、現在の判断に必要な情報、矛盾、Unknown、選択肢、応答待ちを再構成するRouteが必要である。

## Long-term Direction

- BelayとRouteの開発が一段落した後、両者を実際に使いながら、AIへより独立して仕事を任せるためのframework、tool、またはworkflowを検討する。
- 将来の仕組みは、複数の並行タスクについて、現在地、委任先、待機状態、Human Gate、未解決事項、成果の再統合を扱う可能性がある。
- **This is a hypothesis.** Belayが永続的な作業記憶、Routeが判断面の再構成を担えば、その上位レイヤーとして複数AIタスクの委任と再統合を支える仕組みを構築できる。

## Resume Point

- 次回はRouteの機能設計から再開する。
- まず、Route Input、Assessment / Proposal、Human Response、Materialization Preview、Reconciliation Resultの責務と状態遷移を整理し、実装可能なPlanへ落とす。
- モデル進化後の比較価値評価は次回の主題に含めない。
- このNoteはPlan承認または実装開始承認を意味しない。
