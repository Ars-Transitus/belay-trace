# Lifecycle storage contract v1

Frozen before implementation, 2026-09-30, governed by canonical LC00-04 and root
evaluation/inventory-lifecycle/evaluation-v1.md. New immutable Evidence IDs use
128 secure random bits. Legacy timestamp IDs remain readable. One complete,
fsynced no-clobber original publishes per evidence/records/ ID. SQLite indexing
is separate: saved-but-unindexed returns original ID and error; sync repairs.
Config schema2 publishes atomically first; new CLI reads1+2, old0.6.3 refuses.
Stop old writers before upgrade: running old binaries do not honor new locks.

Tracked packs/<SHA256>.json format1 retains sorted manifest descriptors (ID,
revision, original path, raw SHA256), exact UTF8 payloads and descriptor hash.
Filename validates whole pack; all payload hashes, identities, versions and
paths validate. Evidence revision1. Same ID/revision/raw hash deduplicates;
differing hash conflicts. Highest Work/Review revision wins without destroying
older packs. Only terminal Work/Review and immutable Evidence qualify. G/P/D
remain loose. Raw SHA256 binds snapshots; semantic Markdown hash omits revision.

Apply snapshots exact sources, holds recursive exclusive advisory writer lock,
revalidates, adopts schema2, durably publishes/verifies pack, journals immutable
tracked lifecycle/receipts, stages sources by atomic rename, verifies staged
bytes and retires only matches. New original paths are never removed. Changed
staged files restore without clobber. Full monthly files retire only if every
record selected and whole-file hash matches; partial months remain. Recovery
resumes receipts idempotently. Original information stays in verified packs;
loose files may be removed. External referenced logs are not implicitly kept.
Restore recreates exact originals without overwriting newer or conflicting live
revisions. All Belay writers share lock. Unmanaged editors must quiesce during
apply because already-open inode edits cannot be coordinated by advisory locks.

Readers exclude incomplete files and validate packs. Fsync failure is never
claimed durable success. Rebuild preserves packed revisions/links/Evidence;
packed mutations materialize exact original then existing revision/CAS logic.
Managed sources reject symlink, nonregular and outside-root. Unsupported lock
or entropy platforms fail closed. Derived-index verification must reject missing,
extra or different indexed Evidence and pack corruption instead of false pass.

Acceptance: legacy/new read and old refusal; parallel/worktree IDs; visibility;
saved-unindexed repair; mixed duplicates/conflicts; corrupt/missing/unknown pack;
interrupted apply/recover; changed/partial months; writer serialization; exact
restore; newer preservation; packed rebuild/mutation and reference consumers.
File-count, bytes, Git history, timing are separate. No performance improvement
or final0.7.0 is asserted without frozen evaluation.
