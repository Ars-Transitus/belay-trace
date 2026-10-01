# OB-EVAL v1 — approved for single-user dogfooding

Approved 2026-10-01 in this chat: user states Belay is for personal use and accepts the proposed case/ceilings as-is; dogfooding issues may lead to later revisions. Approval fixes the criteria, not the evaluation result.

## Fixed case

Fixed Code case: extend this repository's read-only Contract CLI/helper with a
validation diagnostic showing the exact failing input field, scoped to the
helper/tests/documentation. Freeze input/Contract/commit before timed execution.
It exercises a real code change without publishing or changing third-party data.
Consolidate uses a separately authorized dedicated result area; without that
permission end-to-end external validation remains incomplete.

## Quality gates (mandatory, not traded against speed)

- Reconstruct delegation for 100% of fixed cases.
- List every AC with its evidence status.
- Zero false verified/accepted, zero continuation of prohibited operations,
  zero duplicate core retries.
- Never classify uncertain external update as success.
- Include changed input, cancellation, supersede, expiry, offline, source denied,
  missing/stale/failing evidence, interrupted apply, lost response and retry.

## Approved operational ceilings

- Clarifications caused by lost already-confirmed intent: <= 1 per case.
  A genuinely new requirement/authority decision is recorded separately.
- Human review time: <= 20 minutes per case, measured from active review intervals,
  excluding idle time waiting for the person.
- Operator active time: <= 30 minutes per case (includes the human review above;
  not an additional 30 minutes). Record wall-clock completion time separately.
- Local compile/validate/preview: <= 2 seconds for the frozen single-case input on
  this host; five warm runs, report each and maximum. No network latency included.

These are initial usability ceilings, not measured improvements or inherited
0.7 criteria. Historical human time cannot be reconstructed reliably; exclude
comparative efficiency claims unless a prospective baseline is captured.
If the human changes the ceilings, preserve this proposal and label the approved
version separately. OB-04a/07b numerical gate is now satisfied. Real snapshot intake also waits for backup configuration.
