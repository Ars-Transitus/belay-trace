# OB-10 append and reconciliation design

The chosen connector has no documented compare-and-swap or server idempotency-key
guarantee. The v1 path is therefore a single-writer, explicitly authorized dedicated
result area. This is not exactly-once delivery or complete concurrency prevention.
No helper performs network calls or alters shared page body/status automatically.

An operation binds schema version, exact target page/result area, canonical Capsule
digest, proposed summary/artifact/unresolved/Decision-candidate payload, exact
pre-observation digest and explicit permission reference. Canonical operation ID
is derived once from this fixed intent, excluding changing outcome state.
Do not include raw logs or micro-tasks. Existing external Id/URL remains untouched.
Permission references are records of actual authorization, not authentication.

States:

- pending: fixed reviewed operation has not been dispatched. A matching fresh
  pre-observation and exact permission are prerequisites to dispatch.
- unknown: dispatch began or response was lost/partial. Persist before dispatch;
  never automatically retry merely because the response was absent.
- applied: read-back contains exactly one operation ID with matching payload.
- conflict: pre-observation changed, duplicate operation IDs, or same operation ID
  has different content. Stop and reconcile; no overwrite.

After unknown, complete read-back can establish applied or conflict. An observation
that cannot establish presence/absence stays unknown. Absence alone does not prove
an in-flight request cannot later complete: require settled original request and a
new explicit retry decision before pending again. Keep each attempt in the ledger.
Do not make an append request until concrete target/content permission is established.

The helper validates strict caller observations and returns proposed ledger/diff
JSON on stdout. It does not prove the observation came from Notion. The Skill owns
fetching complete target observations, durable local ledger persistence before
network dispatch, and honest reporting. Live connector effects remain a separate
end-to-end test requiring concrete destination approval.
