---
schema_version: 1
id: WRK-20260925T062148-001-streamline-skill-guidanc
type: work
title: streamline-skill-guidance
status: completed
created_at: 2026-09-25T06:21:48+09:00
updated_at: 2026-09-25T21:33:05+09:00
revision: 3
tags: []
links: []
metadata: {}
---

Fact: User requested a GPT-6 Astra era rule audit and optimization. Localized reversible source-text changes in src/agent.rs shorten discovery, allow proportional reconciliation, and bound redundant validation. Existing permission, review, acceptance, ID and Evidence contracts remain. Audit: docs/design/agent-rules-audit.md. Installed consumer configuration is unchanged; broader Erwin rollout remains separate. Validation: cargo test --test cli init --locked passed 6 tests; cargo fmt --all --check and git diff --check passed. Independent fresh review passed after a bounded fix restored explicit in-progress reporting. Final source passed the same six CLI tests and formatting/diff checks. Standalone skill validator could not run due to missing PyYAML and restricted uv cache. Source changes are verified; installed-policy rollout remains unapplied. Performance impact is unmeasured.
