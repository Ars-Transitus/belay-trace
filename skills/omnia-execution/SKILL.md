---
name: omnia-execution
description: Issue an Omnia Execution Contract from confirmed Code requirements and resume the same bounded Belay work after checking source validity and durable receipts.
---

# Omnia execution

Use after omnia-requirements has produced a confirmed input with exact source,
repository, human confirmation and delegation boundaries. Use this repository's
compiler, source and ledger helpers with its matching Belay core. This source
skill depends on the repository layout; copying the skill file alone is not a
working installation. Do not install consumer settings as part of execution.

## Before issuance or resumed mutation

Read the existing Contract, Work and latest relevant Evidence. Resolve the exact
repository and source configuration independently of the fetched envelope. Use
existing concrete human authorization for the same scope and destination.

Re-fetch only the selected source and required dependencies. Validate completeness,
source identity, cancellation, supersede, input changes and Contract expiry.
Notion outage, denied/incomplete source or unknown validity permits reads/local
checks only: stop modifications and new workers. A caller-supplied
`observed-current` flag records a completed observation; it is not a network check
and must never be set merely because local files validate.

If source changed, retain the old execution snapshot. Compare meaning and return
to the confirmed-input revision decision; do not silently replace requirements
or regenerate issuance timestamps. Cancellation, supersede or expiry stops the old
execution. A fresh Contract is a new explicit decision, not a retry workaround.
A human stop persists until the human explicitly re-enables the relevant work.

For actual sources use the approved original and backup locations and retention
policy. Verify the exact saved bytes and recovery copy against recorded hashes.
Do not put full source snapshots or secrets in Git. Git records must retain the
necessary requirements, AC, constraints, prohibitions, stops and provenance.

## Compile, preview and apply

Compile the exact confirmed input with fixed contract_id, revision, issued_at and
compiler version. Validate and preview against the intended repository. Inspect
all projected requirements, AC-to-SC mapping and the exact preview digest.

```sh
python3 - <<'PYCOMPILE'
import pathlib, subprocess
result = subprocess.run(
    ["python3", "scripts/omnia_contract.py", "compile", "confirmed-input.json"],
    check=True, capture_output=True,
)
# Exclusive creation refuses stale files instead of overwriting them.
with pathlib.Path("contract.json").open("xb") as output:
    output.write(result.stdout)
PYCOMPILE
belay contract validate --file contract.json
belay contract preview --file contract.json --source-freshness observed-current
belay contract apply --file contract.json --approve <exact-preview-digest> --source-freshness observed-current
belay contract show <contract-id> --revision <revision>
```

Select the verified matching core binary, not an older global CLI. Before the first
schema 3 apply, stop old writers and uncoordinated editors and establish a recovery
copy. Only then supply `--legacy-writers-quiesced`; the switch acknowledges a fact,
it does not stop processes. Never use an old CLI to bypass the schema gate.

Apply's `--approve` digest binds reviewed content; it does not authenticate a human
or replace native sandbox/approval. Reuse existing task authorization for allowed
local writes. Publication, installation and external writes retain their own
specific boundaries.

Use the durable `contract show` receipt for ID ledger reconciliation. Runtime
`recovered` or `unchanged` responses are not durable receipt inputs. Reapplying
identical bytes uses the same IDs; any identity, digest, target, preview, projection
or receipt mismatch stops for repair. Do not mint another revision to hide an
incomplete apply. Original-only pre-intent interruption may be retried with the
same Contract; otherwise require the core's recovery/integrity checks.

## Compact delivery handoff

Use the repository's erwin-intake and existing delivery workflow; do not create a
second runner. Hand over exact pointers to Contract ID/revision/digest, durable
receipt, Belay Goal/Plan and Task, existing Work, selected source observation and
Evidence IDs. Include allowed paths/operations, verification and stop conditions.
Compile the focused Contract context; a budget error requires a larger sufficient
budget or explicit failure, never trimming mandatory boundaries.

Mutable Belay Task breakdown/progress is distinct from protected Contract Intent.
Do not edit requirements, AC, provenance or protected projection prefixes in place.
Boundary overruns return to the human/intake decision before dependent work.

## Resume and return

After interruption inspect existing processes, durable receipt, current artifacts,
Work and Evidence before retry. Follow the preflight above before mutation or new
dispatch. Resume the same Contract/Work where valid; do not create duplicates from
an empty working set or a lost response. A completed worker is not verified success.

Record actual checks and unresolved conditions. Generate the Outcome Capsule only
from validated stored Contract and Evidence at a fixed evaluation time. Keep
execution, verification and human acceptance distinct. Pass the Capsule and exact
artifact pointers to Consolidate; local completion does not authorize Notion status
updates, Decision promotion, publication or tag push.
