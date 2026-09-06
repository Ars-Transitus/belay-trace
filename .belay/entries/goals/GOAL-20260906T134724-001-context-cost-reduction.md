---
schema_version: 1
id: GOAL-20260906T134724-001-context-cost-reduction
type: goal
title: context-cost-reduction
status: active
created_at: 2026-09-06T13:47:24+09:00
updated_at: 2026-09-06T13:50:20+09:00
revision: 5
tags: []
links: []
metadata: {}
---

## Summary

- Belay context取得の重複と読み直しを減らし、必要な判断情報を保持したまま入力コンテキストを削減する。

## Success Criteria

- [SC-001] 固定fixtureと計測手順があり、変更前revision、出力bytes、推定tokens、必須情報、追加取得回数を再現可能に比較できる。
- [SC-002] task指定compileとworking-set compileで同一entryの説明を重複掲載せず、canonical参照と必要情報を保持する。重複fixtureでは変更前より推定tokensが減少する。
- [SC-003] focus packetがUnknowns / Decisions Neededを保持し、必須情報をbudgetで黙って欠落させない。収まらない場合は決定的な非成功応答で不足と再取得方法を示す。
- [SC-004] 同一fixtureで前後比較を完了し、必須情報欠落ゼロ、追加取得回数の非増加、重複fixtureで出力量減少をEvidenceで確認する。実課金削減と推定token削減を区別する。

## Constraints

- Deterministic coreを維持し、LLM呼び出し、schema migration、ID標準変更は行わない。
- 制約、Non-goals、未解決事項、Task Acceptance/Verification、Goal item、Evidenceへの参照を短縮のために失わない。
- 実装はTier 3の出力契約変更として、2026-09-06の人間によるPlan指定の実装依頼を承認根拠とする。
- installed skill、agent設定、既存履歴のstatusは変更しない。

## Non-goals

- 第1段階ではSkill分割、メタデータ表示オプション、検索ランキング変更、履歴のarchive、実課金削減率の保証を行わない。

## Assumptions / Unknowns

- Assumption: まずcontext compilerの重複排除と情報保持を改善する。
- Unknown: 実セッション料金への効果。固定fixtureの結果から料金削減率を推定しない。

## Verification

- focused Rust/CLI regression tests、固定fixtureのbefore/after、独立レビュー、最終human acceptance。

## Risks

- 必須情報の保持で出力が増える場合がある。固定fixtureで出力量と追加取得回数を同時評価する。
- budget不足時の非成功応答は既存consumerに影響するため、出力契約の明記と独立reviewを必須とする。
