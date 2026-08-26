---
schema_version: 1
id: REV-20260725T213412-001-review-explore-spacing-revert
type: review
title: Review Explore spacing revert
status: completed
created_at: 2026-07-25T21:34:12+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: reviews
  id: WRK-20260725T213248-001-restore-prior-explore-graph-spacing
metadata: {}
---

Fresh-context focused review. No blocking finding. The code diff restores only ideal edge length, node repulsion, and component spacing to 64, 4096, and 80. Label-aware dimensions, activation arbitration, Explore API, accessibility, read-only behavior, and persistence remain unchanged. Unknown: visual appearance was not observed in a browser.
