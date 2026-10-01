---
schema_version: 1
id: DEC-20261001T063224-001-omnia-boundary-proposal
type: decision
title: omnia-boundary-proposal
status: proposed
created_at: 2026-10-01T06:32:24+09:00
updated_at: 2026-10-01T06:32:56+09:00
revision: 2
tags: []
links:
- relation: fulfills
  id: GOAL-20261001T062823-001-omnia-integration-readin#sc-002
metadata: {}
---

Proposal, not accepted: evaluation/omnia-080/design-proposal.md。推奨はimmutable Contractと可変実行投影を分離し、機微snapshotは既定非追跡＋backup、offlineは既定read/verifyのみ、Intent fieldsをhash保護。D1/D2/D3未確定。connector/配布はOB-07a、数値上限はOB-EVALで別判断。0.7例外は継承しない。
