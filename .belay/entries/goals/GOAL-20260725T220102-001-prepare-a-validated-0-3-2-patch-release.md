---
schema_version: 1
id: GOAL-20260725T220102-001-prepare-a-validated-0-3-2-patch-release
type: goal
title: Prepare a validated 0.3.2 patch release
status: completed
created_at: 2026-07-25T22:01:02+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

Prepare version 0.3.2 for the backward-compatible Browse activation and readability fixes accumulated after 0.3.1.

## Success Criteria

- [SC-001] Cargo package metadata and lockfile consistently report version 0.3.2.
- [SC-002] The 0.3.2 scope is documented as backward-compatible Browse bug fixes and visual refinement.
- [SC-003] Rust 1.87 fmt, clippy, all-target tests, rebuild, doctor, and Playwright E2E pass against the release candidate.
- [SC-004] Tagging, GitHub Release creation, publishing, PR creation, and merge occur only after separate explicit human approval.

## Constraints

- Preserve existing CLI, storage schema, managed Markdown, Explore API, and read-only Browse contracts.
- Draft PR creation and squash merge of PR #16 are approved after review; do not create a tag, GitHub Release, or publish artifact.
- Do not claim release readiness while Playwright interaction tests are unverified.

## Non-goals

- Introducing a release automation workflow.
- Publishing to crates.io or another package registry.
- Adding unrelated product changes.

## Verification

- Confirm Cargo.toml and the belay-trace package entry in Cargo.lock both use 0.3.2.
- Run Rust 1.87 fmt, clippy, all-target tests, rebuild, and doctor.
- Run Playwright E2E in CI or another browser-capable environment.
- Review the final diff and Evidence before any release action.

## Risks

- The repository currently has no remote tags or GitHub Releases, so the intended public release mechanism is Unknown.
- Local Playwright execution is unavailable in the current environment.
- Existing active Goal and AGENTS integration drift can keep doctor non-passing even if the version fields are correct.

## Completion

- Cargo.toml、Cargo.lock、rebuild済みbinaryは0.3.2で一致し、dependency churnはない。
- PR #16 CIでRustとPlaywrightを含む全required checksが成功し、EVD-20260725T134154-002がSC-003を検証する。
- PR #16は個別のhuman approval後にsquash mergeされ、EVD-20260726T111216-001がSC-004の承認境界を記録する。
- tag、GitHub Release、registry publicationは実行しておらず、引き続き別の明示承認を必要とする。
- SC-001からSC-004はすべてverifiedであり、0.3.2 release candidate準備Goalは完了した。
