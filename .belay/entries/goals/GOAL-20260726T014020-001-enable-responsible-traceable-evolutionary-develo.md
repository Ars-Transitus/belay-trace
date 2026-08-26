---
schema_version: 1
id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
type: goal
title: Deliver a safe CLI-first Belay Route MVP
status: active
created_at: 2026-07-26T01:40:20+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
- relation: references
  id: GOAL-20260726T011059-001-make-belay-agent-usage-self-guiding-and-reliable
metadata: {}
---

## Summary

- Belayの型付きtraceを、外部AIと人間が安全に判断へ使えるCLI-firstのRoute runへ再構成する。
- Route MVPは一つの明示primary threadを扱い、途中状態をローカル保存し、承認された結果だけをBelayへmaterializeする。
- モデル進化後の価値、baseline比較、一般化、製品化、Browse UXは後続Goalへ分離する。

## Success Criteria

- [SC-001] versioned Route Input、Assessment、Proposal、Human Response、Materialization Preview、Reconciliation Resultの最小契約が定義され、分類、Belay参照、provenance、authority stateを決定的にvalidationできる。
- [SC-002] Route Inputは明示primary seedから決定的に生成され、Constraint衝突、矛盾するDecision、未検証項目、根拠のないAssumption、未承認範囲を外部AIが出典つきで扱える。
- [SC-003] Human Responseは`accept`、`revise`、`reject`、`defer`と承認範囲を表現し、input fingerprintとproposal revision/hashへ結び付く。未承認またはstaleなProposalはmaterializeできない。
- [SC-004] Materialization Previewは、正式なBelay変更を実行前に決定的に列挙する。Reconciliation Resultは実際のID、link、status、Evidence coverage、accepted/deferred scopeと照合し、差異を残す。
- [SC-005] Route runは`.belay/state/route/<run-id>/`から同一端末で再開できる。run bundleが失われてもprimary seedと現在のBelay成果物から新しいInputを再生成でき、非決定的なAssessmentとProposalの同一性は保証しない。
- [SC-006] CLIだけで、run開始、外部AI output取込、validation、人間応答取込、preview、明示承認後のmaterialization、reconciliationまで実行できる。
- [SC-007] 正常系と、unsupported schema、missing reference、stale input、proposal mismatch、unapproved materialization、partial write/reconciliation mismatchを決定的テストで検出できる。

## Constraints

- Belayを事実、判断、履歴、EvidenceのSource of Truthとして維持し、Route run bundleを正式記録として扱わない。
- Belay coreはlocal-firstかつdeterministicとし、LLM呼び出し、API key管理、非決定的な意味判断を組み込まない。
- Route reasoningとProposal生成はAgent Skillまたは外部agent layerが担う。
- Route Inputは明示primary seedを必須または既定経路とし、関連threadは必要なものだけを追加する。
- 既存のPlan承認、実装開始、Issue、Pull Request、mergeのHuman Gateを維持する。
- raw run bundleは`.belay/state/route/`へ保存し、Git、端末間、clone間の共有を保証しない。
- 意味的に安全なRouteがない場合、`stop`、`insufficient-context`、`no-safe-route`を正常な結果として扱う。

## Non-goals

- モデル進化後にもRouteが独立価値を持つか評価すること。
- baseline比較、複数runの統計評価、複数モデルまたは組織への一般化。
- repository-wideな複数タスク委任、優先順位、WIP、通知、統合順序の管理。
- Browse UI、MCP、独立サービス、ホスティング、RBAC。
- Product Map、Requirement、Route Proposalをfirst-class Belay entryとして追加すること。
- 将来のモデル能力に応じたHuman Gate緩和条件を決めること。

## Verification

- 各documentのvalid/invalid fixtureに対するschema、reference、fingerprint、authority validation。
- local runのpause/resumeと、Belay成果物からのInput再生成。
- Human Response、Proposal revision、Materialization Previewのbindingおよびstale拒否。
- previewどおりのmaterializationと、partial failure後のReconciliation Result。
- Belay coreが外部LLM、API key、network accessを必要としないことのdiff review。
- SC-001からSC-007のpassing Evidence、独立実装review、人間による最終受入れ。

## Risks

- 型付きProposalが未承認でも正式な判断に見えるstructured wrongness。
- schemaを増やしすぎ、Route自身が認知負荷を再生産する。
- Human Responseの自由文をAIが承認範囲以上に変換する。
- stale検査とmaterializationの間にBelay stateが変わるTOCTOU。
- local run bundle消失後の再生成結果が以前のProposalと異なる。

## Assumptions

- **This is a hypothesis.** Belayの型付きtraceは、会話履歴よりRoute Inputの再生成元に適している。
- **This is a hypothesis.** 同一端末でのlocal resumeとBelayからの再生成でMVPの再開要件を満たせる。
- **This is a hypothesis.** 人間のチャット応答をAIが構造化し、Belayが対象fingerprint、proposal revision、許可されたactionを検証する分担で安全な承認境界を作れる。

## Unknowns / Decisions Needed

- 6 documentを個別fileにするか、append-only eventを含む一つのrun manifestにするか。
- Route Input生成に既存`context compile`を再利用するか、Route専用の決定的read contractを追加するか。
- materializationを単一atomic transactionにするか、operation単位のidempotencyとreconciliationで扱うか。
- Human Responseの`revise`をProposalの差分として表現するか、新revision生成要求として表現するか。
