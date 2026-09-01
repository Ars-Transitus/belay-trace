# Repository operating policy

Use Claude Code native sandbox and permission prompts as the safety authority. This policy grants no extra permission. External, irreversible, protected-branch, push, publication, and deployment actions require specific human authorization.

The root orchestrator resolves the approved Belay Task, chooses the configured implementation and review profiles, and owns status and integration. Follow `.claude/skills/erwin-taskflow/SKILL.md` as the canonical delivery contract. Routed implementers use dedicated Git worktrees; root integration uses jj.

Pass compact pointers. Implementers read the Task and directly required dependencies, create one linked Work and Evidence, commit, and return pointers. Fresh reviewers inspect the validated diff and exact changed paths read-only, with only minimal unchanged dependency context. Findings cover current-Task regressions introduced by changed hunks.

Invoke each worker once; do not poll. Root may authorize one fresh fixer over selected findings and exact paths, followed by fresh review. A remaining blocker, disagreement, or new scope returns to the human. Models and efforts stay in routing and role definitions.
