# Route runs

Use Route when the human asks to reconstruct one active decision thread into
typed Assessment and Proposal artifacts. Keep semantic reasoning in the agent:

```sh
belay route start --seed <goal-or-plan-id>
belay route template <run-id> assessment
belay route submit <run-id> assessment --file <assessment.json>
belay route template <run-id> proposal
belay route submit <run-id> proposal --file <proposal.json>
belay route template <run-id> response
belay route submit <run-id> response --file <response.json>
belay route preview <run-id>
belay route pending <run-id>
belay route apply <run-id> --approve <exact-preview-hash>
```

- Treat Route Input as the fixed source bundle and Assessment/Proposal as
  advisory. Preserve Fact, Human Observation, Assumption, Hypothesis, Unknown,
  Conflict, and Belay references in their typed fields.
- Convert the human's explicit chat response into Human Response. Never infer
  acceptance, broaden selected operation IDs, or reuse a response after the
  Proposal hash changes.
- After `preview`, call `pending` and present only that one returned Preview's
  run ID, revision, and operation summary to the human. Keep its hash internal.
  Treat ordinary-language approval as valid only when the pending Preview is
  unique, the approval unambiguously targets it, and no later conversation
  changes its scope. Discard the pending binding on a new Preview, changed
  Input/Proposal/Response, an ambiguous reply, a revision request, multiple
  pending approvals, or a material conversational detour; then re-present a
  freshly checked Preview. `pending`/`apply` verify freshness and the exact
  hash, but Route does not authenticate or interpret chat and does not replace
  repository approval gates.
- Run state under `.belay/state/route/` is local and non-authoritative. Accepted
  materialized Belay entries are the durable source of truth.
