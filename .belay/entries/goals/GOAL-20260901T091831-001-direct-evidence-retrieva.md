---
schema_version: 1
id: GOAL-20260901T091831-001-direct-evidence-retrieva
type: goal
title: direct-evidence-retrieval
status: active
created_at: 2026-09-01T09:18:31+09:00
updated_at: 2026-09-01T09:36:16+09:00
revision: 3
tags: []
links: []
metadata: {}
---

## Summary

- Evidence IDを通常のoperator/agent retrieval経路から直接・一意・read-onlyに解決でき、NDJSON durable recordとSQLite indexの状態差をEvidence消失と誤認しない。

## Success Criteria

- [SC-001] validなexact `EVD-...`に対する`belay show`が、SQLiteに未indexでも`.belay/evidence/*.ndjson`から一意に解決し、schema、ID、kind、verdict、commit、captured_at、issuer、source、summary、detail、links、source locationを安定した形式で表示する。
- [SC-002] unique EVD prefixは一件だけ解決し、zero match、ambiguous prefix、duplicate ID、malformed/unsupported schemaは状態を変更せず明確な非zeroでfail-closedになる。Evidence fragment、slug、Entry mutation semanticsは受け付けない。
- [SC-003] `belay sync`がmirror-only EvidenceをSQLite indexへtransactionally反映し、`belay rebuild`の出力がmanaged Markdown件数とEvidence件数を別々に報告する。`belay verify status <target>`は同期後に同じrecord集合を返す。
- [SC-004] CLI help、README、ID reference、生成されるbelay-trace SkillがEntryとEvidenceの取得契約、Herdrのrunner-owned provenance、Task settlementとGoal coverageの違いを矛盾なく説明する。

## Constraints

- NDJSON Evidenceはappend-onlyとし、`show`や`sync`で書換え・deduplicate・managed Markdown化しない。
- Existing Entry `show`、fragment、unique prefix/slug behaviorとexit statusを後方互換に保つ。
- EvidenceをEntry statusや通常の`belay link` mutation対象へ拡張しない。
- sync/rebuildは不正・重複Evidenceを黙って無視せず、既存の正常indexを破損させない。

## Non-goals

- Herdr runnerをBelay `show`出力のparserへ変更しない。
- Evidence schema、freshness判定、coverage算出、Evidence mutation APIを再設計しない。
- erwinやconsumer repositoryをこのGoalから変更・installしない。

## Verification

- `belay goal lint`と`belay plan lint`が通り、各Taskのfocus compileが対応する一つのSCを解決する。
- Evidence resolver、CLI show、sync/rebuild transactionのunit/integration testsと既存CLI full suiteを実行する。
- Help、README、ID reference、generated Codex/Claude Skillのparityとfresh risk-matched reviewを確認する。

## Risks

- Evidence `show`をEntry mutation semanticsへ誤って広げると型境界とappend-only性を壊す。
- sync中のvalidation failureでEvidence tablesを先に消すと既存の正常indexを失うため、transactional replacementが必要である。
- `show EVD`をHerdr settlementの必須ゲートにすると同じ二重authorityを再導入する。

## Assumptions / Unknowns

- Fact: Belay 0.6.1の`show`はEntry storeだけを解決し、Evidence rebuildはNDJSONをSQLiteへindexするが出力件数にEvidenceを表示しない。
- Hypothesis: `show EVD`をNDJSONから直接解決すれば、record直後・Herdr harvest直後・SQLite rebuild前のすべてで同じdurable recordを確認できる。
- Unknown: 将来Evidence schema versionが増えた場合の複数version renderer。v1以外はこのGoalではfail-closedとする。
