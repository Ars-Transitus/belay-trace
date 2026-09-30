# CX-01 section contract v1

Frozen before implementation on 2026-09-30, for BI-04a/04b/05 and future OB-05
callers. Evaluation: root evaluation/inventory-lifecycle/evaluation-v1.md.

`context_sections::render_sections(header, sections, format, budget)` accepts
`ContextSection { id, title, requirement, order, sources, content }`. IDs and
sources are caller-owned canonical strings, including fragments and Evidence.
Sections sort by `(order, id)`; duplicate section IDs fail. Required content
is never truncated. Optional content is admitted whole.

`RenderedSections` returns text, token estimate, included section IDs, source
IDs and omissions with reasons. `BudgetFailure` reports budget, required token
count, shortfall, required IDs and reason. Failure becomes a CLI validation
error, with no partial execution packet. The estimate is
`markdown::estimate_tokens` (ASCII bytes/4 rounded up, plus non-ASCII scalars),
not a model tokenizer. Selection reserves the larger human/agent rendering;
the same inputs and budget select the same sources. Headers, source IDs and
omission notices count against budget. There is no output-tail truncation.

Sources are rendered once as ordered canonical references with
`belay show <ID>` retrieval instructions. Optional omissions identify section
and budget reason. Callers separately report scope/archive/duplicate exclusions.

Focus packets require the complete task definition/section, mapped SC
criterion, Plan and Goal Constraints/Non-goals, assumptions, unknowns, explicit
stop conditions, status/evidence distinction and sources. Ambiguous mapping
fails. Missing legacy mapping is Unknown. Live summary may omit detail and
requires focus before executing next candidates, which confer no authority.
Active Goal/Plan IDs, next candidates, blockers/unknowns and verification
summaries remain. Full task boundaries belong to focus.
The Task section comes from the validated original Plan, from its exact Task
heading through the next top-level Markdown heading of the same or higher
level. Nested headings remain intact; fenced/quoted heading examples do not
create Task boundaries. Duplicate Task headings or mapping rows fail closed.

Decision applicability requires Accepted, nonempty string metadata.scope,
passing human-approval Evidence and no inbound supersedes/refutes from an
Accepted Decision (`replaced`). Two or more `explicit-scope` decisions on the same
exact scope become `conflict-candidate`; peers without adoption stay unconfirmed. Missing
scope/adoption is unconfirmed. Freshness is separate; timestamps cannot prove
applicability. Semantic conflicts are not detected. Completion never proves
Goal verification.
Canonical scope references to validated originals are compared with the selected
Goal/Plan scope. Query/link selection and live summaries suppress known outside
scope Decisions. An explicit seed may retain one with its actual scope and
outside-scope/current-applicability-Unknown disclosure. Arbitrary human scope
labels are preserved with current scope applicability Unknown; their meaning
is not inferred as a Goal reference. Every retained Decision displays its scope.

Indexed Entry status/body/metadata is not authority over a changed original.
Compile compares each original's canonical revision/content hash with its index
binding using the managed reader (which supports validated pack fallback).
Focus validates its Plan and mapped Goal before emitting their boundaries.
Missing, corrupt or unsynced originals cause an explicit failure rather than
retaining stale Decision applicability or Task constraints.
