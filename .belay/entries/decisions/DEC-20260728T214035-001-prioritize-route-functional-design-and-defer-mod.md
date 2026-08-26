---
schema_version: 1
id: DEC-20260728T214035-001-prioritize-route-functional-design-and-defer-mod
type: decision
title: Prioritize Route functional design and defer model-evolution value evaluation
status: accepted
created_at: 2026-07-28T21:40:35+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Decision

- Routeの当面の作業では、機能契約と安全なend-to-end flowの設計を優先する。
- モデル進化後にもRouteが独立価値を持つか、強いモデル、AGENTS.md、Skill、Belayだけで代替できるかという比較価値評価は、後続Goalへ回す。
- 次回はRouteの機能設計から再開し、実装可能なPlanへ詰める。
- このDecisionはRoute Planの承認または実装開始の承認ではない。既存のHuman Gateを維持する。

## Rationale

- Human judgment: 現段階では、モデルにプロジェクト履歴を完全かつ継続的に記憶させられないため、外部の記録と追跡を担うBelayの仕組みが必要である。
- Human judgment: Belayが記録を保持しても、人間が関連記録を読み直して現在の主題へ戻る負荷は残るため、判断に必要な状態を再構成するRouteが必要である。
- 2026-07-28の議論では、直近数日の議論を即座に再構成できず、Routeの機能設計からモデル進化後の価値評価へ主題が逸れた。これはRouteが対象とする問題の具体例として観察された。
- AIとの並行作業では、タスクAをAIへ依頼中にタスクBを準備・依頼し、タスクAの応答を処理しながらタスクCを進めるようなinterleavingが起こる。人間だけで各タスクの現在地、前提、未決定、応答待ち、次の判断を保持するのは困難である。

## Consequences

- 現在のRoute GoalとPlanは、機能成立のscopeと比較価値評価のscopeを分離する方向で次回再整理する。
- 当面の検証は、input/output contract、provenance、authority state、stale判定、Human Response、Materialization Preview、Reconciliation Result、安全な承認境界など、機能の成立と安全性へ限定する。
- baseline比較、検出率の優位性、モデル世代への耐性、一般化、製品価値は、機能を利用できる状態にした後の別Goalで扱う。

## Source

- Human direction in the 2026-07-28 conversation: 「明日はRouteの設計から再開」「モデル進化の評価価値については、後に回す」。
