---
schema_version: 1
id: GOAL-20260729T204514-001-streamline-agent-workflow-output
type: goal
title: streamline-agent-workflow-output
status: active
created_at: 2026-07-29T20:45:14+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Agent guidanceのcanonical template、sync出力、entry slug生成を簡潔で自己整合的にする。

## Success Criteria

- [SC-001] `AGENT_SNIPPET`はTier 2以上でrepository Skillを参照する5〜10行程度のpointerと、Skill未導入時の最小context fallbackだけを生成する。
- [SC-002] `SHARED_SKILL`はcommand referenceとToken disciplineを含むcanonical sourceとなり、Codex／Claudeの生成・installed Skillが一致する。
- [SC-003] `belay sync`は変更のないentryを個別出力せず、総entry数とunchanged件数をsummaryで報告する。
- [SC-004] 新規entry slugはASCII英数字だけを保持し、最大24文字とし、空の場合は`entry`へfallbackする。
- [SC-005] 対象テスト、全体テスト、再生成後の`belay doctor`が成功し、`jj`上に未解決conflictがない。

## Constraints

- Belay coreの決定性と既存のhuman approval gatesを維持する。
- Route設計記録は独立したChangeとして保持する。
- 生成物の手編集をcanonical sourceの代用にしない。

## Non-goals

- Route protocolまたはモデル進化価値評価の設計。
- `goal.rs`のentry scaffold変更。
- 既存entry IDのmigrationまたはrename。

## Verification

- `cargo fmt --all --check`
- `cargo test --all-targets --locked`
- working-copy binaryによる再生成と`doctor`
- sync出力とslugifyのfocused tests

## Risks

- syncの行単位出力を解析する外部scriptがある場合、summary形式変更の追従が必要。
- 非ASCIIだけのtitleは可読slugを失うが、timestampと連番による一意性は維持される。
