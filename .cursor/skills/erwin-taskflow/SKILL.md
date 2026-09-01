---
name: erwin-taskflow
description: Deliver one approved Belay Task through delegated implementation, independent review, and at most one bounded repair.
---

# erwin-taskflow

This is the canonical product-neutral workflow. Product files are adapters;
Belay and the runtime keep their existing schemas and remain authoritative.
The product's native sandbox, permissions, and approval UI own safety. This
Skill grants no permission and adds no guard or deployment authority.

## Ownership

The root orchestrator owns planning, routing, status, finding disposition,
integration, and human gates. A routed implementer owns one approved Task in a
dedicated Git worktree. A fresh reviewer inspects the validated Work diff
read-only from a separate context. A repair, when authorized, uses one fresh
fixer and the same dedicated worktree. Routed workers do not use jj; root
integration follows the repository's VCS policy.

Belay is the task and result store. Resolve the current Task fragment before
dispatch. Keep the handoff compact: Task pointer, role/profile, checkout or
validated Work locator, and any directly required dependency pointer. Return
only Work, Review, Evidence, and outcome pointers plus a one-line status.

## Workflow

1. **Intake.** Resolve the Task, its mapped criterion, Acceptance, Constraints,
   Non-goals, and material dependencies. Stop for a choice that would change
   scope, permissions, data safety, an external commitment, or an irreversible
   result. Root records the implementation difficulty and review profile.
2. **Implement.** Route one implementation context using the recorded profile.
   The worker changes only the Task scope, runs focused checks, creates exactly
   one linked Work, records Evidence, commits the Task on its worktree branch,
   and returns compact pointers. Model and effort selection come from
   `agent-safety/routing.json` and product role definitions.
3. **Review.** Route a fresh read-only context. Supply the current Task's mapped
   criterion and Acceptance, the validated Work diff, exact changed paths, and
   validation provenance. The reviewer may read the minimum unchanged
   dependency context needed to understand changed behavior. Findings cover
   only current-Task regressions introduced by the changed hunks; pre-existing
   or unrelated issues are reported separately and do not change this verdict.
4. **Repair once.** Root either accepts the review, stops, or authorizes one
   fresh fixer for selected in-scope findings and exact paths. Re-review the
   repaired hunks and affected Evidence in another fresh context. Any remaining
   blocker, new scope, or material disagreement returns to the human.
5. **Integrate.** Root verifies pointers, scope, Evidence, and commit before
   integration. Protected or externally visible actions remain separate human
   decisions.

A runner `pass` means only that its recorded validation and settlement checks
passed. It is not a semantic implementation, review, or Task-completion
verdict. Root reads the current Work and Review bodies and reconciles them with
the Task before acceptance. This workflow adds no outcome schema.

Each routed process is invoked once and allowed to finish. The parent does not
poll worker terminals or run conversational repair loops; completion is read
from the transport and Belay return records.

## Profiles

Implementation difficulty selects the product worker profile. Review risk
selects the review profile. Preserve the configured models and reasoning
efforts exactly. Use the narrowest profile justified by the Task, and record a
material reclassification before dispatch.

- **low:** localized, reversible work with focused validation.
- **medium:** non-trivial multi-file or integration work.
- **high:** approved architecture, security, contract, migration, or other
  high-impact work.

Review is independent at every level. Low-risk work may use a lightweight
review; medium and high-risk work require a fresh reviewer. Tasks may share a
review only when the approved Plan explicitly batches them without combining
their Task scope, Work, or Evidence.

## Boundaries

- Dedicated implementation worktrees, fresh review contexts, current round
  budgets, task/result/checkout binding, and root-owned integration remain.
- Native product controls decide filesystem and permission requests. Do not
  invent command allowlists, approval rituals, or bespoke safety authority.
- Do not widen the workflow into deployment, pull-request watching, backup,
  container, dashboard, or generalized recovery systems.
- A Task settles from its own Work and Evidence. Goal coverage is checked only
  during Goal reconciliation or release and cannot invalidate otherwise valid
  current-Task work.
- Pushing, publishing, protected-branch changes, deployment, force push, ref
  deletion, and other external or irreversible actions require specific human
  authorization.
