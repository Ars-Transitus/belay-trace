---
schema_version: 1
id: GOAL-20260801T223214-001-configure-claude-agent-m
type: goal
title: configure-claude-agent-models
status: draft
created_at: 2026-08-01T22:32:14+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Configure repository-scoped Claude Code roles equivalent to the Codex coordinator, bounded implementer, and independent reviewers.

## Success Criteria

- [SC-001] Shared project settings select Opus with low effort for the primary Claude Code session without modifying local or user-global settings.
- [SC-002] A project implementer uses Sonnet with low effort and is limited to explicitly approved, bounded Tier 2 work.
- [SC-003] A project reviewer uses Opus low and a high-risk reviewer uses Opus high; both are read-only and independent from implementation.
- [SC-004] Claude loads the repository workflow and human gates through CLAUDE.md while preserving existing RTK instructions.
- [SC-005] Claude Code validates the shared settings and discovers the three project agents.

## Constraints

- Preserve `.claude/settings.local.json` unchanged.
- Preserve existing RTK and belay-trace integration.
- Do not delegate Tier 3 implementation to the Sonnet implementer.
- Do not modify application runtime code.

## Non-goals

- User-global `~/.claude` configuration.
- Selecting or configuring OpenAI models in Claude Code.
- Agent teams or background multi-session orchestration.

## Verification

- Parse shared JSON and YAML frontmatter.
- Use Claude Code commands to validate project settings and agent discovery when supported.
- Run independent review and final Belay/Jujutsu checks.

## Risks

- Model aliases resolve to the latest available model and can change over time.
- Low effort is unsuitable for unresolved design or high-risk review, so a separate high-effort reviewer is required.
