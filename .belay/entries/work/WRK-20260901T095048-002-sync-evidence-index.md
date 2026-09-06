---
schema_version: 1
id: WRK-20260901T095048-002-sync-evidence-index
type: work
title: sync evidence index
status: completed
created_at: 2026-09-01T09:50:48+09:00
updated_at: 2026-09-01T09:51:02+09:00
revision: 2
tags: []
links:
- relation: implements
  id: PLN-20260901T091831-001-implement-evidence-show#t-003
- relation: fulfills
  id: GOAL-20260901T091831-001-direct-evidence-retrieva#sc-003
metadata: {}
---

sync indexes mirror-only Evidence transactionally; invalid mirrors keep the previous index; rebuild reports entry and Evidence counts separately.
