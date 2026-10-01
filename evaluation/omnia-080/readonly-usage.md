# OB-02/03 read-only prototype

This Python helper is a synthetic boundary experiment, not the 0.8.0 release.
Run from the repository root:

```sh
python3 scripts/omnia_contract.py validate tests/omnia_contract/fixtures/confirmed-input.json
python3 scripts/omnia_contract.py compile tests/omnia_contract/fixtures/confirmed-input.json
python3 scripts/omnia_contract.py preview tests/omnia_contract/golden/contract-v1.json
python3 -m unittest discover -s tests/omnia_contract -p 'test_*.py' -v
```

Output is JSON on stdout. No output directory, network connector, repository
mutation or apply is provided. Schema files are shipped in the test tree for
this prototype; distribution is a later OB-07a decision. The validator supports
the subset of JSON Schema used by these checked-in schemas, not arbitrary schemas.

The fixture is synthetic. A schema-valid `confirmed` label is not proof that a
person approved its meaning; the Skill must bind actual human instructions to
the input before real use. Hash validity is not authority or source freshness.
The helper does not read or verify the source bytes referenced by the manifest.
Source acquisition, raw hash checks, storage durability, clock-driven lifecycle,
full Evidence evaluation and human acceptance verification are downstream work.

Git will retain the necessary execution requirements, all AC/constraints and stop
conditions, and provenance hashes without secrets. Full real source snapshots
remain local, untracked, and require explicit backup/recovery configuration before
intake. Unknown source freshness stops mutation/dispatch; it does not prevent
this synthetic read-only experiment.

## Input provenance boundary

Input ID/revision, confirmation reference, assumptions and context references are
explicit. Source edited_at is optional: absence means unknown, not unchanged.
Confirmation references are structural provenance, not authenticated approvals.
Real snapshot bytes and completeness still require OB-07b validation.

Prototype verification aggregation is fixed: any failed evidence => failed;
nonempty all passing => verified; passing mixed with missing/stale => partial;
only missing/stale or empty => unverified. Global state uses the analogous
criterion aggregation. Real Evidence freshness and authenticity are not evaluated.
