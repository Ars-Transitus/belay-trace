# ID and reference standard

Belay の Goal、Plan、Decision、Work、Note、Review、Evidence を、文書内の
Success Criterion と Delivery Map task まで含めて一貫して参照するための規格。

## 1. ID のスコープ

ID は二層に分ける。

| 対象 | 形式 | スコープ | 例 |
| --- | --- | --- | --- |
| Entry | `<TYPE>-<timestamp>-<sequence>-<slug>` | repository | `GOAL-20260723T120000-001-safe-sync` |
| Evidence | `EVD-<timestamp>-<sequence>` | repository | `EVD-20260723T120500-001` |
| Success Criterion | `SC-NNN` | defining Goal | `SC-001` |
| Delivery task | `T-NNN` | defining Plan | `T-001` |

Entry type prefix は `GOAL`、`PLN`、`DEC`、`WRK`、`REV`、`NOTE` とする。
timestamp は `created_at` または `captured_at` の時刻を compact form にしたもので、
同一秒内 sequence とともに Belay が採番する。

`SC-NNN` と `T-NNN` はそれぞれ `001` から始め、定義文書内で単調増加させる。
並べ替え、状態変更、文言変更では再採番しない。削除・統合した ID は欠番のまま
残し、別の項目へ再利用しない。並行編集で同じ番号が作られた場合は、マージ時に
新しく追加された側だけを未使用番号へ変更する。

`SC-001` や `T-001` 単体は文書ローカルであり、repository 全体では一意ではない。
文書の外から参照するときは、次の完全修飾参照を使う。

```text
GOAL-20260723T120000-001-safe-sync#sc-001
PLN-20260723T120100-001-deliver-safe-sync#t-001
```

fragment は小文字を canonical form とする。表示ラベルでは大文字の
`SC-001` / `T-001` を使ってよい。

## 2. 定義形式

Goal の `Success Criteria` は、各トップレベル項目の先頭で ID を定義する。

```markdown
## Success Criteria

- [SC-001] 双方変更を検出できる。
- [SC-002] 競合時に既存データを上書きしない。
```

Plan の `Delivery Map` は `ID` 列で task ID を定義し、`Goal item` 列では
対象 Goal の criterion を指定する。

```markdown
## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001 | 双方変更を検出する | AI | implemented | EVD pending |
| T-002 | SC-002 | 非上書きを検証する | AI | verified | EVD-20260723T120500-001 |
```

Plan が複数 Goal を扱う場合、`Goal item` に完全修飾参照を書く。Goal への
リンクが一つだけなら、同じ Plan 内では `SC-001` の短縮形を使用できる。

### 2.1 task の本文セクション

Delivery Map の行は task を**定義**するが、内容は持てない。task の詳細は同じ
Plan 内の `## T-NNN` セクションに書く。行が索引と状態、セクションが実行者の
読む本体である。

```markdown
## T-001

- **Objective**: 双方変更を検出する。
- **Scope**: in — sync の比較経路。out — 競合解決の方針。
- **Steps**: ...
- **Acceptance**: SC-001 が満たされる。
- **Verification**: `cargo test`
```

見出しは fragment を**定義しない**。`#t-001` の定義元はあくまで Delivery Map
の `ID` 列であり、`plan_fragments` はその列だけを読む。見出しが定義元を兼ねると
同じ ID が二箇所で定義され、§4 の一意性検査が壊れる。

必須フィールドは Objective、Scope、Steps、Acceptance、Verification の5つ。
これは「前提を共有していない読み手が着手できる」ための最小集合である。利用側は
Difficulty や Owner などを追加してよく、`belay plan lint` は belay が要求しない
フィールドを finding にしない。

## 3. 文書ごとの参照規格

| 文書 | 定義する ID | 必須の構造化リンク |
| --- | --- | --- |
| Goal | Entry ID、`SC-NNN` | 前提・根拠への `references` / `supports` |
| Plan | Entry ID、`T-NNN` | 対象 Goal/criterion への `fulfills` |
| Decision | Entry ID | 対象 Goal/criterion または Plan/task への `fulfills` / `supports` |
| Work | Entry ID | 対象 Goal/criterion への `fulfills`、Plan/task への `implements` |
| Note | Entry ID | 言及対象への `references`。単なる本文記載だけで関係を表さない |
| Review | Entry ID | 対象への `reviews`、指摘が支える対象への `supports` |
| Evidence | Evidence ID | 検証対象への `verifies` または `refutes` |

### Work の Task 起点作成

Plan Task に対応する実装 Work は、次の一体型コマンドで作成できる。

```sh
belay work create \
  --task PLN-20260723T120100-001-deliver-safe-sync#t-001 \
  --title "Implement safe sync" \
  --body "Implementation details." \
  --format id
```

`--task` は完全修飾された `PLN-...#t-nnn` で指定する。Belay は Delivery
Map の `Goal item` と Plan の `fulfills` リンクから Goal criterion を導出し、
Work に `implements Plan#task` と `fulfills Goal#sc-nnn` の両方を保存する。
Plan が複数 Goal を扱う場合、`Goal item` は完全修飾された
`GOAL-...#sc-nnn` でなければならない。Task または Goal の解決が欠ける、
無効、非 canonical、または曖昧な場合は Work を作成しない。

`--format id` は Work ID だけを、`--format json` は
`{ "id": "WRK-..." }` だけを標準出力へ出す。これにより、人間向けの
`Created WRK-...` をシェルで解析する必要がない。

Entry の関係は frontmatter の `links` に、Evidence の関係は append-only record の
`links` に保存する。本文に完全修飾 ID を書くことは可読性のために推奨するが、
本文の文字列だけを provenance graph の edge とはみなさない。
Browse Reader は本文中の完全修飾 Entry/fragment と Evidence ID をリンク表示する。
inline code と code block 内の文字列はリンク化しない。

例:

```yaml
links:
  - relation: implements
    id: PLN-20260723T120100-001-deliver-safe-sync#t-001
  - relation: fulfills
    id: GOAL-20260723T120000-001-safe-sync#sc-001
```

```sh
belay verify record \
  --kind test \
  --verdict pass \
  --source "cargo test" \
  --summary "conflict cases passed" \
  --verifies GOAL-20260723T120000-001-safe-sync#sc-001 \
  --verifies PLN-20260723T120100-001-deliver-safe-sync#t-002
```

## 4. 整合性

Belay は新しい構造化リンクまたは Evidence target を保存するとき、Entry の存在に
加えて fragment の存在も検査する。存在しない `#sc-NNN` / `#t-NNN` は拒否する。
重複定義されて一意に解決できない fragment も拒否する。定義後に直接 Markdown を
編集して参照先を削除・重複させた場合は、`belay doctor` が drift として報告する。

Goal lint は Success Criterion の ID 欠落と重複を報告する。旧来の Goal hash
fragment、`#sc-1`、Plan の `#task-t-1` は解決しない。既存文書にこれらの参照が
残っている場合は、`#sc-NNN` / `#t-NNN` へ変換してから `belay rebuild` を実行する。

Plan lint は Delivery Map の構造を報告する。task ID の形式と重複に加えて、
`State` の値、`Goal item` 列の有無、そして §2.1 の本文セクションの有無と
必須フィールドを検査する。Delivery Map を持たない Plan は skip であり、
finding ではない。

fragment が Delivery Map の行に解決しても対応する本文セクションが無い場合、
それは fragment の解決失敗ではない。`belay show` はその欠落を明示して返し、
`plan lint` が finding として報告する。

## 5. 取得

fragment は参照だけでなく取得にも使う。

```sh
belay show PLN-20260723T120100-001-deliver-safe-sync#t-001
belay show GOAL-20260723T120000-001-safe-sync#sc-001
belay show EVD-20260723T120500-001
```

fragment を付けた `show` は、定義行（Delivery Map の行、または Success
Criterion の項目）と、対応する本文セクションだけを返す。Plan 全体を読まずに
一つの task を取得するための形式であり、task 数に比例して差が開く。

canonical でない、存在しない、一意に解決しない fragment は、entry 全体の出力へ
fallback せずエラーになる。要求していないものを黙って返さないためである。

Evidence は fragment を持たない。`belay show EVD-...` は exact ID または一意
prefix だけを `.belay/evidence/*.ndjson` から解決する。slug は使えない。
SQLite 未indexでも durable record を表示できる。Herdr などの runner は自身の
provenance を使い、`show EVD` を settlement の必須ゲートにしない。Task
settlement と Goal coverage は別判断である。

section の切り出しには `entry_chunks` を用いる。検索や context 生成と同じ
分割規則であり、Markdown を別途解析し直すことはない。

## 6. 並行書き込み

複数のプロセスが同時に書いてよい。`add`、`link`、`status` はいずれも SQLite の
IMMEDIATE トランザクションを開き、**そのロックを保持したまま** managed Markdown
を書き、最後に commit する。したがって writer は interleave せず serialize する。
display ID の採番もトランザクション内で行われるため衝突しない。

保証の範囲:

- 同時実行しても entry は失われず、ID は重複せず、SQLite と Markdown ミラーは
  一致する。8 プロセスの同時 `add`、および同一 entry に対する 4 本の `link` と
  1 本の `status` の競合で検証済み（`cargo test concurrent_`）。
- ロック待ちは SQLite の busy timeout 5 秒。待ち時間がこれを超えると、破損では
  なくエラーとして返る。極端に多い writer を同時に走らせる場合はこの上限に
  注意する。
- 保証されないのは、mirror 書き込みと commit の間でプロセスが強制終了した場合
  である。この場合ミラーに DB が知らないファイルが残る。次の `belay sync` が
  それを取り込むので復旧はするが、中断の窓が存在すること自体は事実である。

この節はテストが示した範囲のみを述べている。範囲外の主張はしない。

ID は同一性を表し、内容の正しさや関係の意味を保証しない。たとえば Evidence が
`SC-001` を target に持つだけでは、そのテストが criterion を十分に検証したことに
ならない。意味的妥当性は review の対象であり、freshness と verdict は別途評価する。
