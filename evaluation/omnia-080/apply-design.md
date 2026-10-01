# OB-04b/c implementation boundary

OB-EVAL v1 is human-approved for single-user dogfooding. It is not passing
quality evidence and does not authorize publication or arbitrary Notion updates.

- Core owns Contract validation, preview/preconditions, immutable canonical
  artifact storage, Goal/Plan projection and durable receipts. It must not invoke
  Python, Notion or an LLM to decide whether an apply is valid.
- Keep Python fixture canonical UTF-8 bytes compatible with Rust; compare golden
  fixtures, including Japanese/non-ASCII text. Validate before creating storage.
- Bind a preview to repository identity, Contract digest and deterministic planned
  projections. A user-supplied hash is content binding, not proof of authority.
- Serialize writers with existing lifecycle lock. Store transaction intent before
  projections, with planned stable IDs and exact intended content hashes. Retry
  may only reuse exact originals/IDs; no timestamp or random-ID regeneration.
- Contract id+revision with a different digest is a conflict. A new revision creates
  new projections; no silent supersede, no old projection overwrite.
- Recovery observes each boundary: original, intent, Goal, Plan, completed receipt.
  Missing projection may be created from durable intent; unexpected bytes or
  orphan state must fail closed. SQLite-only receipts are insufficient after DB loss.
- Do not put raw source snapshots in tracked projections. Actual snapshot intake
  stays behind configured backup/recovery and explicit source selection.
- A root storage compatibility gate must prevent 0.7 writers from mutating new
  Contract projections. Do not silently enable a new format in the live checkout
  while using an old trace writer; develop and validate in temporary repositories.
- Offline/unknown source freshness, explicit cancellation, expiry and supersede
  block mutation/dispatch. A local status flag cannot prove live Notion reachability;
  the adapter is responsible for obtaining and binding a fresh observation.
