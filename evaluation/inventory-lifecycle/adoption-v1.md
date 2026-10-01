> Historical v1 assessment. See `evaluation-v2.md` and `release-final.json` for
> the2026-10-01 human-authorized completion; original failures below are retained.

# Adoption assessment against frozen v1 — release gates remain open

No formal 0.7.0 adoption or release is claimed. `evaluation-v1.md` remains
unchanged; all earlier measurements and failed reviews are retained.

## Context: fixed quality and numerical checks pass; selected-data gate is open

The integrated `context-acceptance-v1.json` comparison measured 8537 → 5585 retrieval
tokens (34.58% reduction), 7 → 5 commands, and paired medians 0.606578 → 0.631570
seconds (+4.12%). Both binaries used Rust 1.98.1 release builds. The initial
frozen baseline was 0.448396 seconds; it is retained. The paired control helps
separate contemporaneous environment cost from the compiler comparison.
No fixed required source was missing. These five scripted cases do not certify
unknown-query or model behavior, or replace adversarial quality review.

Independent review repaired nested Task omission, ancestor-policy loss and
query hits broadening explicit Task/criterion scopes. Fresh review of integrated
source `7fa7884bf56dd28809c447351c30ddb7ad4b17bc` found no remaining context or
Evidence/help behavior regression: context14 and four focused Evidence checks
passed. Its sole remaining finding was a README link absent from that review
snapshot; the link now targets included trial release notes, with a separate
link-only review. See `reviews-v1.json`.

The raw-snapshot comparison passes its bounded checks, but BI-07 also depends on
BI-03c and a selected-data comparison. Those remain blocked by human selection;
do not claim complete BI-07 adoption from the raw experiment alone.

## Lifecycle: bounded integrity passes; runtime criterion fails

`lifecycle-acceptance-v1.json` records 675 successful records across repeated 1/8-process
trials, plus 50 records from independent copies merged without ID collision.
An older binary refused schema 2 without changing originals. On the frozen
real-data snapshot, 195 records were packed, 51 loose files retired, all eligible
loose records were removed from the packing candidate set, and all 116 original
source files restored with exact SHA-256 equality after synchronization.
All 113 Entry and 147 Evidence IDs remained readable after reconstruction.

Tracked-candidate files declined 130 → 82, while bytes grew 461202 → 1182016.
Loose Git objects in the isolated two-commit experiment grew 207136 → 477127
bytes. This is file-count reduction, not byte or Git-history reduction. The
experiment did not modify live source records or preserve external raw logs.

Independent review of storage/inventory SHA `540c613c605cce6c93ff5894071b2d2336d4f204`
passed after revision recovery and mutable-edit repairs. The root integrated
suite passed 240 tests after recompilation. The first root run failed one
revision assertion once; the independent reviewer passed that probe six times.
The cause remains Unknown; both root results are in `validation/`.

`storage-acceptance-v1.json` measured median record 0.0121034 seconds and rebuild
0.0144722 seconds. Against frozen baselines 0.00699379/0.00881350, regressions are
+73.06%/+64.20%. A contemporaneous old-binary control measured
0.00817921/0.01050129, leaving +47.98%/+37.81% respectively. Neither comparison
passes +10%. No durability guarantee or criterion was weakened.

The user's explicit numerical approval covered context. Extending +10% to
storage was the orchestrator's conservative v1 interpretation, not a separate
human decision on acceptable storage latency. Old append publication has no
explicit fsync; the new contract fsyncs completed files and their directory.
That is a verified code-level contract difference, not measured attribution of
the runtime increase. A changed criterion needs a reason, new version and the
applicable decision; v1 failures must remain visible. Under v1, quality-only
success permits limited trial, not full LC06 adoption.

## Summary and selected-data boundaries

`summary-acceptance-v1.json` holds compiler and original data fixed: authored summary
navigation uses one read/384 estimated tokens versus two reads/526. The fixture
retains bounded outcomes, rationale/scope, constraints, unresolved encoding,
hypothesis, artifact and original Evidence references. This is authored fixture
evidence, not a statistical LLM semantic-quality guarantee. Hash/JSON checks
alone cannot certify prose fidelity.

The completed Phase 6 Work archive preview passed isolated apply/rebuild/restore
again with unchanged Coverage. Human selection remains pending. No live status
operation ran, so the selected-data cleanup comparison and effect remain
unmeasured. BI-03c is not complete and no no-target conclusion is substituted.

## Release boundary

BI-07 and LC06 are separate gates. Unmet lifecycle runtime
criteria and the unselected data operation are not waived by ordinary tests.
Cargo remains 0.6.3 in the development tree, without formal release, push, tag,
publication, user-local installation or the dependent Omnia chat.

Source and validation are reviewable through `product.patch`,
`product-manifest.json` and `verification-v1.json`. Root remains uncommitted to
preserve the separate user harness changes. Tests bind the local review snapshot
and exact file hashes; Git-based Evidence freshness in the root repository may
remain unresolved until integration. No overall Goal verification is inferred.
