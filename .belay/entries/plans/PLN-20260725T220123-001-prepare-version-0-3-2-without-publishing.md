---
schema_version: 1
id: PLN-20260725T220123-001-prepare-version-0-3-2-without-publishing
type: plan
title: Prepare version 0.3.2 without publishing
status: completed
created_at: 2026-07-25T22:01:23+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: fulfills
  id: GOAL-20260725T220102-001-prepare-a-validated-0-3-2-patch-release
metadata: {}
---

## Intent Brief

### Problem

Cargo metadata remains at 0.3.1 while the working history contains backward-compatible Explore activation and readability fixes intended for the next patch version. The repository has no remote tag, GitHub Release, changelog, or release workflow defining publication semantics.

### Desired Outcome

Create a reviewable 0.3.2 release candidate with consistent package metadata and passing verification, while keeping publication actions behind separate human gates.

### Success Signals

- Cargo.toml and Cargo.lock agree on 0.3.2.
- The release-candidate diff contains only intended fixes, trace records, and version metadata.
- Rust checks pass locally and Playwright passes in CI or a browser-capable environment.
- No tag, release, PR, publish, or merge occurs without explicit approval.

### Constraints

- Preserve CLI, schema, API, Browse read-only behavior, and Rust 1.87 compatibility.
- Version bump, draft PR creation, and squash merge of PR #16 are approved; tagging, GitHub Release creation, and publishing remain separate approvals.
- Keep the prior user-tuned Explore spacing values unchanged.

### Non-goals

- Release automation, registry publication, or unrelated feature work.
- Retroactively creating tags for earlier versions.

### Assumptions

- This is a hypothesis: 0.3.2 will be used as the next package version even though no public release mechanism currently exists.
- The accumulated changes are patch-level because they preserve documented interfaces and storage formats.

### Unknowns / Decisions Needed

- Unknown: whether 0.3.2 means only Cargo metadata, a GitHub tag and Release, or registry publication.
- Resolved: the human approved version metadata implementation followed by PR creation.
- Resolved: after Copilot review, the human approved squash merge of PR #16 and closure of Issue #15.
- Tagging, GitHub Release creation, and registry publication remain unapproved.
- Separate human approval is still required for tag, GitHub Release, or publishing.

## Scope and Approach

1. Update Cargo.toml package version and regenerate the matching Cargo.lock package entry.
2. Verify no dependency version was accidentally changed.
3. Run Rust 1.87 checks, rebuild, doctor, and Playwright where available.
4. Perform fresh-context review of scope, validation, and release readiness.
5. Create and verify the approved draft PR, perform the separately approved squash merge, then stop before tag, GitHub Release, or publish actions.

## Delivery Map

| ID | Goal Criteria | Outcome / Task | State | Verification |
| --- | --- | --- | --- | --- |
| T-001 | SC-001 | Set the package and lockfile version to 0.3.2 without dependency churn | verified | Focused diff of Cargo.toml and Cargo.lock |
| T-002 | SC-002 | Record the backward-compatible 0.3.2 scope and exclusions | verified | Work and review entries match the actual diff |
| T-003 | SC-003 | Run Rust 1.87 fmt, clippy, all-target tests, rebuild, doctor, and Playwright | verified | Local checks plus passing PR CI Evidence |
| T-004 | SC-001, SC-002, SC-003 | Fresh-context review the release candidate | verified | Review entry linked to Work |
| T-005 | SC-004 | Enforce separate human gates for external release actions | verified | PR creation and merge have explicit Evidence; tag, GitHub Release, and publishing remain prohibited |

## Acceptance and Evidence

Metadata changes are implemented only after explicit approval. Version 0.3.2 is not called released until the intended external publication action and passing verification are both identified.
