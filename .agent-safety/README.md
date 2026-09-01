# Shared runtime source

This directory is copied into consumer projects as `.agent-safety/`. The product native sandbox, permission system, and approval UI enforce safety. Files retained here support installed compatibility and focused fixtures; they do not create independent permission or approval authority.

`routing.json` is the machine-readable source for implementation and review profiles, model and effort selection, round budgets, and transport contracts. `herdr-routing/scripts/route_worker.py` enforces runtime binding and settlement. Their schemas are unchanged by the prose-only taskflow reduction.

The active workflow is intentionally narrow: one approved Belay Task, a dedicated implementation worktree, compact Work and Evidence returns, fresh read-only review, and at most one root-authorized repair by a fresh fixer. The parent invokes each worker once and does not poll.

Run focused source fixtures from the repository checkout:

```sh
python3 agent-config/test_erwin_taskflow.py
python3 agent-safety/herdr-routing/scripts/test_herdr_routing.py
```
