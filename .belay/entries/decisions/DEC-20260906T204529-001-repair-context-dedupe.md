---
schema_version: 1
id: DEC-20260906T204529-001-repair-context-dedupe
type: decision
title: repair-context-dedupe
status: proposed
created_at: 2026-09-06T20:45:29+09:00
updated_at: 2026-09-06T20:45:37+09:00
revision: 3
tags: []
links:
- relation: references
  id: PLN-20260906T134828-001-context-cost-first-phase#t-002
- relation: references
  id: REV-20260906T204514-001-context-dedupe-recovery
metadata: {}
---

Human authorization: the 2026-09-06 request to continue remaining approved Plan tasks authorizes the canonical one bounded repair within T-002 scope. Fact: F-001 is a product defect and F-002 is an acceptance verification gap; both are current-Task findings and independently actionable. Herdr lead disposition JSON: {"schema_version":1,"task":"PLN-20260906T134828-001-context-cost-first-phase#t-002","review":"REV-20260906T204514-001-context-dedupe-recovery","dispositions":[{"finding_id":"F-001","disposition":"Act on"},{"finding_id":"F-002","disposition":"Act on"}]}
