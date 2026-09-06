---
name: safe-autonomy
description: Run Claude Code /loop or repeated agent work safely inside a sandboxed workspace boundary, with recoverable worktrees, stop conditions, and belay evidence. Use for long-running, autonomous, unattended, repeated repair, or multi-step repository work.
---

# safe-autonomy

The product native sandbox, permission system, and approval UI are the safety authority. This Skill does not classify shell commands, grant permissions, or create a parallel approval process. Follow the active repository policy and stop when the native control requires a human decision.

For an approved delegated Task, use the implementation profile already selected in `agent-safety/routing.json`. The root orchestrator owns planning, integration, status, and human gates. Implementers use dedicated Git worktrees; reviewers use fresh read-only contexts. Read only the Task fragment and directly required dependency context.

Return Work and Evidence pointers through Belay. Do not poll workers. Allow at most one root-authorized repair by a fresh fixer over exact finding paths, followed by fresh review. A remaining blocker or widened scope returns to the human.
