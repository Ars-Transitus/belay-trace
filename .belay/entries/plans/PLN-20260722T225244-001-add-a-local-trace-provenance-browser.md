---
schema_version: 1
id: PLN-20260722T225244-001-add-a-local-trace-provenance-browser
type: plan
title: Add a local trace provenance browser
status: completed
created_at: 2026-07-22T22:52:44+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: supports
  id: GOAL-20260712T163956-001-maintain-intent-to-evidence-alignment-during-age
metadata: {}
---

## Intent Brief

### Problem

- Belay entryはMarkdownとCLIでは閲覧できるが、人間が文書間のtyped link、Evidence、検証時点のcommitを横断的に探索する専用UIがない。
- 全体グラフだけでは規模が増えた際に可読性が落ち、Evidenceのcommit SHAだけでは変更ファイルとの関係を誤解しやすい。
- Evidenceが指すcommitの変更ファイルは、Entryの実装ファイルや検証範囲を直接証明しない。

### Desired Outcome

- ローカル開発者がEntry本文を中心に関連文書を辿り、必要に応じてEntry → Evidence → Commit → そのcommitで変更されたFileという来歴を探索できる。
- 表示は読み取り専用で、SQLite、managed Markdown、Evidence、sync baselineを変更しない。
- Evidence欠落、commit不明、Git object欠落、SQLite/Markdown driftを推測で埋めず、明示的に表示する。

### Success Signals

- `belay browse`からLibrary、Reader、Exploreへ移動し、検索・typed link・Evidence・commit・diff・commit時点のファイル内容を辿れる。
- CLI検索とBrowse検索でFTS5/BM25およびfilterの意味が一致する。
- Reload中に元DBが変化しても、画面内で異なる世代のEntryや検索結果が混在しない。
- failed/refuted/stale/unknown Evidenceが隠されず、色以外の表現でも識別できる。
- Browse起動前後でリポジトリ状態が変化しないことを自動テストで保証する。

### Constraints

- Rust 1.87を維持する。
- localhostの開発者本人による利用に限定し、LAN公開用のhost optionを設けない。
- 外部CDNや実行時ネットワークアクセスを必要としない。
- Belay coreは決定的に動作し、LLMを呼ばない。
- SQLite/Markdown/Evidenceスキーマは変更しない。

### Non-goals

- EntryやEvidenceの編集・登録、status変更、`belay sync`の実行。
- 静的HTML export、チーム共有、認証、複数利用者対応。
- live reload、Coverage dashboard、任意repository file browser。
- commitの変更ファイルをEntryの実装ファイルまたはEvidenceの検証範囲と断定すること。

## Public Interface

```text
belay browse [--port PORT] [--open]
```

- 既定は`127.0.0.1:0`へbindし、OSが割り当てたURLだけを表示する。
- `--port`は固定port、`--open`は既定browser起動に使用する。`--host`は追加しない。
- `--open`失敗時は警告してserverを継続する。
- repository未初期化はexit 3、bind・snapshot・runtime失敗はexit 6とする。
- 人間向けdeep linkとして`/`、`/entries/{id}`、`/evidence/{id}`、`/commits/{sha}`、`/commits/{sha}/files/{opaque-id}`、`/explore?focus={node-id}`を提供する。
- `/api/*`は同梱UI専用の内部APIとし、外部互換性を保証しない。

## Implementation Design

### Snapshot and Search

- Reload時にrusqliteの`backup` featureで元DBを読み取り専用のin-memory SQLiteへcopyする。
- Entry、両方向link、Evidence、FTS5検索を同じsnapshotから提供する。
- 既存search処理をConnection単位で再利用できるように分離し、CLIとBrowseで検索意味論を一致させる。
- snapshotへ生成時刻、HEAD SHA、Markdown drift/invalid診断を記録する。
- drift時もSQLite snapshotを表示し、`belay sync`後のReloadを案内する。
- Reloadはsame-originかつprocess固有nonce付きPOSTとする。新snapshotの構築成功後だけ交換し、失敗時は直前のsnapshotを保持する。

### HTTP, Assets, and Security

- HTTP serverに`tiny_http 0.12.0`、sanitizerに`ammonia 4.1.4`、`--open`に`webbrowser 1.2.1`を使用する。
- Cytoscape.js 3.34.0のminified UMDとMIT licenseをvendorし、JS/CSSとともにbinaryへ埋め込む。
- Markdownは既存pulldown-cmarkでHTML化後、Ammoniaでallowlist sanitizeする。
- raw HTML、event handler、危険なURL、画像の自動表示を除去する。外部linkは利用者が明示的にclickした場合だけ開く。
- CSP、`X-Content-Type-Options: nosniff`、frame禁止、`Cache-Control: no-store`、Host/Origin検証を設定し、CORSとinline script/styleを許可しない。
- UI assetの取得に外部通信やCDNを使用しない。

### Git Provenance

- Git readerはsnapshot内Evidenceが参照するSHAだけを許可する。
- `unknown`、非hex、曖昧または欠落したcommitは利用不能として表示し、任意revisionを解決しない。
- shellを介さずGitを実行し、external diff、textconv、working-tree filterを無効化する。
- merge commitは第一親との差分、root commitは空treeとの差分とし、比較元SHAを表示する。
- 追加・変更・renameはcommit時点、削除は第一親側の削除前内容を表示する。
- symlinkはlink target、submoduleはgitlink SHA、binary・非UTF-8はmetadataだけを表示する。
- Gitが列挙したpathへopaque IDを割り当て、URLから任意pathを指定できないようにする。
- file一覧、blob、diff、Evidence detail、graph近傍、Git processには上限を設け、超過・timeout・truncationをUIへ明示する。無制限bufferingや黙った欠落は行わない。

### User Experience

- LibraryはFTS検索、型・状態・tag filter、最近更新されたEntryを提供する。
- Readerはmetadata、sanitize済み本文、inbound/outbound link、全verdictのEvidence card、折りたたみ式近傍summaryを表示する。
- GoalのSuccess Criteriaへ既存coverageと同じ`sc-*` anchorを付け、fragment linkから該当項目をscroll・highlightする。
- Exploreの全体graphはEntryだけから開始し、次の規則で段階展開する。
  - Entry: 直接のEntry linkとEvidence
  - Evidence: 対象Entryとcommit
  - Commit: 参照Evidenceと変更File
  - File: diffとcommit時点の内容
- edgeはentry relation、`verifies`、`captured at`、`changed`を区別する。
- graphと同じ関係を通常HTML listでも辿れるようにし、keyboard操作とcanvas非対応環境を保証する。
- snapshot時刻、HEAD、drift、Reload結果を常時確認可能にする。
- Fileは「このcommitで変更」と表記し、Entryとの直接関係や検証範囲を主張しない。

## Delivery Map

| ID | Outcome / Task | State | Verification |
| --- | --- | --- | --- |
| T-001 | `belay browse` CLI、loopback server、help/READMEを追加 | verified | CLI integration tests passed; WRK-20260722T231453-001-implement-local-trace-provenance-browser |
| T-002 | atomic in-memory SQLite snapshot、drift診断、既存search再利用を実装 | verified | snapshot/search/drift tests passed; WRK-20260722T231453-001-implement-local-trace-provenance-browser |
| T-003 | sanitize済みReader、Library、internal API、security headersを実装 | verified | Rust HTTP/XSS tests passed; readability and Delivery Map navigation inspected in Browse; WRK-20260722T231453-001-implement-local-trace-provenance-browser; WRK-20260723T192431-001-refine-browse-readability-and-trace-navigation; EVD-20260723T200110-001 |
| T-004 | Evidence限定Git reader、first-parent diff、historical blob表示を実装 | verified | isolated Git fixtures passed; WRK-20260722T231453-001-implement-local-trace-provenance-browser |
| T-005 | Cytoscape.jsによる意味別段階展開とaccessible HTML導線を実装 | verified | Goal-first graph, distinct type colors, staged expansion, Goal item links, and canvas-independent fallback were inspected or exercised by EVD-20260723T200110-001 and the later passing PR #16 Playwright Evidence EVD-20260725T134154-001 / EVD-20260725T134154-005 |
| T-006 | Rust 1.87 CI、Playwright job、dogfooding手順を統合 | verified | Rust 1.87 locked fmt, clippy, tests, rebuild, and doctor passed locally (EVD-20260723T200110-001); the previously pending Linux Playwright run passed in PR #16 under EVD-20260725T134154-002 and EVD-20260725T222128-002 |

## Completion Reconciliation

- EVD-20260723T200110-002 correctly recorded the local macOS Chromium launch limitation and the then-pending Linux CI result.
- PR #16 later ran the release-candidate Playwright suite successfully in Linux CI. EVD-20260725T134154-001、EVD-20260725T134154-002、EVD-20260725T134154-005、EVD-20260725T222128-002 resolve the pending verification without rewriting the earlier warning.
- All Delivery Map tasks are now verified. Related implementation and refinement Work entries are completed, and the Plan can be completed.

## Test and Acceptance

- CLI parsing、help、loopback bind、exit category、browser起動失敗を検証する。
- snapshot交換、Reload失敗時の旧snapshot維持、検索一致、drift警告を検証する。
- script、raw HTML、危険URL、画像、Goal fragmentのMarkdown fixtureでXSS防止を検証する。
- normal/root/merge/missing/unknown SHA、rename、delete、symlink、submodule、binary、非UTF-8、巨大diffのGit fixtureを用意する。
- Host/Origin拒否、nonce付きReload、任意SHA/path拒否、security headerをHTTP testで検証する。
- PlaywrightでLibrary検索、Reader遷移、折りたたみ、graph展開、deep link、diff、Reload成功・失敗、keyboard導線を検証する。
- `cargo fmt -- --check`、Rust 1.87での`cargo clippy --all-targets --locked -- -D warnings`、`cargo test --all-targets --locked`、`belay rebuild`、`belay doctor`を通す。
- Browse前後でtracked Markdown、SQLite、Evidence、revision、sync baselineが変化しないことを検証する。

## Assumptions and Risks

- 現在のdogfood repositoryはEvidence 0件のため、Git連携はfixtureで検証し、実運用評価には後続の実Evidence登録が必要である。
- 過去commitには現在削除済みの機密情報が残る可能性がある。内容はlocalhost限定で表示し、画面上にGit履歴を閲覧していることを明示する。
- shallow cloneや履歴整理でEvidenceのcommitが存在しない場合は、fallback推定せず利用不能として表示する。
- durable design文書との二重管理を避け、このPlanを実装追跡の正本とする。
