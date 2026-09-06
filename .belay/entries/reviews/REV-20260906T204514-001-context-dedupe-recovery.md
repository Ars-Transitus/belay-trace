---
schema_version: 1
id: REV-20260906T204514-001-context-dedupe-recovery
type: review
title: context-dedupe-recovery-review
status: completed
created_at: 2026-09-06T20:45:14+09:00
updated_at: 2026-09-06T20:45:21+09:00
revision: 4
tags: []
links:
- relation: references
  id: PLN-20260906T134828-001-context-cost-first-phase#t-002
- relation: reviews
  id: WRK-20260906T202725-001-dedupe-context-compile
metadata: {}
---

Fresh read-only recovery review of WRK-20260906T202725-001-dedupe-context-compile at validated checkout /private/var/folders/lp/cf8008053v12_7knfvy17jy40000gn/T/erwin-routed/5e4bce33c2d04f64-fd05a7993599. Herdr review finding JSON: {"finding_id":"F-001","category":"product-defect","severity":"high","target_paths":["src/context.rs","tests/cli.rs"],"summary":"Working-set compile omits Goal Unknowns because selected Goal rendering excludes Unknowns and canonical deduplication removes the ranked fallback; the added test uses an OR with always-present Next and misses the loss."} Herdr review finding JSON: {"finding_id":"F-002","category":"acceptance-verification-gap","severity":"high","target_paths":["tests/cli.rs"],"summary":"No regression or Evidence compares duplicate-fixture estimated tokens against the pinned T-001 baseline, so the token-reduction clause is unverified."} Herdr review outcome JSON: {"outcome":"fail"}
