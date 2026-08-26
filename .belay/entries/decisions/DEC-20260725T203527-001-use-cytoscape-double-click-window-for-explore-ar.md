---
schema_version: 1
id: DEC-20260725T203527-001-use-cytoscape-double-click-window-for-explore-ar
type: decision
title: Use Cytoscape double-click window for Explore arbitration
status: accepted
created_at: 2026-07-25T20:35:27+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260725T194008-001-make-explore-node-activation-reliable-and-unambi
- relation: supports
  id: PLN-20260725T194011-001-resolve-explore-single-and-double-click-contenti
metadata: {}
---

Decision: Defer each node tap by cy.multiClickDebounceTime(), keep at most one pending timer per node, cancel the matching timer on dbltap, and clear all timers on pagehide. Rationale: using Cytoscape's configured recognition window avoids a duplicated magic number and keeps single/double activation mutually exclusive without changing the Explore API. Alternatives rejected: immediate expansion preserves the race; a longer fixed delay degrades responsiveness; a global timer incorrectly couples independent nodes. Consequence: single-click expansion is intentionally delayed by Cytoscape's recognition window (250ms with the vendored 3.34.0 configuration).
