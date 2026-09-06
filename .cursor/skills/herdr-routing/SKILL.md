---
name: herdr-routing
description: Launch routed implementation and review workers as separate CLI processes in Herdr panes instead of nested Cursor agents, with model pinned per worker, belay as the return channel, and up to three Task lanes. For independently warranted routed work, use by default when HERDR_ENV=1 unless the human explicitly opts out.
---

# herdr-routing

Herdr is a transport for an already approved routed Task. It does not select scope, profiles, permissions, or completion. The root orchestrator resolves the Task, chooses the configured role, and invokes one worker process.

Implementation runs in a dedicated Git worktree. Review runs in a fresh read-only context over the validated Work diff. Pass compact pointers: Task, role, checkout or Work locator, exact changed paths, validation provenance, and only directly required dependency context. Results return through linked Belay Work, Review, and Evidence records.

Do not poll panes or conduct conversational repair. After a failed review the root may authorize one fresh fixer for selected current-Task findings and exact paths, then one fresh re-review. Preserve task/result/checkout binding and configured round budgets.

Herdr does not authorize push, publication, deployment, protected-branch changes, or any other external action. Runtime schemas and enforcement remain in `agent-safety/routing.json` and `scripts/route_worker.py`.
