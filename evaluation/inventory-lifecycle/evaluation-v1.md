# Frozen evaluation v1 — 2026-09-30, before feature implementation

User approval: chat, 2026-09-30, "2については提案の内容で進めてOK".
Root resolved the updated harness; no substitute-policy approval is needed.

## Context experiment

Primary metric: total retrieved tokens over the five fixed navigation cases,
including fallback reads. Target: at least 20% less than the old compiler.
No increase in retrieval commands; median total CLI runtime regression <=10%;
at most one added maintenance operation. Quality: no missing required source,
false completion/Decision-validity claim, inaccessible original, or lost
history/Evidence relation. Neither fixed output budget nor ID presence alone
establishes success. Every required fact/source is checked.

Freeze source: tracked `.belay` originals at commit
`9d4b16e341a23ee505fad1d2315fb389b892a298`. Ignore local generated state and
unrelated later harness/trace additions. Restore that commit in temporary
repositories, reconstruct indexes with each binary, and preserve raw outputs.
Old binary is a release build of 0.6.3. Compare release builds, sequential
commands, three warmups followed by 15 repetitions. Report median and all raw
durations; no timing exclusions. Alternate old/new trials. Time setup separately.
Token unit: existing `markdown::estimate_tokens`: ceil(ASCII bytes / 4) plus
non-ASCII scalar count. The initial `baseline/report.json` used a rough UTF-8
estimator and is exploratory only; never silently compare its numbers with
the fixed estimator.

Fixed cases, agent format, budget 2500 where applicable:

1. Current work: `context compile`; must reach active
   `GOAL-20260925T214740-001-belay-guidance-boundarie`, active
   `PLN-20260906T134828-001-context-cost-first-phase`, and next candidate
   `PLN-20260906T134828-001-context-cost-first-phase#t-002`. Preserve statuses;
   next is not execution authorization. Goal verification must not be inferred
   from Task completion. If omitted, retrieve those two entries with `show`.
2. Resume: `context compile --focus
   PLN-20260906T134828-001-context-cost-first-phase#t-001`; require complete
   Task, mapped SC-001, Goal/Plan Constraints and Non-goals, assumptions,
   unknowns/stop conditions when present, and its Evidence IDs with freshness.
   Missing full boundary requires `show` of the Plan and mapped Goal; account
   for both retrievals. A budget failure is safe behavior but not successful
   navigation until the fallback retrieves the required information.
3. Historical Decision: `show
   DEC-20260725T203527-001-use-cytoscape-double-click-window-for-explore-ar`;
   require canonical ID, accepted status, rationale and source links. Do not
   equate historical acceptance to current applicability without scope and
   adoption evidence.
4. Prior failure: `show EVD-20260926T004249-001`; require exact record ID,
   fail verdict, source, summary, commit and linked target(s).
5. Goal evidence: `coverage`; require the guidance Goal above and missing
   verification reasons. Evidence cannot be fabricated by status or summary.

Human and agent outputs must resolve the same sources on boundary fixtures.
Adversarial fixtures: empty/all archived/multiple active/blocked, done Task with
missing/stale/failing Evidence, accepted unscoped Decision, accepted scoped
Decision with passing human-approval Evidence, supersedes/refutes, same-scope
conflict candidates, unrelated recent failures, long final constraint/stop,
ambiguous or missing Task mapping, required packet overflow. Stable ordering
must hold on identical state/query/budget.

Comparison matrix: old/new compiler on raw snapshot; old/new on identically
reorganized isolated snapshot if operations are selected. No real operation
selected means no cleanup effect can be estimated; report that limitation.
Procedure comparison fixes compiler/data and counts prescribed fallback reads;
do not claim a model behavior improvement from a scripted CLI probe.

## Lifecycle extension v1

Freeze before LC01–04 implementation. Baseline record/read/rebuild timings use
0.6.3 with one worker and 100 synthetic Evidence records on an isolated copy.
Concurrent matrix: 1 and 8 processes, 25 records each, repeated 3 times; two
independent worktree-equivalent copies each generate 25 at a fixed timestamp,
then integrate originals. Legacy monthly records remain fixture inputs.

Required adversarial cases: publication before/after file completion, before
and after DB indexing, unavailable DB, simultaneous reader, ID prefix ambiguity,
same ID same content duplicate and conflicting content, interrupted compaction
at snapshot/pack publication/cleanup, concurrent compactor, new writer record,
changed monthly file, partially selected month, corrupt/missing/version-unknown
pack, missing index, terminal Entry restored and updated, changed Entry during
cleanup, missing/stale summary binding, and source material outside repository.

Quality: 100% original payload/hash restoration; zero missing originals,
unintended overwrite, reference loss, double counting, or false verification.
Measure successful/failed/conflicting/recovered operations, loose-file count,
pack-inclusive bytes, Git history bytes, record and reindex timing, retrieval
tokens and semantic summary rubric independently. External raw logs are not
implicitly preserved by packing their references.

Operational targets apply the approved no-extra-search and <=10% median runtime
regression limits to like-for-like sequential record/reindex trials; original
preservation takes precedence over performance. For the file-count purpose,
the fixed all-eligible corpus should leave zero eligible loose original files
after verified packing (pack/manifest/receipt files counted separately).
No storage-byte or Git-history reduction target is asserted. One explicit
pack operation is the added maintenance action; recovery must be resumable.
If only quality passes, report limited trial, not full adoption or 0.7.0 final.

## Adoption and revision

BI-07 and LC06 are separate judgments. Full adoption requires both quality and
the relevant prior numerical goals. Do not update to final 0.7.0 until both are
eligible. Changes to criteria require a reason and new version; retain results
against this original v1. The fixtures establish bounded evidence, not a
guarantee about arbitrary data or future models.
