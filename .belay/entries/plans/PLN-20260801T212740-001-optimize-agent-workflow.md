---
schema_version: 1
id: PLN-20260801T212740-001-optimize-agent-workflow
type: plan
title: optimize-agent-workflow
status: approved
created_at: 2026-08-01T21:27:40+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260801T212650-001-risk-based-agent-workflo
metadata: {}
---

## Tier Classification

- Tier 2: agent workflow policy changes across three repositories are meaningful but reversible and do not alter runtime security, persistent data, or public APIs.

## Problem

- The current canonical candidate applies trace and independent review too broadly, and older copies duplicate or conflict with the installed belay-trace skill.

## Desired Outcome

- A concise risk-based AGENTS.md provides clear tier boundaries and proportionate review requirements, then replaces obsolete copies in belay-trace-template and lectio.

## Success Signals

- Tier 1, Tier 2, and Tier 3 are distinguished by impact, uncertainty, and reversibility.
- Tier 1 has no Goal, Plan, Delivery Map, Work, or independent review requirement by default.
- Tier 2 uses concise trace and conditional independent review.
- Tier 3 retains the full belay-trace workflow and independent review.
- The managed belay-trace block is the final section in every copied file.

## Constraints

- Preserve all explicit human gates.
- Preserve unrelated working-copy changes.
- Keep repository-specific additions outside the shared canonical content.
- Use jj for version-control operations.

## Non-goals

- Change the belay CLI or installed skill implementation.
- Migrate old work-log files.
- Create issues, pull requests, or merges.

## Assumptions

- The human instruction explicitly authorizes implementation and complete replacement of lectio and belay-trace-template AGENTS.md.
- The belay-trace repository file is the latest policy baseline.

## Unknowns / Decisions Needed

- Unknown: whether other repositories beyond the named template and lectio should be updated in this implementation. They are excluded unless separately requested.

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
|---|---|---|---|---|---|
| T-001 | SC-001 | Define objective risk-based tier boundaries in the canonical file | Codex | verified | Focused text inspection and independent review passed after remediation |
| T-002 | SC-002 | Scale trace and review requirements by tier | Codex | verified | Independent review findings addressed |
| T-003 | SC-003 | Keep the managed block last and avoid duplicated ownership | Codex | verified | One marker pair at lines 223-233; end marker is final line |
| T-004 | SC-004 | Replace template and lectio copies | Codex | verified | All three SHA-256 hashes are identical |
| T-005 | SC-001, SC-002, SC-003, SC-004 | Run final validation and record Evidence | Codex | verified | doctor, coverage, jj inspection, file identity, and marker checks passed |
