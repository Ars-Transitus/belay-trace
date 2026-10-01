---
name: omnia-requirements
description: Turn an explicitly selected Omnia Idea, Goal or Plan into confirmed Code requirements for a single Belay repository, preserving source and delegation boundaries.
---

# Omnia requirements

Use the configured Area ID, data-source ID, repository identity and connector as
independent expected settings. One selected Code Goal/Plan is the intake boundary;
relations are candidates, not permission to crawl or execute all related work.
This source skill requires this repository's `scripts/omnia_source.py`,
`scripts/omnia_contract.py` and schemas under `tests/omnia_contract/schemas/`.
Deployment must retain that layout; installing this file alone is insufficient.

## Idea and Goal

Read the selected source and only dependencies needed to understand it. Check the
existing Idea/Goal IDs and mapping before preparing creates. If no Goal exists,
propose one with Outcome and observable success criteria. With multiple plausible
Goals, show the candidates and ask which Goal the Idea belongs to; do not silently
choose or create another Goal. Preserve an existing Idea's identity on retry.

Separate the proposed page operation (create/update), exact target, relevant
source/version and differences. Use existing concrete authorization for the same
operation and destination. Without it, prepare the reviewable content before
asking. A request to analyze an Idea does not authorize page creation or execution.
A broad request to implement an Idea still requires resolving the selected Goal
and the concrete page operations; it does not identify those by itself.
Do not overwrite an existing Id/URL property to establish a new mapping.

## Plan and confirmed input

Distinguish updating the existing Plan from creating a new Plan linked to the
selected Goal. Turn meaning into Outcome, stable requirement IDs, source refs,
verifiable AC, constraints, non-goals, assumptions, unknowns, delegation scope,
verification and stop conditions. Keep a requirement's ID when its meaning stays
stable; do not silently reuse an ID for another requirement.

Use the confirmed-input schema as the mechanical shape. Bind input_id/revision,
confirmation_ref, fixed contract_id/revision/issued_at/compiler_version and exact
target. Confirmation must point to actual human instructions accepting the meaning;
a `confirmed` label or schema pass is not human approval. Existing explicit user
instructions can supply that evidence; do not ask again merely to populate a form.
Unresolved mandatory meaning or delegation keeps status draft and blocks emission.
The current v1 helper rejects any nonempty unknowns; retain nonblocking open issues
in the draft/Plan until classified explicitly, never silently discard them.

Validate connector envelopes against independent settings:

```sh
python3 scripts/omnia_source.py compile envelope.json --config expected-config.json
python3 scripts/omnia_contract.py validate draft-input.json --kind draft
python3 scripts/omnia_contract.py validate confirmed-input.json
python3 scripts/omnia_contract.py compile confirmed-input.json
```

Draft validation checks shape only and returns issuable=false. It does not prove
source completeness or permission, and compile still rejects draft inputs.
The helper performs deterministic compilation, not interpretation. Preserve the
exact input revision and source bundle; rewording meaning requires a new input
revision and differences for human review. Do not regenerate issuance values on
retry. Capsule/evaluation success cannot supply missing input confirmation.

## Original source and pause

Keep full real snapshots local and outside Git. Before real intake, configure
backup destination, retention and a recovery procedure. Git may contain necessary
Belay requirements/AC/constraints/prohibitions/stops and provenance hashes without
secrets; the digest is not a recoverable copy. Inspect what is being saved, rather
than relying on automated secret detection alone.

Unknown blocks, truncated content, denied access, archived/deleted mandatory source,
missing pagination evidence or cyclic required dependencies prevent issuance.
Absence of a flag is not evidence that retrieval was complete. The current helper
validates a caller-provided envelope; it does not itself fetch Notion or prove the
connector's claims. Adapt actual connector results conservatively.

When Notion is unavailable, allow reads/local checks only. Stop modifications and
new dispatch. On recovery, reacquire and compare source changes/cancellation and
Contract expiry before resuming. Do not replace the execution snapshot in place.

Return exact Idea/Goal/Plan IDs, input revision, confirmation pointer, source bundle
hash and unresolved decisions. For bounded execution, hand compact pointers to the
existing repository delivery workflow; this skill does not create another runner.
