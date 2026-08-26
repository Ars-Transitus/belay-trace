---
schema_version: 1
id: NOTE-20260712T134440-001-workflow-feedback-from-first-external-adoption-m
type: note
title: Workflow feedback from first external adoption (MarketTapestry)
status: active
created_at: 2026-07-12T13:44:40+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links: []
metadata: {}
---

# 出典

2026-07-12、MarketTapestryリポジトリへの初の実戦導入時のフィードバック。
導入作業と1日分のプロトタイプ開発セッションをClaude (Fable 5) が
このワークフロー下で実施した際の所見。

# 導入時に実際に起きた問題(バグ報告)

- テンプレート展開直後の `belay doctor` がdrift 9件で失敗した。
  ローカルSQLiteにテンプレート自身の開発履歴
  (WRK-20260615T\*-package-github-rulesets等)が残っており、
  Markdown側は空。`belay rebuild` で解消したが、
  テンプレート配布物または `belay init` が state を確実に
  初期化すべき。
- 生成アセット(AGENTS snippet / Codexスキル)がstale判定。
  `make bootstrap` で解消。
- Claude用belay-traceスキルはデフォルト未生成
  (`belay init --install-skill claude` が別途必要)。
  Codexと同時生成にするか、SETUP.mdに明記すべき。

# 改善提案(重要度順)

## 1. 軽量パス(Tier分け)の明文化

数行のチューニングやdocs修正の反復(プロトタイピング局面で頻発)に
フルフロー(plan→承認→jj new→work→独立レビュー→review)は重く、
トレースが儀式化して検索ノイズになる。
AGENTS.mdにTier定義を追加する:
小変更は会話中の指示を承認とみなしコミットのみで可、
plan/decision/reviewが必要なのはアーキテクチャ・API契約・
非自明なトレードオフを含む作業、等。

## 2. 「独立レビュー」の独立性定義

デフォルトの「同一エージェントが高推論でjj diffを見直す」は、
実装と同じコンテキストウィンドウの思い込みを持ち込む。
独立性の本質はモデル差ではなくコンテキスト分離。
「レビューはフレッシュなコンテキスト(サブエージェント/新セッション)で」
を最低線とし、feature級以上はクロスモデルレビューをデフォルトに
格上げする案。

## 3. belay doctorがマージゲートにない

CIはmarkdownlint/typos/PRタイトルのみで、トレース整合性は
保護されていない。driftは「人間がMarkdown直接編集→sync忘れ」で
必ず再発する。`belay doctor` をCIジョブ化しrulesetの必須チェックに
追加すべき。「source of truth」と呼ぶならその整合性こそ保護対象。

## 4. 承認の記録がない

ゲートは明示承認を要求するが、planを approved にするのはエージェントで、
「承認されたとエージェントが主張した」記録しか残らない。
承認時に誰が・いつ・どの発言でを plan entry に追記する規約
(または `belay status --note` 的な機構)を追加する。

## 5. 小さい点

- Makefile/CIのlint対象にプロジェクト実ドキュメント(docs/\*_/_.md)が
  含まれない。テンプレート付属docsのみ検査している
- スキル3系統(.shared/.agents/.claude)が手動三重管理で、
  実際にstaleが発生した。.sharedを正典として生成/差分検証する
  makeターゲットが欲しい
- docs/ と belay entries の境界ガイドが無い。
  「恒久設計文書はdocs/、意思決定の経緯はdecisionからdocsを参照」等を
  AGENTS.mdに一言入れないと真実が二箇所に育つ

# 肯定的所見

- ゲート位置(issue/実装開始/PR/マージ)は正確。
  内側ループの自由度と外向き不可逆行為の統制のバランスが良い
- `belay context --budget` によるトークン経済への配慮は実運用で有効
- decisionのsupersedesチェーンは設計局面(次フェーズ)に適合する見込み
