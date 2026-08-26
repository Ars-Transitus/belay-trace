---
schema_version: 1
id: DEC-20260726T011127-001-separate-agent-guidance-concerns-and-improve-bel
type: decision
title: Separate agent guidance concerns and improve Belay usage before Route
status: accepted
created_at: 2026-07-26T01:11:27+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
metadata: {}
---

## Decision

- Belay Routeとは別の改善として、AIエージェントがBelayを正確かつ一貫して利用できるよう、agent integrationを改善する。
- AGENTS.mdはリポジトリ固有の方針、Human-Gated Workflow、禁止事項を担当する。
- belay-trace SKILLは、再利用可能なワークフロー、代表的なcommand recipe、判断基準、復旧手順を担当する。
- CLI helpは、全コマンドとオプションの正確な構文に関するSource of Truthを維持する。
- 改善前に、Skillの未インストール、未activation、stale、内容不足、CLI helpの正常な利用を区別して観測する。
- Belay coreにLLM呼び出しを追加しない。

## Rationale

- 現在のAGENTS統合とSkillはworkflow policyを十分に説明する一方、日常的なtrace操作のcommand recipeが限定的である。
- AIが都度helpを読むという観測だけでは、Skill内容不足とSkill activation不足を区別できない。
- AGENTS.mdへCLI仕様を複製すると長文化とdriftを招くため、規範、手順、完全な構文を分離する。
- Routeの仮説検証前にBelayの利用経路を安定させることで、Route側でも同じintegration問題を再発させるリスクを下げられる。

## Alternatives Considered

- AGENTS.mdへ全操作例を追加する案は、repository policyとgeneric CLI guidanceが混在し、更新コストが高いため採用しない。
- CLI helpを読ませる現状だけを維持する案は、無効な呼び出しやworkflow判断ミスの改善余地を検証できないため採用しない。
- Belay coreへLLM支援を追加する案は、local-firstかつdeterministicな設計原則に反するため採用しない。

## Consequences

- Skillは多少長くなる可能性があるため、代表的なrecipeに限定し、完全なreferenceはCLI helpへ残す。
- Skillの内容だけでなく、導入状態、activation、doctorによるdrift検出も評価対象になる。
- 実装前にTier 2 Plan、Intent Brief、Delivery Map、評価シナリオを作成し、人間へ訂正機会を提示する。
- Belay Routeのアーキテクチャおよび製品方針はこのDecisionでは確定しない。

## Evidence Basis

- Human observation: AIはBelayの使い方を保持せず、操作時に都度helpを参照する傾向がある。
- Repository inspection: 現行SkillはTier分類、Frame、Map、Execute、Assureを中心に説明し、CLI操作例は限定されている。
- Unknown: 不要なhelp参照、無効なCLI呼び出し、Skill未activationの発生率と主要因。
