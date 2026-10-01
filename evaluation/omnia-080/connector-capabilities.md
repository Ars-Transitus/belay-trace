# OB-07a — connector capability and distribution decision

Decision (implementation owner, 2026-10-01): use the already-connected Notion MCP
at the Skill boundary. Keep deterministic helper/source validation in this Belay
repository under scripts/ and skill source under skills/; no Erwin or installed
consumer changes, no independently published package in this release. Core has
no Notion credentials, SDK or LLM dependency.

Evidence: callable Notion fetch/update_page tool descriptions and parameter schemas
inspected in this chat. This is interface inspection, not a successful write test.

| Capability | Evidence / status | Consequence |
| --- | --- | --- |
| Fetch exact page | fetch accepts ID/URL and exposes page_last_edited_at | Use explicit selected IDs, not broad crawl |
| Completeness | fetch documents truncated/unknown_block_count/unknown_block_ids | Reject incomplete mandatory source; omission is not proof of completeness |
| Conditional revision update | update_page has no documented expected-version/If-Match parameter | Unsupported by this interface; do not assume CAS |
| Native idempotency key | update_page has no documented idempotency parameter | Do not assume exactly-once |
| Append | insert_content position start/end documented | Use only an explicitly authorized dedicated result area |
| Read-after-write | fetch available; linearizable consistency undocumented | Reconcile operation ID and exact content; uncertain result remains unknown |
| Async result | allow_async and task polling documented | Acknowledged queue is not applied; confirm succeeded and fetch result |
| Pagination | fetch promises entity content; bulk collection query is separate | Source bundle must carry completeness/continuation evidence; no all-relations guarantee |

Single-writer scope is one configured result area. A local operation ledger can
avoid blind retransmission but cannot prevent another writer on the service.
A lost response is unknown until read-back conclusively identifies the operation.
No automatic shared body/status overwrite, no automatic completed/Decision update.
Real writes need a concrete page and applicable human authorization.

Configuration will explicitly bind Area ID, Scriptorium data-source ID, repository
identity and connector kind. Fetching the source page does not authorize any other
Area/repository. Required snapshot backup configuration remains a separate real-data
intake gate; synthetic fixture acquisition tests can proceed without external writes.

## ntn read fallback (2026-10-01)

User requested ntn after MCP HTTP500. Installed ntn `pages get <id> --json`
retrieved the exact Plan with `truncated=false`, `unknown_block_ids=[]`,
`in_trash=false`, page identity/properties and data-source parent. ntn `api -X GET`
is also available. Exact connector config `ntn` is supported; helper still rejects
a source envelope whose connector differs from independent expected config.

The Plan is readable while its parent page currently returns HTTP500. Do not infer
that every page is available or that source Area membership is independently
verified. Parent-dependent real intake remains blocked until that fact is established.
No live write through ntn was performed. The absence of documented CAS/idempotency
is unchanged. ntn pages edit is whole-content editing and is not the fallback append
mechanism; use only a specifically authorized result area's append API after
confirming its supported request shape. No shared Plan body/status replacement.

## Explicit per-run Area binding

The human selected Area `3cbc2ad7-b431-814c-9c1c-e6fcaffad340` in this chat.
This is the independent configured boundary for this invocation, not a global
single-Area restriction. No requirement is imported from the Area page body.
Therefore failure to fetch that optional body does not block the fixed local
implementation or issuance from the complete explicitly selected Plan. Do not
claim that the service's Area-membership relation was independently verified.
The source envelope's Area binding is based on the human's selection, while its
page ID/data-source/content/completeness must still match the actual fetched Plan.
A source without this explicit binding, a different repository/data-source, or
an unavailable mandatory selected source remains blocked. Multiple Area configs
may coexist; one exact expected config is selected for each invocation.

ntn's selected OpenAPI operation `PATCH /v1/blocks/{block_id}/children` was
inspected with `--spec` (no mutation): request has `children` (max 100) and
optional `position`, with only block ID and Notion-Version parameters. This
supports an explicitly authorized append of a new dedicated result block, not
CAS or native idempotency. Reconcile using operation ID and complete block reads.
Source: https://developers.notion.com/reference/patch-block-children .
