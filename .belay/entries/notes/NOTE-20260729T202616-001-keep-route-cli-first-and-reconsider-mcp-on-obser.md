---
schema_version: 1
id: NOTE-20260729T202616-001-keep-route-cli-first-and-reconsider-mcp-on-obser
type: note
title: Keep Route CLI-first and reconsider MCP on observed need
status: active
created_at: 2026-07-29T20:26:16+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: DEC-20260729T200801-001-use-primary-thread-route-runs-and-deterministic-b
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: supports
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Current Direction

- Belay coreにMCP serverまたはMCP Apps UIを組み込まない。
- Routeの決定的処理はBelay coreのRust APIとCLIとして設計する。
- 初期RouteはCLI-firstで進め、AI agentはCLIを介してRoute Input生成、validation、Materialization Preview、Reconciliationを実行する。
- 人間との対話はAI chat interface上のtext protocolを標準fallbackとして扱う。

## MCP Boundary

- MCPはBelay coreの必須依存ではなく、Codex、Claudeその他のMCP clientへRoute toolとinteractive UIを公開するoptional adapterである。
- 将来MCPを導入する場合は、`belay-route-mcp`または同等の薄いadapterがBelay coreのRust APIまたはCLIを呼ぶ構成を優先する。
- MCP App UI、button、multi-select、inline confirmationはpresentation concernであり、RouteのJSON contract、authority boundary、validation、Preview、ReconciliationのSource of Truthにしない。
- MCPまたはUI非対応のsurfaceでも同一Route workflowがtextで完結できるようにする。

## Reconsideration Signals

MCPは、次の必要性が実際に観測された場合に別Goal、Plan、Decision、人間承認を経て再検討する。

- text protocolでProposal選択またはPreview承認の取り違えが反復する。
- CodexとClaudeの両方で同じRoute tool contractを再利用するためのadapter重複が増える。
- click可能な選択UIが、人間の確認負荷または誤操作を明確に減らす。
- CLI process invocation、structured result受け渡し、session lifecycleの管理がAgent Skillだけでは不安定になる。
- remote、mobile、または複数repositoryを跨ぐ利用が必要になる。

## Constraints

- MCPを後送りすることは、RouteのJSON schemaやrenderer分離を後回しにする理由にはしない。将来adapterを追加できるtransport-neutral contractを維持する。
- このNoteはRoute Goal/Planの承認または実装開始承認ではない。

## Source

- Human direction in the 2026-07-29 conversation: 「MCPは必要になった時に再検討する。まずはCLIで進めよう」。
