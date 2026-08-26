---
schema_version: 1
id: WRK-20260725T220500-001-prepare-version-0-3-2-release-candidate
type: work
title: Prepare version 0.3.2 release candidate
status: completed
created_at: 2026-07-25T22:05:00+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260725T220123-001-prepare-version-0-3-2-without-publishing
- relation: fulfills
  id: GOAL-20260725T220102-001-prepare-a-validated-0-3-2-patch-release
metadata: {}
---

Implement approved PLN-20260725T220123-001 tasks T-001 through T-004 and prepare the authorized draft PR. Within this release-preparation change, update only package metadata; the PR also contains the previously approved Browse interaction fix, user-tuned spacing, AGENTS integration, and trace records accumulated after 0.3.1. Reconcile release scope, run Rust 1.87 and Playwright verification, perform fresh-context review, and stop before merge, tag, GitHub Release, or publication.

## Progress

- T-001: verified
- T-002: verified
- T-003: verified
- T-004: verified
- T-005: verified for PR creation and squash merge gates; tag, GitHub Release, and publication remain prohibited

## Validation

- Passed: Cargo.toml and the belay-trace Cargo.lock package entry are 0.3.2 with no dependency churn.
- Passed: the rebuilt binary reports belay 0.3.2.
- Passed: Rust 1.87 fmt, clippy, and all 123 tests.
- Passed: rebuild and doctor after refreshing the marker-managed AGENTS section.
- Local limitation: Playwright Chromium aborts before assertions because macOS denies Mach port rendezvous.
- Passed in PR CI: Rust, Playwright, markdownlint, typos, links, and PR-title validation; 9 checks passed with no failures.
- PR CI follow-up resolved: docs-ci identified the valid Cytoscape `cose`/`COSE` name as a typo and a bare Issue URL in the managed Plan. The typo allowlist and Markdown link were corrected and the rerun passed.
- Merge gate: Copilot reviewed 25 of 26 changed files and generated no comments; GraphQL confirmed zero review threads; all 9 checks passed; the human approved squash merge of PR #16 and closure of Issue #15.
