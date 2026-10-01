---
schema_version: 1
id: GOAL-20260930T220628-001-inventory-lifecycle-rele
type: goal
title: inventory-lifecycle-release
status: completed
created_at: 2026-09-30T22:06:28+09:00
updated_at: 2026-10-01T06:22:36+09:00
revision: 2
tags: []
links: []
metadata: {}
---

## Summary

Deliver the approved inventory/context/lifecycle Plan locally as Belay 0.7.0, preserving source data and traceability.

## Success Criteria

- [SC-001] Reproducible baseline, inventory findings and reversible selected-operation previews preserve references and distinguish facts from semantic proposals.
- [SC-002] Shared required/optional renderer and context retrieval preserve boundaries and sources; fixed evaluation meets approved quality and efficiency thresholds.
- [SC-003] Concurrent Evidence, provenance summaries, validated packs, safe compaction/recovery and reconstruction preserve all originals and verification semantics.
- [SC-004] Local deploy, documentation, independent review, acceptance checks and release preparation agree with BI-07/LC06 adoption judgments.

## Constraints

- Native permissions; no publication, push, protected branch merge, user-local deploy, Erwin source changes or installed consumer edits.
- Preserve user harness modifications. Real data reorganization requires selection of concrete previews; isolated copies and fixtures are authorized.
- Approved by user 2026-09-30: total retrieval tokens -20%, searches not increased, median runtime regression <=10%, additional maintenance <=1 operation. Required reference loss, false completion/validity, and history loss must be zero.

## Non-goals

Omnia 0.8.0, embedding/GraphRAG, core LLM, complete original deletion, Git history rewrite and external log preservation.

## Verification

Frozen evaluation, lifecycle failure matrix, relevant Rust/Python checks, independent fresh review and local deployment version check. Never count unverified work as complete.

## Risks

Pack reduces file count, not necessarily bytes. Old in-flight writers require quiescent migration. Evaluation is fixed-case evidence, not universal proof.
