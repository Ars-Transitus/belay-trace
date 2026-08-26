---
schema_version: 1
id: WRK-20260729T215937-001-implement-route-mvp
type: work
title: implement-route-mvp
status: completed
created_at: 2026-07-29T21:59:37+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Scope

- Implement `PLN-20260726T104808-001#t-001` through `#t-008` for the approved CLI-first Route MVP.
- Keep semantic reasoning external and deterministic Route processing in Belay core.
- Store non-authoritative run bundles under `.belay/state/route/<run-id>/`.

## Progress

- T-001..T-008: verified — protocol, CLI lifecycle, local resume, authority binding, preview/apply, crash-safe reconciliation, docs, Evidence, and independent review passed.

## Validation

- Rust 1.87 formatting and `clippy --all-targets --locked -- -D warnings` passed.
- `cargo test --all-targets --locked`: 135 tests passed.
- Focused Route tests cover exact preview approval, stale rejection, create/link materialization, revision guards, fragment validation, durable operation receipt, and idempotent retry.
- EVD-20260730T061625-001 records fresh validation against SC-001..SC-007.
- Independent review REV-20260729T230114-001 approved the final code and focused recovery suite with no blocking/high findings.

## Observations

- Human approved the revised Goal, Plan, local persistence boundary, and implementation start on 2026-07-29.
- Existing store mutations combine SQLite and managed Markdown per entry, so Route uses operation-level recovery rather than a cross-entry batch transaction.
- Review found a crash window between mutation and Reconciliation. Schema v4 now records a unique receipt in the same transaction as each mutation, after the manifest binds the run to `applying`.
- MVP Evidence materialization was narrowed to read-only Goal Coverage; creating Evidence remains the existing append-only `belay verify` workflow.
- Working-copy `belay init` refreshed generated Skills. Updating the active `.agents` Skill is unavailable in this execution environment; canonical source and generated artifacts are updated.

## Assumptions

- Confirmed for MVP: a manifest plus immutable revisioned artifact files supports validation, pause/resume, and recovery without a new persisted Belay model.

## Blockers

- None identified.

## Pull Request Delivery

- Draft PR [#21](https://github.com/Ars-Transitus/belay-trace/pull/21) targets `main` from `agent/route-mvp`.
- PR creation was explicitly approved by the project owner on 2026-07-30.
- Merge is not approved.
