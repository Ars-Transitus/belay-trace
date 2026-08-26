---
schema_version: 1
id: WRK-20260725T212012-001-improve-explore-graph-spacing
type: work
title: Improve Explore graph spacing
status: completed
created_at: 2026-07-25T21:20:12+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: PLN-20260722T225244-001-add-a-local-trace-provenance-browser
- relation: follows-up
  id: WRK-20260723T192431-001-refine-browse-readability-and-trace-navigation
metadata: {}
---

## Scope

Increase Cytoscape cose spacing and include rendered labels in node dimensions so Explore remains readable without manual repositioning.

## Classification

Tier 1: localized, reversible layout configuration change. No API, graph data, interaction contract, or persistence change.

## Validation

- Passed: `node --check src/browse.js`
- Passed: `cargo fmt --all -- --check`
- Passed: Rust 1.87.0 `cargo clippy --all-targets --locked -- -D warnings`
- Passed: Rust 1.87.0 `cargo test --all-targets --locked` (123 tests)
- Passed: fresh-context focused review; no blocking implementation finding.
- Unknown: visual result in a real browser; the in-app Browser and local Playwright were unavailable in the preceding Explore task.

## Implementation

- Include rendered labels in cose node dimensions.
- Increase ideal edge length from 32 to 64.
- Increase node repulsion from 2048 to 4096.
- Increase disconnected component spacing from 40 to 80.
- Reuse one layout factory for initial render and post-expansion relayout.

## Review

- REV-20260725T212313-001-review-explore-graph-spacing
- Human visual inspection is recommended because viewport fit and maximum-size readability were not observable in this environment.
