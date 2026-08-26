---
schema_version: 1
id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
type: goal
title: Make Belay agent usage self-guiding and reliable
status: completed
created_at: 2026-07-26T01:10:59+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- AIエージェントがBelayの基本操作とHuman-Gated Workflowを、反復的なhelp探索や無効なCLI呼び出しに依存せず、正確かつ一貫して実行できる状態にする。

## Success Criteria

- [SC-001] AGENTS.md、belay-trace SKILL、CLI helpの責務が明文化され、方針、実行手順、完全な構文のSource of Truthが重複なく区別される。
- [SC-002] belay-trace SKILLが、context取得、entry作成、link、status、sync、Evidence記録、coverage、conflict対応について、実行可能な代表的command recipeを提供する。
- [SC-003] Skillのインストール、activation、stale状態を人間とAIが識別でき、未導入または未起動を説明不足と混同しない。
- [SC-004] 事前定義した代表シナリオで、エージェントが無効なBelay CLI呼び出しを行わずにtrace操作を完了し、help利用が未記載または例外的な操作に限定される。
- [SC-005] 生成テンプレート、Codex/Claude向けSkill、AGENTS統合、READMEおよびdoctor検査が整合し、更新後のdriftを検出できる。

## Constraints

- Belay coreは決定的なままとし、LLM呼び出しを追加しない。
- AGENTS.mdはリポジトリ固有の方針と承認ゲート、SKILLは再利用可能な実行手順、CLI helpは正確な構文を担当する。
- CLI helpを読むこと自体を失敗と扱わず、不要な反復探索、無効な呼び出し、誤った状態遷移の削減を目的とする。
- Belay Routeの設計、製品化、Route Proposal生成はこのGoalに含めない。

## Non-goals

- CLIの全オプションをAGENTS.mdまたはSKILLへ複製すること。
- AIがBelayの全コマンドを暗記すること。
- Belay Routeを実装すること。
- Human-Gated Workflowを弱めること。
- 特定の単一エージェントベンダーだけに最適化すること。

## Verification

- 現状のSkill導入・activation・help利用・無効呼び出しを代表シナリオでベースライン測定する。
- 更新後に同じシナリオをfresh contextで実行し、command選択、引数、relation、status、Evidence記録を比較する。
- CodexおよびClaude向け生成Skillと共有ソースの一致をテストする。
- agent integrationを有効化したrepositoryでbelay doctorがpassingとなり、意図的にstale化したfixtureを検出することを確認する。

## Risks

- Skillを長くしすぎるとtrigger後のcontext costが増え、重要な指示が埋もれる。
- command例を複製するとCLI変更時にstaleになる。
- activation不足を説明不足と誤診すると、内容を増やしても利用性が改善しない。
- 単一の成功例だけではモデル、環境、セッション差に対する再現性を確認できない。
