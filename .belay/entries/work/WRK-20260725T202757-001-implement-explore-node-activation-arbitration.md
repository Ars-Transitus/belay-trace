---
schema_version: 1
id: WRK-20260725T202757-001-implement-explore-node-activation-arbitration
type: work
title: Implement Explore node activation arbitration
status: completed
created_at: 2026-07-25T20:27:57+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: DEC-20260725T203527-001-use-cytoscape-double-click-window-for-explore-ar
- relation: implements
  id: PLN-20260725T194011-001-resolve-explore-single-and-double-click-contenti
- relation: fulfills
  id: GOAL-20260725T194008-001-make-explore-node-activation-reliable-and-unambi
metadata: {}
---

## Scope

Implement PLN-20260725T194011-001 Delivery Map tasks T-001 through T-005 and reconcile the T-006 human acceptance gate after browser-capable CI verification.

## Progress

- T-001: verified
- T-002: verified
- T-003: verified
- T-004: verified
- T-005: verified
- T-006: verified

## Assumptions and Unknowns

- Confirmed from vendored Cytoscape.js 3.34.0: `multiClickDebounceTime()` supplies the configured double-click window (250ms by default), so the single-click timer uses that value rather than duplicating a magic number.
- Confirmed in PR #16 Playwright CI for the covered browser: deferring expansion for that window and cancelling the matching node timer on `dbltap` resolves the reported single/double activation contention. Cross-browser and device-wide generalization remains Unknown.

## Validation

- Passed: `node --check src/browse.js`
- Passed: `node --check tests/e2e/browse.spec.js`
- Passed: `cargo fmt --all -- --check`
- Passed: Rust 1.87.0 `cargo clippy --all-targets --locked -- -D warnings`
- Passed: Rust 1.87.0 `cargo test --all-targets --locked` (123 tests)
- Blocked by execution environment: Playwright Chromium and local Chrome both abort before test startup because macOS denies Mach port rendezvous. No interaction assertion ran.
- Browser plugin unavailable: the requested in-app Browser returned `Browser is not available: iab`.
- Passed: fresh-context focused review after resolving two findings (initial graph readiness race and duplicate in-flight expansion); no blocking implementation finding remains.
- Passed in PR #16 CI: Rust and Playwright, including the real-node single-click, double-click, and Accessible Goal fallback assertions; EVD-20260725T134154-001, EVD-20260725T134154-003, EVD-20260725T134154-004, and EVD-20260725T134154-005.
- Passed human acceptance: the corrected Explore activation behavior was accepted for inclusion in 0.3.2; EVD-20260725T220721-001.
- PR #16 was squash merged after separate approval and Issue #15 was closed.

## Review

- REV-20260725T204017-001-review-explore-node-activation-arbitration
- The review-time real-browser gap was resolved by PR #16 Playwright CI and human acceptance.
