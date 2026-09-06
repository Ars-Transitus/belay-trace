---
schema_version: 1
id: WRK-20260901T093640-001-ndjson-evidence-show
type: work
title: ndjson evidence show
status: completed
created_at: 2026-09-01T09:36:40+09:00
updated_at: 2026-09-01T09:51:01+09:00
revision: 2
tags: []
links:
- relation: implements
  id: PLN-20260901T091831-001-implement-evidence-show#t-001
- relation: fulfills
  id: GOAL-20260901T091831-001-direct-evidence-retrieva#sc-001
metadata: {}
---

Implement a read-only Evidence resolver that reads .belay/evidence/*.ndjson in deterministic order. Dispatch belay show to that resolver only for EVD-shaped queries. Render v1 fields, links, and source location without mutating SQLite or mirrors.
