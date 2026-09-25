# Agent rule audit — 2026-09-25

## 結論

GPT-6 Astra を理由に一律に規則を外すより、機械的な契約、権限境界、
作業方法の助言を分ける。今回の完了範囲は Belay の配布スキル、Route 参照資産、
生成・任意導入・doctor の整合、およびそれらの lifecycle テストである。
Goal/Plan/coverage/status の実行時意味論と、消費者リポジトリの既存承認設定は変更しない。

Fact: 作業開始時の jj 親は `60138f13`、作業コピーは clean。
この checkout の `AGENTS.md` が要求する `.agent-safety/routing.json` と
`.agents/skills/erwin-taskflow/SKILL.md` は存在しない。
`agent-config/`、`agent-safety/`、`skills/` もこの checkout の正本ではない。
Belay が配布するスキルの正本は `src/agent.rs::SHARED_SKILL`。

Fact: ローカルの兄弟リポジトリ Erwin の `skills/erwin-taskflow/SKILL.md`
は既に委譲を任意とし、固定修正回数を診断情報として扱っている。
これは現在のローカルファイルの観測であり、公開版・承認済み配布版の確認ではない。
同正本は PR/MR と SHA に結びついたレビュー、永続 clone を採用しており、
旧 worktree/Belay-return 運用への単純なファイル差し替えはできない。

## 判断と適用範囲

| 対象 | 判断 | 今回 |
| --- | --- | --- |
| スキル説明の目的・工程の列挙 | 呼び出す条件を短く記述する | 正本を変更 |
| 固定の reconciliation 出力 | 必要な状態情報を維持し、空欄と固定書式を省く | 正本を変更 |
| 検証の反復 | Acceptance と必須チェックを満たした後は、変更・失敗・具体的懸念・必須ゲートがある場合に再実行 | 正本へ明記 |
| 常時委譲・1回だけの修正 | 期待品質、引継ぎ費用、失敗影響で選ぶ。回数だけで止めない | Erwin 提案として除外。Belay は消費者方針へ委ねる文言だけを完了 |
| 全作業への fresh review | 低リスク変更は省略候補。契約・権限・移行などには独立レビューを残す | Erwin 提案として除外。Belay は消費者方針・Goal 基準に従う |
| 全 Goal の最終人間受入 | 検証済み成果と人間受入を分ける候補。既存の受入を捏造しない | Belay 文言を完了。必要な受入だけを消費者方針・Goal 基準で記録し、推論しない |
| 7節の Intent Brief と Route 手順の常時ロード | 段階的ロードの候補。ただし lint と生成・導入契約も調べてから分割する | Belay 実装を完了。Intent Brief は必須3項目と関連項目だけ、Route は参照資産へ移動 |
| ID、1 Task/1 SC、Evidence、競合保護 | CLI と追跡可能性の契約 | 維持 |
| sandbox・外部操作の承認・秘密保護 | モデル性能とは独立した権限境界 | 維持 |

## 次の適用案

このプロジェクトの運用を更新する場合、Erwin 正本を利用する導入操作を
別途承認し、導入前に対象ファイル一覧と差分を確認する。
不足ファイルだけをコピーすると新旧の delivery authority が混ざるため避ける。

採用する運用の要件案:

- Root は範囲と検証が明確な変更を直接完了できる。委譲は並列性、専門性、
  独立性、総費用の改善が見込める場合に選ぶ。
- 同じ承認範囲での可逆的な修正と関連チェックは継続する。新しい権限、
  重大な仕様判断、範囲拡大、解消できない障害が生じたら人へ戻す。
- レビューは変更の影響先まで読む。指摘は変更による回帰と Acceptance の
  未達を対象とし、既存の無関係な問題は verdict に混ぜない。
- モデル名を全役割で Astra に置換しない。Root の選択は利用者に委ね、
  小モデルでも使う共有スキルには明確な契約と検証条件を残す。

## 評価方法

今回の受入条件は、生成・再生成テストの通過、Codex/Claude の生成内容一致、
承認・Evidence・競合規則が差分で維持されること。導入は別操作とする。

検証結果: Route の生成・再生成、Codex/Claude の任意導入・参照だけの更新、
doctor の inactive/active/stale、既存 symlink と Route 参照ディレクトリの
symlink 拒否を対象にした `cargo test --test cli --locked <filter>` は通過。
統合後の `cargo test --all-targets --locked`、
`cargo clippy --all-targets --locked -- -D warnings`、
`cargo fmt --all --check` と `git diff --check` も通過。
抽出した最終 Skill と Route 参照資産の `quick_validate.py` は通過。
独立レビューで見つかった旧文言のテスト期待値と doctor ヘルプを修正し、
fresh review は指摘なし。インストール済み設定は更新していない。
モデルによる実課題比較は未実施であり、生成テストは行動改善の証拠ではない。

Hypothesis: 固定書式と不要な再検証の削減は、品質を維持したまま出力と
ツール呼び出しを減らせる。実測の改善率は Unknown。
代表的な小修正、複数ファイルの修正、契約変更について、同一モデル・同一課題で
旧規則と候補規則を比較する。受入達成率、回帰、不要な停止、ツール回数、時間、
トークンを記録する。モデル変更と規則変更を同時に行わず、原因を切り分ける。

## 出典

- [OpenAI: Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
  — 適用条件の限定、必要時だけの文書読み込み、過度な工程指定と早期停止の見直し。
- 現行 checkout の `AGENTS.md`、`CLAUDE.md`、`.codex/config.toml`、
  `.codex/agents/{implement_high,review_high}.toml`、`src/agent.rs`。
- 兄弟 checkout の `../erwin/skills/erwin-taskflow/SKILL.md`。
