# Authored semantic fixture v1

This is a bounded human-review fixture. JSON validity and digest matching do not
establish semantic quality. Replace illustrative IDs with fixture original IDs
when constructing the artifact; preserve every source reference.

## Original completed Work (WRK-fixture)

Outcome: the staging import accepts UTF-8 CSV and rejects malformed amounts.
Constraint: operate only on the staging fixture; production migration was excluded.
Unresolved: legacy Shift-JIS inputs have not been tested.
Artifact: staging-import.rs. Verification: EVD-fixture, pass, local UTF-8 cases.

## Original accepted Decision (DEC-fixture)

Rationale: strict amount parsing prevents silently truncating invalid input.
Scope: staging CSV import only. Explicit adoption: EVD-approval-fixture.
No claim was made about production or every encoding.

## Authored summary

Outcome [WRK-fixture, EVD-fixture]: staging UTF-8 fixture import accepts valid
amounts and rejects malformed amounts; Evidence covers those local cases.
Decision [DEC-fixture, EVD-approval-fixture]: strict parsing was adopted for
staging CSV import because silent truncation would hide malformed input.
Constraint [WRK-fixture]: production migration remained excluded.
Unresolved [WRK-fixture]: legacy Shift-JIS inputs remain untested.
Hypothesis / lesson [WRK-fixture, DEC-fixture]: explicit rejection may reduce
silent corruption for other importers; no general result has been established.
Artifacts and verification [WRK-fixture, EVD-fixture]: staging-import.rs and the
original local test record are the paths back to the result.

## Review rubric

- Outcome matches bounded source and does not invent completion or deployment.
- Decision reason and exact adoption scope remain visible.
- Constraints and unresolved issue are retained.
- Lesson is labelled Hypothesis; it is not presented as verified fact.
- Original Work/Decision/Evidence and artifact references remain reachable.
- Evidence verdict and freshness must be obtained from the original record.

Intentionally rejected summary: "All imports are now verified safe for production."
It generalizes beyond UTF-8 staging, loses the unresolved encoding question and
production exclusion, and substitutes prose for Evidence authority.
