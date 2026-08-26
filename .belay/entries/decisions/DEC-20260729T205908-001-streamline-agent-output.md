---
schema_version: 1
id: DEC-20260729T205908-001-streamline-agent-output
type: decision
title: streamline-agent-output-contract
status: accepted
created_at: 2026-07-29T20:59:08+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260729T204514-001-streamline-agent-workflow-output
metadata: {}
---

## Decision

- `AGENT_SNIPPET`はTier 2以上をrepository Skillへ導く短いpointerと最小context fallbackだけを持つ。
- `SHARED_SKILL`をCodex／Claude共通のcanonical guidanceとする。
- `belay sync`はchanged outcomeだけを列挙し、totalとunchanged件数をsummaryで返す。
- 新規slugはASCII英数字、最大24文字とし、空の場合は`entry`へfallbackする。
- 既存の非ASCII display IDは引き続きparse可能とし、migrationしない。

## Rationale

- 重複したinstructionとunchanged行、長いIDはAI contextと人間の読解コストを増やす。
- canonical templateから生成・installすることでdoctorのstale検査と配布経路を一致させる。

## Consequences

- 行単位の旧sync出力へ依存する外部scriptはsummary形式への追従が必要。
- 日本語のみの新規titleは可読slugを持たないが、timestampとsequenceで一意性を保つ。

## Scope Boundary

- `goal.rs` scaffold、既存ID、Route protocolは変更しない。
