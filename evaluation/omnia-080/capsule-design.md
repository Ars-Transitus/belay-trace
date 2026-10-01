# OB-06 deterministic Outcome Capsule

Core reads a validated stored Contract/receipt and Evidence mirrors. It never
interprets free text, calls Notion, or infers human acceptance from tests.

- Explicit evaluation timestamp (`as_of`) fixes temporal decisions. Captures
  after that timestamp cannot count. Same Contract, Evidence set, repository
  state and timestamp must produce identical canonical bytes.
- Every Contract AC maps by its stable position to the receipt Goal SC-NNN.
  Only verifies links to that exact SC contribute. Goal-wide evidence does not
  silently cover every AC. Missing evidence is an empty evidence list, with
  unverified state; stale evidence cannot make verification pass.
- Each listed Evidence is identified by ID and canonical-record SHA-256.
  Existing core freshness policy is evaluated at as_of, not wall-clock now.
  Unknown freshness is conservatively non-passing. Failures are retained.
- Criterion aggregation: any failed => failed; nonempty all passing => verified;
  passing plus stale/missing => partial; otherwise unverified. Global status
  uses all criteria, with the same conservative ordering.
- Execution is an explicit observation, separate from verification; human
  acceptance defaults pending. The first implementation does not offer an
  automatic accept/reject transition. A later explicit human evidence binding
  can be added without treating an agent assertion or hash as authentication.
- Contract lifecycle is reported; expiry is evaluated at as_of. Source freshness
  remains unknown unless separately observed; local evidence cannot establish
  current Notion state.
- No mutation of Contract, Goal, Plan, Evidence or external state. Return JSON
  conforming to capsule-v1 plus a human-readable explanation of uncovered ACs
  through a separate renderer, without silently expanding the schema.

Tests must include all ACs missing, mixed passing/stale, failing, future captures,
fixed-time repeatability, expired Contract, schema compatibility, and absence of
inferred human acceptance. Use isolated stores; the live schema2 store stays intact.

One Evidence record may explicitly verify several SCs. Repeat its binding under
those ACs without dropping coverage; repeated ID must keep identical hash/state.
Duplicate IDs within one AC remain invalid. The Python Capsule validator must
allow this exact cross-AC reuse so actual core output is schema-compatible.

## Conservative repository-state guard after OB06 review

Legacy freshness thresholds alone can report a descendant commit or changed working
product as fresh. Capsule must additionally require an existing Evidence commit
that is ancestor-or-equal to current HEAD. Unknown/unresolvable commits or HEAD
are non-passing. Nonignored tracked/staged/untracked changes outside `.belay/`
make pass Evidence stale because its tested product state is unbound. Uncommitted
Belay trace records are allowed; this is not a globally clean checkout requirement.
Scoped working-diff attestations are not implemented in v1. Keep failures visible.
