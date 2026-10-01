# Inventory and derived summary contract v1

Inventory is read-only. Reports carry repository, optional HEAD, explicit observation
time, counts, status counts, links, Evidence freshness and stable findings.
Every finding has stable ID, rule version, source references (ID/revision/SHA256),
reason, actor, class and recommendation. Readable Entry references bind the actual
original revision and exact bytes. Unavailable originals retain indexed identity and
canonical index digest for diagnostics only; current invalid bytes never bind to an
old indexed revision. Missing originals,
index drift and Evidence freshness are separate findings. Long-active age defaults
to 90 days and is configurable; it is a heuristic, never completion. Exact duplicate
candidates normalize whitespace only; semantic equivalence is never claimed.
Task validity uses existing fragment/map lint. Decisions have explicit scope only
when accepted, scalar nonempty metadata.scope, passing human-approval Evidence,
and no accepted superseding/refuting Decision. Matching scopes yield conflict
candidates; missing support remains unconfirmed. Missing, invalid, conflicting or
drifted originals also leave applicability unconfirmed, and cannot establish a
superseding/refuting Decision. Revision or updated-at drift is detected independently
of the canonical content digest. Evidence freshness is separate.

Selected status previews carry ID/revision/hash, before/after, inbound references,
operation ID and preview digest. Apply verifies preview digest, revision and original
hash and delegates to existing guarded status mutation with transactional retry
receipt. Restore creates a fresh selected preview using the receipt's post-revision;
intervening change is rejected. No delete, merge, auto-archive or real data operation
is implied. Retry is idempotent.

Summary v1 is authored JSON: artifact ID/revision, generator_version, kind=derived-summary,
authority=derived-only, source bindings (ID/revision/canonical original SHA256),
sections (heading/text/source_ids), limitations. Core validates source bindings,
section citations and authority; it never calls an LLM or judges prose semantics.
Save is no-clobber, requires current source binding and increasing artifact revision.
Entry bindings hash exact original bytes; Evidence bindings hash canonical full
record JSON independent of location. Indexed/original/loose/unindexed counts are
separate; unindexed originals produce read-only drift findings. Type/status counts,
long-active age and Task map validation use all readable originals, including
unindexed files. Unavailable indexed entries remain visible as diagnostics, but do
not contribute stale type/status counts or original-derived relationships. Generator version
is nonempty and independent of source and artifact revision.
Get/list do not regenerate. Missing/stale sources are explicit; valid summaries never
replace Evidence or Coverage authority. Pack-aware readers are used when available.

Cache preview only names reconstructable SQLite index, with explicit rebuild method.
No original, summary, pack, manifest, route input or receipt is a cache candidate.
Unknown cache paths are rejected. Regeneration explicitly rebuilds the index from
originals; this is separate from preservation receipts.

## Bounded local verification

`cargo +1.87.0 test --test inventory_summary -- --nocapture` passed 10 fixtures
on 2026-09-30 in the isolated clone. Fixtures cover unchanged read-only originals/
index, stable human/JSON finding IDs, unindexed original, exact duplicate candidate,
scoped adoption/supersedes, unconfirmed Decision, stale/failing Evidence, invalid
Task map, stale preview, idempotent retry, restore, summary stale/missing/tampering,
raw-only original changes, and receipt-preserving explicit cache regeneration.

1000 independent Note fixture originals: one informational debug collection trial
513321 microseconds, human output 429078 bytes, JSON output 657077 bytes,
1000 findings. Time excludes fixture creation, includes read-only inventory and
independent original enumeration; output serialization measured separately as bytes.
This is a scale probe, not a comparative performance or adoption result. The exact
trial printed by the current test run is authoritative when code changes.

Semantic quality has a separately reviewable authored fixture in
`summary-quality-fixture.md`; automated format validation is not semantic review.
Shared writer-lock and packed-source integration require the storage lane; the
parent must validate these integrated paths before accepting lifecycle delivery.
