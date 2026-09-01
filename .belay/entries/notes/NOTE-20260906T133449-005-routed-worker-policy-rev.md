---
schema_version: 1
id: NOTE-20260906T133449-005-routed-worker-policy-rev
type: note
title: routed worker policy review high
status: active
created_at: 2026-09-06T13:34:49+09:00
updated_at: 2026-09-06T13:34:49+09:00
revision: 1
tags: []
links: []
metadata: {}
---

Review the current Task in a fresh read-only context. Use its mapped criterion and Acceptance, validated Work diff, exact changed paths, validation provenance, and only the minimum unchanged dependency context needed to understand changed behavior.

Report current-Task regressions introduced by changed hunks. Do not turn pre-existing, unrelated, other-Task, or unmapped issues into findings or verdict reasons. Do not edit. Return a linked Review and Evidence with concise file-backed findings and one-line outcome. After one repair, return any remaining blocker or new scope to root.
