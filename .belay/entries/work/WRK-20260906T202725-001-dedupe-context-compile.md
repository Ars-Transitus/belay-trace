---
schema_version: 1
id: WRK-20260906T202725-001-dedupe-context-compile
type: work
title: dedupe-context-compile
status: completed
created_at: 2026-09-06T20:27:25+09:00
updated_at: 2026-09-06T20:31:31+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260906T134828-001-context-cost-first-phase#t-002
- relation: fulfills
  id: GOAL-20260906T134724-001-context-cost-reduction#sc-002
metadata: {}
---

Fact: implement canonical-entry deduplication and whole-output budget allocation in src/context.rs with focused CLI regressions in tests/cli.rs. Verification will cover task compile, working-set compile, empty, duplicate, low-budget, and the committed context-cost fixture.
Herdr return metadata JSON: {"base_commit":"95e5323b31cd2f83844ab503ba6b1887a8aae7f6","checkout_locator":"/private/var/folders/lp/cf8008053v12_7knfvy17jy40000gn/T/erwin-routed/5e4bce33c2d04f64-fd05a7993599","working_copy_diff_sha256":"ccd1890dbe972e2d8d2bbe7710ca46172828453cb4958fccceb3c3b3a57255e8"}
