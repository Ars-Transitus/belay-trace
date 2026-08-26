---
schema_version: 1
id: GOAL-20260725T194008-001-make-explore-node-activation-reliable-and-unambi
type: goal
title: Make Explore node activation reliable and unambiguous
status: completed
created_at: 2026-07-25T19:40:08+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

## Summary

- Explore graphのノード操作を明確かつ信頼できるものにし、展開操作と詳細ページへの移動が互いに競合しないようにする。

## Success Criteria

- [SC-001] 関連ページを持つノードをダブルクリックすると、近傍展開を発火せず、一貫してその詳細ページへ移動する。
- [SC-002] ノードのシングルクリックは、ダブルクリックとの判定後に一度だけ近傍を展開し、予期しないページ移動を起こさない。
- [SC-003] canvasに依存しないAccessible Goal listとkeyboard導線を維持し、グラフ操作を利用できない場合も同じ情報へ到達できる。
- [SC-004] シングルクリックとダブルクリックの競合を再現する自動interaction testが追加され、既存のBrowse、Rust、Playwright検証が通る。

## Constraints

- Browseは読み取り専用のままとし、SQLite、managed Markdown、Evidence、sync baselineを変更しない。
- 既存のExplore internal API、deep link、Cytoscape.js 3.34.0、CSPおよび外部通信不要の制約を維持する。
- シングルクリックの待ち時間は、ダブルクリックを識別できる範囲で最小限にし、重複timerや遅延した古い展開処理を残さない。

## Non-goals

- Explore graph全体のinteraction designやlayout algorithmの再設計。
- Issue #15で報告されていないtouch gesture、long press、context menuの追加。
- Entry、Evidence、commit、file間の関係またはExplore API payloadの変更。

## Verification

- Playwrightで関連ページを持つノードのダブルクリックを複数回実行し、詳細ページへの移動と近傍非展開を確認する。
- Playwrightでシングルクリック時に近傍が一度だけ展開され、詳細ページへ移動しないことを確認する。
- canvas非依存の通常リンクとkeyboard導線に関する既存検証を維持する。
- `cargo fmt --all -- --check`、Rust 1.87でのclippyと全target test、Playwright E2E、`belay doctor`を実行し、結果をEvidenceとして記録する。

## Risks

- 短すぎる判定窓では競合が残り、長すぎる判定窓ではシングルクリックの応答性が悪化する。
- 非同期の展開処理が判定窓の後に残ると、ページ移動直前のgraph mutationや重複展開が起きる可能性がある。
- CI環境の入力timingに依存したテストはflakyになり得るため、内部timerの経過だけでなく観測可能な遷移・graph状態を検証する必要がある。

## Completion

- PR #16 CIでRustとPlaywrightを含む全checksが成功し、EVD-20260725T134154-001がSC-004を検証する。
- EVD-20260725T220721-001が修正後の操作に対するhuman acceptanceを記録する。
- PR #16は明示承認後にsquash mergeされ、Issue #15はcloseされた。
- SC-001からSC-004はすべてverifiedであり、未検証またはblockedのDelivery Map taskは残っていない。
