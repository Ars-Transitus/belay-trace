---
schema_version: 1
id: GOAL-20260801T212650-001-risk-based-agent-workflo
type: goal
title: risk-based-agent-workflow
status: draft
created_at: 2026-08-01T21:26:50+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Make the shared agent workflow proportionate to operational risk while preserving strict assurance for high-risk changes.

## Success Criteria

- [SC-001] The canonical AGENTS.md classifies work by impact, uncertainty, and reversibility with objective Tier 1, Tier 2, and Tier 3 boundaries.
- [SC-002] Trace and independent-review requirements scale by tier, with no separate review by default for Tier 1 and full assurance retained for Tier 3.
- [SC-003] The belay-managed block remains the final section of AGENTS.md and does not duplicate policy owned by the repository.
- [SC-004] belay-trace-template and lectio receive the optimized canonical file without preserving their obsolete AGENTS.md content.

## Constraints

- Preserve explicit human gates for implementation, issue creation, pull request creation, and merge.
- Preserve repository-specific release versioning rules only in the belay-trace repository.
- Do not overwrite unrelated working-copy changes.

## Non-goals

- Changing source code, runtime behavior, or Belay CLI contracts.
- Migrating historical trace or work-log data.
- Relaxing Tier 3 review or approval requirements.

## Verification

- Compare all three resulting files and confirm the managed block is last.
- Validate the canonical Markdown structure and inspect fresh diffs in each repository.
- Obtain one independent focused review of the policy change.

## Risks

- A full-file copy can accidentally propagate belay-trace-specific release rules to unrelated repositories; separate canonical shared policy from project-local additions before copying.
- Conditional Tier 2 review must not conflict with the generic skill completion language.
