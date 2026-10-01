---
name: omnia-consolidate
description: Prepare a human-readable Omnia result from a verified Belay Capsule and reconcile an explicitly authorized dedicated result append without blind retries or automatic status changes.
---

# Omnia Consolidate

Use the matching repository's `scripts/omnia_consolidate.py` with its Contract
validator and schemas. Keep this repository layout when distributing the source
skill; no consumer installation is implicit. Use the existing exact Area, source,
repository and connector selection. A selected Area is not permission to crawl it.

## Prepare a reviewable result

Read the stored Contract, durable core receipt, actual artifact references and
Evidence. Obtain a Capsule at a fixed as-of from the matching core. Preserve its
execution, verification, human acceptance, lifecycle and source-freshness axes.
Do not turn technical verification into human acceptance or Omnia completed.

Prepare only a result summary, artifact references, unresolved issues and optional
Decision/Learning candidates. Do not copy raw logs or micro-task transcripts.
Retain existing external Id/URL properties. Decision promotion and completed/status
changes are separate concrete human judgments and operations.

Select an exact page and dedicated result area. With current MCP/ntn capabilities,
use append plus read-back; do not assume CAS, idempotency keys, linearizability or
exactly-once. Do not replace a shared page body with `ntn pages edit` for an append.
One writer owns this local operation ledger/result area; another service writer
can still race it. If the chosen area must first be created, its creation also
needs specific authorization and an unknown-response recovery plan.

Present the exact target, result content, Capsule digest and proposed append
(including the fact that a generated operation-ID marker will be attached) before
asking for missing permission. Do not prepare an authorization-bound operation
using a fictional or placeholder permission_ref. After the concrete permission
exists, run prepare with its real reference to derive the operation ID and ledger.
Confirm that target/result/Capsule are unchanged from the approved proposal; a
meaningful change returns to permission review, while the disclosed generated
marker does not require duplicate approval. Reuse an existing specific
authorization for the same operation and destination. A permission_ref is a pointer to real human authority,
not an authorization granted by the helper or a digest. Preparing a request is
not permission to send it. Native sandbox/trust/approval remain authoritative.

## Observe and bind

Fetch the complete dedicated result area with the selected connector. Stop on
unknown blocks, truncation, missing pagination, denied access or unavailable source.
Observe all occurrences of this operation ID, including duplicates. Normalize the
actual result content faithfully; do not discard mismatches to make it compare.

The helper's `--help` defines its JSON request/observation shapes. Supply the exact
page/result_area_id, complete flag, canonical snapshot_sha256 of the full fetched
result-area snapshot, and operation occurrences/content. Compute observation_digest
from canonical JSON of all observation fields except observation_digest. Preserve
the underlying snapshot locally for audit. These caller observations are not
independently authenticated by the helper.

Prepare binds Capsule, exact result, target, pre-observation digest and permission
reference. Persist the returned immutable operation and initial ledger locally.
The helper prints an envelope; subsequent commands require its `operation` and
`ledger` objects, not the whole envelope. Never substitute stdout error JSON for a
successful artifact. Check exit status before saving any proposed state.

```sh
python3 scripts/omnia_consolidate.py prepare --capsule capsule.json --request request.json
python3 scripts/omnia_consolidate.py begin-dispatch --operation operation.json --ledger ledger.json --observation before.json
python3 scripts/omnia_consolidate.py reconcile --operation operation.json --ledger ledger.json --observation after.json
```

These commands only print proposals. Use crash-safe local writes (new temporary
file, flush/fsync, atomic replacement, then fsync its parent directory) and retain attempt history. Do not dispatch
until the exact returned unknown ledger is durably saved and the response permits
`dispatch_allowed_after_persist`. Never replace a ledger with an older generation.
An exact existing occurrence means already applied, so do not append again.
Conflict or incomplete pre-observation blocks dispatch.

## Apply and reconcile

Immediately before network dispatch recheck the human stop, source validity,
Contract cancellation/supersede/expiry and exact authorization. An outage allows
reads/local checks only; do not modify or launch workers until required source
validity is observed again. Keep the execution snapshot unchanged on recovery.

Append exactly the operation content with its operation ID to the authorized
result area, using the connector's documented append request. A queued response,
HTTP success alone, a missing response or partial response is not applied proof.
Keep unknown until complete exact-target readback finds exactly one matching
operation ID and matching content. Feed that observation to reconcile and durably
save the returned ledger. Duplicates, mismatches or later drift are conflict.
Report separately: applied, manual reflection pending, conflict, and unknown.

## Unknown and retry

Never rerun prepare to erase an unknown attempt or generate another operation ID
for the same unresolved append. Reconcile the retained operation/ledger first.
After conclusive complete absence, establish that the original request is settled
(no delayed append can still arrive). If that fact is unavailable, stay unknown.
A retry also needs an explicit human retry decision tied to the same operation:

```sh
python3 scripts/omnia_consolidate.py authorize-retry --operation operation.json --ledger ledger.json --observation absent.json --decision retry-decision.json
```

Persist the returned pending attempt; reacquire a valid pre-observation and repeat
begin-dispatch before any allowed retry. A helper flag never substitutes for a
real complete observation, settled request or human decision. Never edit terminal
states or hashes to force progress. Preserve unresolved outcomes in Belay and
leave Omnia shared status/Decision changes for the specifically authorized action.
