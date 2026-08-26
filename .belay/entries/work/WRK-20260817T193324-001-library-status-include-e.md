---
schema_version: 1
id: WRK-20260817T193324-001-library-status-include-e
type: work
title: library-status-include-exclude
status: completed
created_at: 2026-08-17T19:33:24+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: implements
  id: PLN-20260817T190256-001-library-status-include-e#t-001
- relation: implements
  id: PLN-20260817T190256-001-library-status-include-e#t-002
- relation: implements
  id: PLN-20260817T190256-001-library-status-include-e#t-003
- relation: fulfills
  id: GOAL-20260817T185935-001-library-status-include-e
metadata: {}
---

- Shared `SearchRequest` now has `status_include` and `status_exclude`.
- SQL applies `IN` / `NOT IN` before LIMIT for exact, structured, and keyword search.
- Overlapping include/exclude is a validation error.
- CLI: repeatable `--status` and `--exclude-status`. Single `--status` still works.
- Library: Include/Exclude radios and a checklist of all thirteen statuses.
- Legacy `?status=accepted` remains include-one.
- Tests: search unit, Browse form/exclude, CLI include/exclude/overlap.
