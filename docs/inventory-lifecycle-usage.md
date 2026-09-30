# Inventory, summaries, and reversible storage

## Retrieve only what is needed

Start with `belay context`. Follow the named Task using
`belay context compile --focus <plan-id>#t-001 --budget 2500`.
Then use `show` or a targeted `search` for omitted details. A live summary's
next action is a candidate, not authorization. A focused execution packet
must contain its complete required boundaries; if the budget cannot hold
them, the command reports the shortage instead of silently cutting them.

Task completion, Goal verification, Decision applicability, and summary
freshness are separate states. A Decision needs explicit adoption evidence
and scope; an old accepted status alone does not establish current validity.
Same-scope conflicts are review candidates, not machine-proven contradictions.

## Inspect and preview changes

```sh
belay inventory
belay inventory --format json --long-active-days 90
belay inventory --format json --now 2026-09-30T12:00:00Z
belay inventory preview <entry-id> --status archived > selected-preview.json
```

Inventory does not change source records. Each finding identifies its rule,
source version, reason, classification and proposed follow-up. An age threshold
or whitespace-normalized match does not authorize archival, merging, deletion,
or a changed Evidence verdict. Review originals and dependencies before choosing
an operation. After selecting a concrete preview:

```sh
belay inventory apply --file selected-preview.json > receipt.json
belay inventory restore --file receipt.json > restoration-preview.json
```

The last command only creates a restoration preview. Apply that preview
separately when selected. Stale revisions or changed originals are rejected;
repeating an already recorded operation does not create another transition.
Keep the receipt. These CLI checks bind bytes and revisions, not the identity
of the human who authorized the operation.

## Author a derived summary

```sh
belay lifecycle summary sources <full-entry-id> <full-evidence-id>
belay lifecycle summary store --file authored-summary.json
belay lifecycle summary list
belay lifecycle summary show <artifact-id>
```

Use the exact source bindings returned by `sources`. The artifact records a
schema version, artifact ID/revision, generator version, source bindings,
cited sections and limitations. Its `kind` is `derived-summary` and its
`authority` is `derived-only`. Preserve outcomes, Decision reasons and scope,
constraints, unresolved issues, lessons and artifact/Evidence references.
Label hypotheses and untested generalizations explicitly. See the schema and
authored quality fixture alongside this document.

When sources change or become unavailable, the artifact is stale. Retrieve the
originals or author a new revision. The core does not generate prose, call an
LLM, or certify semantic fidelity merely because JSON passes validation.
Never replace Evidence or Coverage with a summary's assertion of success.
A compiled query may use the latest current summary for a selected completed
Work or Review as optional prose. Required Goal/Plan/Task sections and original
Evidence remain separate. Stale or unavailable summaries fall back to original
excerpts; `show <source-id>` expands the original.

## Preserve originals while reducing loose files

Evidence begins in immutable individual files. Legacy monthly NDJSON remains
readable. Packs contain exact original payloads plus versioned IDs, revisions,
hashes and acquisition paths. Compaction previews fix source identities and
hashes before any cleanup. A pack must be durably published and verified
before unchanged individual source files can be retired. Partially selected
monthly files remain intact. Recovery resumes the recorded operation.

Only immutable Evidence and eligible terminal Work/Review enter the initial
packing policy. Goal, Plan and Decision originals remain ordinary files.
Restoring an old Work/Review never overwrites a newer live revision. A restored
entry can be reopened and edited through ordinary commands.

```sh
belay lifecycle pack preview --id <full-id> > pack-preview.json
belay lifecycle pack apply --file pack-preview.json
belay lifecycle pack original <full-id> --revision 1
belay lifecycle pack recover <receipt>
belay lifecycle pack restore <pack-hash> --id <full-id>
```

`original` returns the exact requested raw payload and digest, including a
historical revision. `restore` is an explicit mutation and preserves newer
live Entry revisions. Use the pack subcommands' preview, apply, recover and
restore paths deliberately.
First validate restoration on an isolated copy. Keep packs, manifests and
receipts in repository history. Fewer loose files does not imply fewer total
bytes, faster retrieval, or smaller Git history. External logs named by an
Evidence record are references, not automatically preserved attachments.

## Compatibility and recovery boundaries

Application version and storage version are separate. New readers accept
legacy data. Before publishing a new-format original, the storage guard changes
so a legacy 0.6.3 CLI rejects the repository rather than silently ignoring new
data. Stop legacy writers and uncoordinated manual editing before migration or
compaction. File locks coordinate cooperating current Belay processes; they
cannot control arbitrary external editors or already-running legacy binaries.

If raw publication succeeds but indexing fails, retain the reported Evidence
ID and repair/reindex. Do not blindly submit it again with a new ID. Missing
indexes are reconstructable from originals and verified packs. Corrupt packs
or conflicting same-ID originals require investigation; a cached success must
not hide them. Do not downgrade the config number to force old CLI access.
Use a compatible reader or restore a complete pre-migration repository snapshot.

```sh
belay lifecycle cache
belay lifecycle cache rebuild <exact-path-listed-by-preview>
```

Cache operations are limited to identified regenerable state and preserve
operation receipts. Do not delete `.belay/state` wholesale: it may contain
state whose regeneration is not established. Original data, packs, summary
history and recovery receipts are never classified as disposable cache.
