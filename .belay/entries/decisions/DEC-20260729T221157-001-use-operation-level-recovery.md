---
schema_version: 1
id: DEC-20260729T221157-001-use-operation-level-recovery
type: decision
title: use-operation-level-recovery
status: accepted
created_at: 2026-07-29T22:11:57+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
- relation: supports
  id: WRK-20260729T215937-001-implement-route-mvp
metadata: {}
---

## Decision

- Route MVPのmaterializationは、run全体のbatch transactionではなく、operation単位のatomicityとidempotencyを採用する。
- `create-entry`はrun IDとoperation IDをentry metadataへ記録し、再実行時の重複作成を防ぐ。
- `link`と`set-status`は既存Belay mutationのidempotencyを利用する。
- 各applyは全operationの`applied`、`unchanged`、`failed`とalias解決結果をReconciliation Resultへ保存する。
- 部分成功後の再実行では、前回Reconciliationに保存したpost-apply fingerprintと現在のBelay stateが一致する場合だけ続行する。
- Human Responseの`revise`は直接materializeしない。requested changesをもとに新しいProposal revisionを生成し、新hashへ改めてHuman Responseを結び付ける。

## Rationale

- 既存Belay mutationはSQLiteとmanaged Markdownを個別に安全更新するため、複数operationを一つのSQLite transactionへ包むとmirror更新とrollback contractを壊す。
- operation journalとpost-apply fingerprintにより、部分失敗を隠さず、外部変更を上書きせず、安全に再開できる。
- `revise`を承認として扱うと、AIが人間の修正要求を具体的operationへ変換する際に承認範囲を拡大できてしまう。

## Consequences

- Materialization Previewはoperation順序を契約に含め、aliasは作成後のoperationだけが参照できる。
- Route applyはpartial failureをReconciliationへ残して非zero終了する。
- batch全体のall-or-nothing保証はMVPのNon-goalとし、必要性が観察された場合にstore transaction APIを別設計する。

## Source

- Approved Plan includes partial-failure reconciliation and idempotent resume.
- Implementation inspection of existing SQLite and managed Markdown mutation boundaries on 2026-07-29.
