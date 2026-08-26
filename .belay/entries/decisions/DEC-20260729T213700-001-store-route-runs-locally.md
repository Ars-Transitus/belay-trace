---
schema_version: 1
id: DEC-20260729T213700-001-store-route-runs-locally
type: decision
title: store-route-runs-locally
status: accepted
created_at: 2026-07-29T21:37:00+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: DEC-20260729T200801-001-use-primary-thread-route-runs-and-deterministic-b
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Decision

- Route MVPは、実行途中のRoute Input、Assessment、Proposal、Human Response、Materialization Previewを、`.belay/state/route/<run-id>/`のlocal operational stateとして保存する。
- run bundleはBelayの正式な事実、判断、履歴ではなく、再開とstale検査のための非権威的な作業状態とする。
- 人間が承認しmaterializeした結果だけを、既存のGoal、Plan、Decision、Work、Evidenceなどの正式なBelay成果物へ反映する。
- run bundleが失われた場合は、seedと現在のBelay成果物からRoute Inputを決定的に再作成し、外部AIのAssessmentとProposalを再生成できるものとする。意味推論の再生成結果が同一になることは保証しない。
- 初期版はGit、端末間、clone間でのrun bundle共有を保証しない。export/importまたはtracked raw artifactは、実利用上の必要が観察された場合に再検討する。

## Rationale

- Routeが扱う途中のProposalをfirst-class Belay entryにすると、未承認の推論結果がSource of Truthと混同される。
- `.belay/state/`はローカルの運用状態という既存の境界に合い、チャットの中断後も同一端末で作業を再開できる。
- Git保存がなくても、正式なBelay成果物は残るためRoute runを再構成できる。ただし外部AIの非決定的な出力まで復元するものではない。

## Consequences

- Route runは明示seed、input fingerprint、proposal ID/revision/hash、authority stateを持ち、再開時に現在のBelay stateとのstalenessを検査する。
- raw Proposalは正式な承認や実装開始の根拠にならない。
- Materialization PreviewとHuman Responseは同一のinput fingerprintおよびproposal revisionへ結び付ける。
- `.belay/state/route/`はreview artifactとして扱わず、必要なprovenanceと最終結果は正式なBelay成果物へ残す。

## Source

- Human approval in the 2026-07-29 conversation: 「これはGitになくてもGitにあるBelay成果物から再生成できる（決定論的ではないけど）ので推奨の方針でOK」。
