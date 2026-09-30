# Omnia–Belay 0.8.0 handoff

## Start

Use product commit `457136c4c1861742d195c8e881823630ad7ebd00` as OB-00 baseline.
Use the built `target/release/belay` (0.7.0) for its new commands; the global
user-local CLI was deliberately not replaced. Current release checks are in
`release-final.json`.
The integration Plan is [Omnia–Belay Integration](https://app.notion.com/p/3b8c2ad7b43180d7a0a8d78e58d2392b).
`omnia-plan-source.json` preserves the fetched source, edited2026-09-30.
Read the current installed repository policy; user harness changes are a separate
working-copy change and must remain intact. Do not treat them as product changes.

Begin OB-00: build a code/evidence-backed reuse/gap table for CLI, Route, context,
Evidence, metadata, sync/rebuild/export and generated Skill source. Then formulate
OB-01 authority/lifecycle/storage decisions. Do not jump into all0.8.0 features.

## Reuse

- `src/context_sections.rs`: ContextSection/SectionRequirement/render_sections;
  required boundaries fail explicitly on insufficient budget. Contract context
  connects here in OB-05; do not duplicate the renderer.
- `src/context.rs`: scope selection, focus and original Evidence boundary.
- `src/inventory.rs`: read-only findings, source-bound previews and status receipts.
- `src/lifecycle.rs`, `src/pack.rs`: durable originals, shared writer lock,
  exact historical payloads, preview/apply/recover/restore. Summary/pack hashes
  establish integrity, never human authorization.
- `src/summary.rs`: derived-only source bindings; summaries never verify Goals.
- `src/agent.rs`: shared generated guidance source. Consumer copies are not edit targets.
- `Makefile`, `scripts/build-local.py`: reuse temporary-BINDIR deployment.

## Acceptance and limits carried forward

0.7.0 integrity and context-quality gates passed. Storage latency remains worse
than v1's+10% goal; the user accepted this specific tradeoff in evaluation-v2.md.
Do not transfer that exception to OB-EVAL or claim storage performance improved.
Original v1 reports and failing tests/reviews remain available for provenance.
One original Work was archived with receipt; no content/Evidence deletion.
Root storage schema remains1 until a new-format Evidence/pack operation; stop old
writers before schema2. The installed user-local CLI was not replaced.
No push,tag,registry publication or consumer installation was performed.

## Decisions still belonging to 0.8.0

OB-01 fixes retry/new revision/cancel/expire/supersede/offline behavior, snapshots,
secret handling and projection edits before dependent implementation. OB-07a fixes
connector capability and helper distribution. OB-EVAL numeric operational limits
need explicit human decision before OB-04a/07b implementation. This readiness
handoff does not approve unrelated external writes, publishing or broad crawling.
Execution, verification and human acceptance remain separate state axes.
