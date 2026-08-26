---
schema_version: 1
id: PLN-20260729T204610-001-align-agent-templates-and-output
type: plan
title: align-agent-templates-and-output
status: approved
created_at: 2026-07-29T20:46:10+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260729T204514-001-streamline-agent-workflow-output
metadata: {}
---

## Intent Brief

### Problem

- Agent guidanceの改善が生成物へ直接反映され、canonical templateと不一致になっている。
- `belay sync`のunchanged行と長い非ASCII slugが、AIのcontextと人間の読解を浪費する。

### Desired Outcome

- Guidanceは短いAGENTS pointerとcanonical Skillへ分離され、CLI自身が低ノイズな出力と短いIDを保証する。

### Success Signals

- Goal SC-001〜SC-005がfresh validationでpassする。
- `grep -v ': unchanged'`なしでsync出力が実用的な量になる。

### Constraints

- 既存の承認境界、deterministic core、managed Markdown schemaを変えない。
- Route記録を本改修へ混在させない。

### Non-goals

- `goal.rs` scaffoldの変更。
- 既存IDのmigration。
- Route機能の実装。

### Assumptions

- This is a hypothesis: syncの個別`unchanged`行に依存する正式な外部contractはない。
- timestampと連番がentry IDの一意性を担保するため、slugの`entry` fallbackは安全である。

### Unknowns / Decisions Needed

- Unknown: 非公開の外部scriptが旧sync出力を解析しているか。
- Human decision received: 指定された4改修を実装し、scaffoldは変更しない。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
|---|---|---|---|---|---|
| T-001 | SC-001 | `AGENT_SNIPPET`を短いSkill pointerへ変更する | Codex | verified | EVD-20260729T205732-001 |
| T-002 | SC-002 | `SHARED_SKILL`を承認済みSkill内容へ更新し生成物を揃える | Codex | verified | EVD-20260729T205732-001 |
| T-003 | SC-003 | syncのunchanged詳細を抑制しsummaryを拡張する | Codex | verified | EVD-20260729T205732-001 |
| T-004 | SC-004 | slugifyをASCII限定・24文字上限へ変更する | Codex | verified | EVD-20260729T205732-001 |
| T-005 | SC-005 | fmt、all-target tests、rebuild、doctor、conflict確認を実行する | Codex | verified | EVD-20260729T205732-001 |

## Acceptance Criteria

- 生成物とcanonical constantsが一致する。
- `belay sync`はchanged outcomeだけを列挙し、`Sync completed: N entries (M unchanged)`を出す。
- 日本語のみのtitleは`entry`、長いASCII titleは24文字以内になる。
- `goal.rs` scaffoldに差分がない。
