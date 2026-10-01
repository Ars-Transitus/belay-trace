---
schema_version: 1
id: WRK-20261001T213201-001-contract-work-link
type: work
title: contract-work-link
status: completed
created_at: 2026-10-01T21:32:01+09:00
updated_at: 2026-10-01T21:42:47+09:00
revision: 2
tags: []
links:
- relation: implements
  id: PLN-20261001T062923-001-omnia-readiness-delivery#t-021
- relation: fulfills
  id: GOAL-20261001T062823-001-omnia-integration-readin#sc-013
metadata: {}
---

Dogfood revealed Work create only accepts fulfills Plan Goal link, while Contract and context use implements. Fix resolver compatibility without changing immutable projection/receipt; validate actual CLI Work creation.
