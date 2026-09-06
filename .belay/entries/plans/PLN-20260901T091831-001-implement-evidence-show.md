---
schema_version: 1
id: PLN-20260901T091831-001-implement-evidence-show
type: plan
title: implement-evidence-show
status: active
created_at: 2026-09-01T09:18:31+09:00
updated_at: 2026-09-01T09:51:19+09:00
revision: 8
tags: []
links:
- relation: fulfills
  id: GOAL-20260901T091831-001-direct-evidence-retrieva
metadata: {}
---

## Intent Brief

### Problem

- `belay verify record`がNDJSONへ保存したEVDを`belay show EVD-...`では取得できない。`rebuild`はEvidenceをindexするが出力はmanaged Markdown件数だけで、`show`はindex済みEvidenceにも対応しないため、agentが正常なEvidenceを消失と誤診しWorkをblockedへ変更した。

### Desired Outcome

- `belay show`をEvidence IDにも安全に拡張し、sync/rebuildのindex可視性と生成Skillの契約を揃える。Herdr固有settlementは引き続きNDJSONを直接検証する。

### Success Signals

- mirror-only EVDを`show`でき、同じrecordがsync後の`verify status <target>`にも現れる。
- ambiguous/duplicate/malformed Evidence fixturesがfail-closedになる。
- rebuild出力がEntryとEvidenceの件数を別表示する。
- help/docs/generated Skillの例が実際のCLI behaviorと一致する。

### Constraints

- read-only showとappend-only Evidenceを維持し、Entry resolverの既存behaviorを変えない。
- SQLiteを唯一のretrieval sourceにせず、mirror-onlyのdurable recordを確認可能にする。
- syncはtransaction boundary内でEvidence indexを更新し、validation failure時に旧indexを保持する。

### Non-goals

- Evidenceをmanaged Entryへ変換すること、status/link mutationを許可すること、Herdr runnerをshow依存にすること、coverage semanticsを変更することは含めない。

### Assumptions

- Hypothesis: exact/unique-prefix dispatchをCLI境界でEntry resolverとEvidence resolverへ分けることで、既存`ShownEntry` APIを壊さず追加できる。
- Fact: `evidence::read_mirrors`と`rebuild_into`は既にNDJSON parse/validationとSQLite挿入の基礎を持つ。

### Unknowns / Decisions Needed

- Unknown: 将来schema version用renderer。現行v1以外は明示エラーとし、今回のimplementerが推測で互換表示を追加しない。
- Human decision: 2026-09-01 chatでPlan対応の開始を指示。Tier 3 retrieval/sync変更の実装を開始する。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | Difficulty | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | NDJSON-backed Evidence resolverとread-only rendererを追加する | implement_high | high | verified | EVD-20260901T095056-001 |
| T-002 | SC-002 | EVD prefix/error boundaryをfail-closedで固定する | implement_high | high | verified | EVD-20260901T095056-001 |
| T-003 | SC-003 | sync/rebuildのEvidence index整合性と件数表示を実装する | implement_high | high | verified | EVD-20260901T095056-001 |
| T-004 | SC-004 | help、docs、generated Skillを新契約へ更新する | implement_medium | medium | verified | EVD-20260901T095056-001 |

## T-001

- Objective: exact EVDをSQLite追随状態に依存せずdurable NDJSONから表示する。
- Difficulty: high — durable store resolutionとread-only CLI APIを追加する。
- Scope: `src/evidence.rs`のread-only resolver/render model、`src/cli.rs`のshow dispatch/output、CLI integration tests。
- Steps: EVD形状を検出した場合だけEvidence resolverへ分岐し、全NDJSONを決定的順序で検証してexact IDを一件解決する。v1全fieldとlinks、source file/lineを表示し、Entry `show` pathは変更しない。
- Acceptance: SQLiteにrecordがなくてもvalid NDJSON EVDを表示し、before/afterのrepository bytesとSQLite hashが不変である。
- Verification: mirror-only exact-ID CLI fixture、existing Entry show/fragment suite、fresh review_high。

## T-002

- Objective: Entry互換の使いやすさを保ちながらEvidence固有の曖昧性と不正入力を拒否する。
- Difficulty: high — identifier resolutionとfailure semanticsを後方互換で拡張する。
- Scope: Evidence ID/prefix resolution、CLI validation、error rendering、negative tests。
- Steps: canonical exact IDまたはunique prefixだけを許可し、slugとfragmentを拒否する。zero/ambiguous candidate、cross-file duplicate、invalid JSON/schema/kind/verdict/linkを区別して非zeroにする。
- Acceptance: 一意prefixだけが成功し、全negative fixtureは候補または原因を示してstate mutationなしで失敗する。
- Verification: table-driven unit tests、CLI exit-code assertions、fresh review_high。

## T-003

- Objective: NDJSONとSQLite Evidence indexをsupported commandで一致させ、rebuildの実処理をoperatorへ可視化する。
- Difficulty: high — sync/rebuild transactionと既存index保全を扱う。
- Scope: sync/rebuild orchestration、Evidence transaction、human-readable output、integration tests。
- Steps: managed Entry syncが成功したtransactional boundaryでvalid mirrorsをEvidence tablesへ反映する。validation/duplicate failureでは旧Evidence indexを保持する。rebuildはEntry件数とEvidence件数を別表示する。
- Acceptance: mirror-only recordが`belay sync`後に`verify status <target>`へ現れ、不正mirrorではsync/rebuildが失敗して既存正常indexが保持される。
- Verification: temporary-repository sync/rebuild/status tests、failure injection、full CLI suite。

## T-004

- Objective: 人間とagentの取得契約をCLI実装と一致させる。
- Difficulty: medium — help、docs、generated Skillの配布面を同期する。
- Scope: CLI after-help、README、`docs/id-reference-standard.md`、`src/agent.rs` generated Skill、installer/snapshot tests。
- Steps: `belay show <entry-or-evidence-id>`、EVDのprefix/非fragment契約、rebuild/sync index behaviorを記載する。Herdr reviewerはrunner provenanceを使い、`show EVD`をsettlement必須ゲートにしないことを明記する。Task settlementとGoal coverageを別判断として記載する。
- Acceptance: generated Codex/Claude Skillとdocsが同じ例・制約を持ち、既存Entry examplesも残る。
- Verification: help output tests、agent Skill golden/parity tests、docs lint、fresh review_medium。
