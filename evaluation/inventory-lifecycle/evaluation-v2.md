# Adoption decision v2 — 2026-10-01

Human authorization: “どれもOK8.0進められるように整えちゃって”.
This accepts the disclosed one-Work archive operation, measured storage latency
tradeoff, and local 0.7.0 completion so Omnia 0.8.0 can proceed.

V1 and every failing result remain unchanged. Context numerical and all integrity,
source, concurrency, recovery and review gates remain required. The v1 storage
runtime +10% release gate is waived for this 0.7.0 release by human acceptance
of observed record12.1ms/rebuild14.5ms versus contemporaneous8.2ms/10.5ms.
This is an explicit release exception, not a benchmark pass or a new universal
latency target. Retain durability; future performance changes require measurement.

Apply only the exact archive-proposal.json Work, validate receipt and before/after
integrity, and compare identical reorganized snapshots with old/new compilers.
Then update local version to0.7.0, run release checks and isolated make deploy,
and prepare the Omnia0.8.0 handoff. Push/tag/publication and actual user-local
installation remain outside this authorization.
