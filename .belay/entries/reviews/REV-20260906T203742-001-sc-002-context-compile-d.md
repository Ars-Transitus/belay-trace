---
schema_version: 1
id: REV-20260906T203742-001-sc-002-context-compile-d
type: review
title: SC-002 context compile dedupe review
status: pending
created_at: 2026-09-06T20:37:42+09:00
updated_at: 2026-09-06T20:38:04+09:00
revision: 2
tags: []
links:
- relation: reviews
  id: PLN-20260906T134828-001-context-cost-first-phase#t-002
metadata: {}
---

Policy: NOTE-20260906T133449-001-routed-worker-policy-com rev.1; NOTE-20260906T133449-005-routed-worker-policy-rev rev.1. F-001 evidence: runner snapshot expected working_copy_diff_sha256 ccd1890dbe972e2d8d2bbe7710ca46172828453cb4958fccceb3c3b3a57255e8, but git diff of the allowlisted paths from base 95e5323b31cd2f83844ab503ba6b1887a8aae7f6 resolved to bf2102beee65ff716e59061e9a61021e0583a388297969eb1762efb1bf1ed541; the validated diff identity is therefore not established. Herdr review finding JSON: {"id":"F-001","category":"acceptance-verification-gap","target_paths":["src/context.rs","tests/cli.rs"],"summary":"Validated snapshot hash does not match the actual allowlisted diff from the supplied base commit, so SC-002 cannot be verified against the claimed snapshot."} F-002 evidence: tests/cli.rs checks one canonical constraint occurrence and low-budget rejection, but contains no baseline estimated-token measurement or assertion that the duplicate fixture is smaller than baseline. The focused context tests (10), the dedicated low-budget test, cargo fmt check, and git diff --check passed; none supplies the required baseline comparison. Herdr review finding JSON: {"id":"F-002","category":"acceptance-verification-gap","target_paths":["tests/cli.rs"],"summary":"The required duplicate-fixture estimated-token comparison against baseline is absent, leaving the token-reduction clause of SC-002 unverified."} Herdr review outcome JSON: {"outcome":"fail"}
