---
name: jujutsu
description: "Manage root-orchestrator version control with Jujutsu (`jj`): snapshot approved Goal/Plan, inspect and rewrite local history non-interactively, merge worker branches, and synchronize bookmarks. Use on the main checkout of a colocated jj repository. Do not use for routed worker Git worktrees, reviewer git -C inspection, Git worktree add/remove, or hosting-provider PR operations."
---

# Jujutsu

This skill is for the **root/main orchestrator** on the main checkout of a
colocated jj repository. Shell tools may edit files. For orchestrator snapshot,
history, merge, and bookmark operations, use `jj` and do not fall back to raw
`git`.

It does not apply to routed Codex/Claude workers, reviewers, or Git worktree
lifecycle. Those layers use Git (or belay) as assigned below.

Repository `AGENTS.md` / `CLAUDE.md` rules and human approval gates take
precedence over this skill.

## Version control layers

| Layer | VCS | Role |
| --- | --- | --- |
| Codex/Claude worker dedicated worktree | Git | `git worktree add`, file changes, branch commit |
| Reviewer | Git read-only | inspect the target diff with `git -C`; never cd into the checkout |
| root/main orchestrator | jj | snapshot, history, final merge, bookmark |
| Belay | belay | Goal/Plan/Work/Evidence source of truth |
| worktree cleanup | Git plumbing | `git worktree remove` and related plumbing; an explicit exception |

The Herdr runner owns `git worktree add`. After a passing review, merge with
jj, then remove the worktree with Git plumbing. That cleanup is the explicit
exception to "orchestrator uses jj". The runner may also remove a just-created
unused worktree after an allocation failure; that is the same exception.

## Invariants

- One completed Task is one dedicated jj change on the main checkout after the
  worker branch is merged. Create or describe that change before rewriting
  history around it.
- Jujutsu has no staging area: every unignored working-copy modification is
  snapshotted into `@`. `jj file untrack` is not a substitute for staging.
  Never finish or publish while unrelated paths are present.
- Use explicit revisions, paths, bookmarks, and remotes. Do not rely on an
  implied current branch; jj bookmarks do not have an active/checked-out state.
- Add global `--no-pager` to commands that display status, history, diffs, or
  lists.
- Do not open a TUI, text editor, diff editor, pager, or external merge tool
  in an agent run.
- Local task authorization does not authorize push, PR creation, merge to a
  protected bookmark, destructive cleanup, or publication.

## Concurrent agents

One jj workspace has one working-copy change. Never let concurrent agents
operate in the same checkout.

Routed workers are isolated by Git worktrees that the Herdr runner creates
with `git worktree add`. Do not `jj workspace add` for them, do not create
those worktrees yourself, and do not `git worktree remove` until the worker
branch has been merged with jj — except the runner's narrow allocation-failure
rollback, which is Git plumbing.

Stay on the main checkout. Inspect a worker tree read-only with
`git -C <checkout_locator>` if needed; never cd into it to run jj.

## Start one orchestrator change

Inspect the current repository before creating anything:

```sh
jj root
jj --no-pager status
jj --no-pager log -r 'heads(::@ & mutable())' -n 12
jj --no-pager diff --summary -r @
```

Classify the current `@` and its parentage:

- Fact: an empty `@` can still represent another task boundary. Do not silently
  repurpose it when its ownership is Unknown.
- If `@` contains unrelated work, preserve it. Select the intended base
  explicitly; do not squash, abandon, restore, or rewrite the unrelated change.
- If the task depends on current work, use that dependency change as `<base>`.
  Otherwise use the agreed bookmark, remote bookmark, or change ID.
- If the correct base or ownership of existing changes is Unknown and choosing
  would change the result, ask the human.

Create the change and verify it:

```sh
jj new <base> -m "<imperative task description>"
jj --no-pager log -r '@ | @-' -n 2
jj --no-pager status
```

Use `jj describe -m "<updated description>" <change>` to revise a description
without opening an editor. Prefer `describe` plus a later explicit `new` over
`jj commit`; `jj commit -m "..."` also creates a new child change and can blur
task boundaries.

A loop snapshot is this same operation: the approved Goal and Plan must exist
in the source commit before `git worktree add`. Snapshot with jj, then keep
the main checkout clean. Do not launch a worktree against a Plan that exists
only in the root working copy.

## Routine command reference

Use these forms directly; do not consult help for them.

```sh
# Working copy and scope
jj --no-pager status
jj --no-pager diff --summary -r @
jj --no-pager diff --stat -r @
jj --no-pager diff --git -r @

# History and one change
jj --no-pager log -r 'mutable()' -n 20
jj --no-pager log -r '<revset>' -n 20
jj --no-pager show <change> --summary
jj --no-pager show <change> --git

# Change lifecycle
jj new <base> -m "<description>"
jj describe -m "<description>" <change>
jj edit <change>

# Explicit, non-interactive rewrites
jj rebase -r <change> -o <new-parent>
jj squash --from <source> --into <destination> -m "<combined description>"
jj split -r <change> <path>... -m "<selected-part description>"
jj restore --from <source> --to <destination> <path>...

# Bookmarks and remotes
jj --no-pager bookmark list --all-remotes
jj bookmark create <bookmark> -r <change>
jj bookmark move <bookmark> --to <change>
jj --no-pager git remote list
jj --no-pager git fetch --remote <remote>

# Conflicts without a merge tool
jj --no-pager resolve --list -r <change>

# Operation inspection and recovery preview
jj --no-pager operation log -n 10
jj --no-pager operation show <operation> --stat
```

Before `rebase`, `squash`, `split`, `restore`, `abandon`, `undo`, or any
`operation restore/revert`, inspect the exact revision or operation and its
affected descendants. If it includes pre-existing or human-owned work, stop
and ask. For conflict resolution, edit conflict markers as ordinary files and
re-run `status` and tests; do not invoke `jj resolve` without `--list`. Treat
`resolve --list` exit code 2 with `No conflicts found at this revision` as a
clean result, not a tool failure.

If a jj operation fails because the sandbox cannot write repository metadata,
request the required permission for that same scoped jj command. Do not switch
to raw Git for an orchestrator snapshot, history, merge, or bookmark. Git
worktree add/remove and reviewer `git -C` remain the assigned Git layers, not
a fallback.

## Keep the orchestrator change isolated

During implementation on the main checkout, run at meaningful checkpoints:

```sh
jj --no-pager status
jj --no-pager diff --summary -r @
```

After merging a worker branch, or before handoff or publication, run:

```sh
jj describe -m "<final task description>" @
jj --no-pager status
jj --no-pager diff --summary -r @
jj --no-pager diff --git -r @
jj --no-pager log -r '@ | @-' -n 2
```

Verify that every changed path belongs to the task, the description is
non-empty, no conflicts remain, and relevant tests passed. If extra work is
discovered, either move it to a separately authorized task change using
explicit paths or leave it untouched and report it; never hide it with
`file untrack`.

Then, if a worker worktree is no longer needed, remove it with Git plumbing
(`git worktree remove`). Do not `jj workspace forget` a Git worktree.

## Push approval gate

Push only an explicit bookmark to an explicit remote. Avoid `--all`,
`--tracked`, `--deleted`, `--change`, and `--named`; they broaden or obscure
the publication target.

Prepare the local bookmark without pushing:

```sh
# New local bookmark
jj bookmark create <bookmark> -r <approved-change>

# Existing local bookmark
jj bookmark move <bookmark> --to <approved-change>

jj --no-pager show <bookmark> --summary
jj --no-pager bookmark list <bookmark> --all-remotes
jj --no-pager git push --dry-run --bookmark <bookmark> --remote <remote>
```

For a new remote bookmark, add `--allow-new` to both the dry-run and the
eventual push, and explicitly tell the human that a new remote bookmark will
be created.

After a successful dry-run, ask for human approval. Include:

- the exact push command;
- remote and bookmark names;
- target change ID and commit ID;
- dry-run output describing the remote update;
- changed-path summary and verification results.

Approval is valid only for that exact preview. A general request to implement,
commit, finish, or prepare a change is not push approval. If the bookmark
target, remote state, diff, tests, or dry-run output changes after approval,
invalidate it and ask again.

Immediately before the approved push, repeat `status`, bookmark inspection,
and the exact dry-run. Then run only the approved command:

```sh
jj --no-pager git push --bookmark <bookmark> --remote <remote>
# Add --allow-new only when the approved preview created a new remote bookmark.
```

Never add `--allow-private` or `--allow-empty-description` to bypass safety
checks. After pushing, verify the result:

```sh
jj --no-pager git fetch --remote <remote>
jj --no-pager bookmark list <bookmark> --all-remotes
jj --no-pager log -r '<bookmark> | <bookmark>@<remote>' -n 4
```

Report the exact local and remote targets. Do not create a PR or merge unless
separately requested and authorized.

## Commands that must not become interactive

Do not run:

- `jj arrange`, `jj diffedit`, `jj config edit`, or `jj sparse edit`;
- `jj resolve` except with `--list`;
- any jj command with `-i`, `--interactive`, `--tool`, or `--editor`;
- `jj describe`, `jj commit`, or description-combining `jj squash` without
  `-m`, `--stdin`, or another explicit non-editor message choice;
- `jj split` without explicit filesets and `-m`.

Use ordinary file edits, explicit filesets, and message flags instead. If the
requested result genuinely requires hunk-level selection or a merge UI, stop
and ask the human to perform that interaction or approve a different
non-interactive decomposition.

## Version drift

This reference targets jj 0.44.0 and the current official CLI reference:
https://docs.jj-vcs.dev/latest/cli-reference/

If a covered command rejects an option, run `jj version`, check the official
reference for that command, and use one targeted `jj help <command>` only if
the installed version remains ambiguous. Report the mismatch and update this
skill when behavior changed; do not improvise orchestrator operations with
raw Git.
