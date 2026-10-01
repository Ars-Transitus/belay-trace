# Inventory and lifecycle — 0.7.0

Local release preparation; no tag, publication or user-local installation.
The human accepted the measured storage-latency tradeoff on2026-10-01 under
`evaluation/inventory-lifecycle/evaluation-v2.md`. Original v1 runtime failures
remain visible; this release does not claim a storage speed improvement.

This version adds read-only source inventory, explicit reversible status
previews, required context sections with budget failure, authored source-bound
summaries, distributed individual Evidence files, and validated packs retaining
exact original payloads. Required execution boundaries and original Evidence
remain authoritative; summaries are optional derived prose.

## Compatibility

Readers accept existing schema 1 data and monthly Evidence. The first new
Evidence record or pack publication installs schema 2 before exposing its new
original. Older released CLIs reject schema 2; stop old writers before upgrading.
Already-running legacy writers and uncoordinated manual editors cannot be made
safe by an advisory lock, so quiesce them during migration and compaction.
Application and storage schema versions are independent.

A published record whose SQLite update fails is reported as saved but not
indexed, with its canonical ID. Repair using sync or rebuild instead of retrying
as a new record. A durability error after publication is explicitly uncertain;
inspect the named original rather than assuming failure means nothing was saved.

## Recovery and limits

Review a pack preview, apply its exact snapshot, retain receipts, and recover
interrupted operations by receipt. Originals remain in verified packs. Restore
by pack hash and optional ID; newer live Work/Review revisions are retained.
Use `lifecycle pack original ID --revision N` for exact historical raw payloads.
Check restoration on an isolated copy before applying storage changes to live
data. Never force a config downgrade to bypass an incompatible old reader.

Packing fewer loose files does not imply smaller total or Git-history storage.
External raw logs named in Evidence are not automatically included. Summary
JSON validation and freshness do not certify semantic correctness. Existing
historical doctor warnings remain separate from new implementation failures.

Frozen evaluation results, independent review findings, selected archive receipt
and release checks are retained in `evaluation/inventory-lifecycle/`. The v1
storage runtime limit remains unmet; v2 records the explicit human release
exception. Integrity and context-quality requirements remain unchanged.
