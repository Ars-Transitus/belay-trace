# Belay 0.8.0 Omnia integration

Adds immutable Execution Contracts, deterministic Goal/Plan projection and receipts,
Contract-first context, and Evidence-bound Outcome Capsules. Source Skills and Python
helpers handle selected Notion input and explicit, reconciled result appends.

## Verification and limits

The real diagnostic dogfood verified all four acceptance criteria and appended one
specifically approved result to Notion with exact read-back. Core retries and identity
ledger reconciliation were no-ops. Human operation and review time were not measured;
the usability thresholds therefore remain unverified. The user explicitly authorized
commit, push, PR creation and main merge with that limitation on 2026-10-01.
This is a delivery decision, not a measured efficiency claim or inferred Capsule acceptance.

Raw snapshots and backups stay in ignored local directories. Notion outages stop
mutations. A Capsule does not infer source freshness or human acceptance. External
appends need concrete permission and complete read-back; uncertain delivery remains
unknown and cannot be blindly retried.

## Compatibility

Existing 0.7 commands remain supported. First Contract apply upgrades its repository
storage to schema 3 and requires legacy writers to be quiesced. Older binaries must
not write that store. The operator store remains schema 2 until Contract apply.
Python helpers require Python 3.10 or newer and the adjacent schemas under tests.
No tag, registry publication, consumer installation or external distribution is included.
