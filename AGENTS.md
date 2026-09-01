# Repository operating policy

Use the product native sandbox, permission system, and approval UI as the safety authority. This policy grants no extra permission. External, irreversible, protected-branch, push, publication, and deployment actions require the human authorization that applies to that exact action.

For non-trivial work, the root orchestrator resolves the approved Belay Task, chooses the implementation and review profiles from `.agent-safety/routing.json`, and owns status and integration. Trivial root work need not be delegated. Routed implementers use one dedicated Git worktree and Git; root integration uses jj.

Follow `.agents/skills/erwin-taskflow/SKILL.md` as the canonical delivery contract. Hand off compact pointers. An implementer reads the Task fragment and directly required dependencies, creates one linked Work, records Evidence, commits, and returns pointers. A fresh reviewer inspects the validated diff and exact changed paths, reading only minimal unchanged dependency context. Findings are limited to current-Task regressions introduced by changed hunks.

The parent invokes each worker once and does not poll. Root may authorize one fresh fixer over selected findings and exact paths, followed by fresh review. Any remaining blocker, material disagreement, or new scope returns to the human. Models and reasoning efforts remain defined by routing and role files.

Installed copies are consumer configuration. Change source under `agent-config/`, `agent-safety/`, and `skills/`; installation is a separate human-controlled step.

<!-- belay-trace:start -->
## belay-trace

For Tier 2 and Tier 3 work, follow the repository-installed
`.agents/skills/belay-trace/SKILL.md`. The Skill owns context retrieval, Intent Briefs,
Delivery Maps, reconciliation, Evidence, and conflict-safe trace updates.

If the Skill is unavailable, run
`belay context compile "<task>" --format agent --budget 4000` before broad
history reads and preserve all repository-specific human approval gates.
<!-- belay-trace:end -->
