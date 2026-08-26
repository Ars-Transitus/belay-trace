---
schema_version: 1
id: GOAL-20260801T221649-001-configure-project-agent
type: goal
title: configure-project-agent-models
status: draft
created_at: 2026-08-01T22:16:49+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Configure repository-scoped Codex roles so Sol coordinates and reviews while Luna handles approved, bounded implementation.

## Success Criteria

- [SC-001] Project configuration selects `gpt-5.6-sol` with low reasoning for the primary agent and registers the repository custom-agent roles.
- [SC-002] A project-scoped implementer uses `gpt-5.6-luna` with low reasoning and is limited to approved, bounded Tier 2 implementation.
- [SC-003] A project-scoped reviewer uses `gpt-5.6-sol` with risk-proportionate reasoning and remains read-only and independent from implementation.
- [SC-004] AGENTS.md defines when delegation is required, prohibited, or escalated without weakening human gates or review independence.
- [SC-005] Codex can parse and expose the project configuration and custom agents, or any runtime limitation is recorded as Unknown.

## Constraints

- Do not modify `~/.codex` or other user-global configuration.
- Preserve the existing human-gated workflow and risk tiers.
- Do not delegate Tier 3 implementation to the Luna implementer.
- Keep the managed belay-trace block as the final section; `@RTK.md` appears before it as supplied by the project.

## Non-goals

- Changing runtime source code or CI.
- Configuring other repositories.
- Guaranteeing model entitlement or availability outside the current environment.

## Verification

- Parse project TOML with Codex configuration commands.
- Inspect resolved model and agent settings when the CLI exposes them.
- Run focused file and policy consistency checks.

## Risks

- Current public documentation lists Luna, but the active subagent tool may expose a narrower model allowlist.
- Low reasoning can be insufficient for broad or security-sensitive review; escalation rules must override the low default.
