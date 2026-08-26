---
schema_version: 1
id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
type: plan
title: Implement the CLI-first Route MVP
status: approved
created_at: 2026-07-26T10:48:08+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: GOAL-20260726T103644-001-make-belay-route-decision-state-legible-and-trus
- relation: references
  id: REV-20260726T101157-001-review-belay-route-goal-draft
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Intent Brief

### Problem

- Belayは開発の事実、判断、履歴、Evidenceを保持できるが、人間がそれらを現在の判断へ再構成する認知負荷は残る。
- AIとの作業は複数thread間でinterleaveし、会話履歴だけでは現在の主題、前提、未決定、次の判断へ即座に戻れない。
- 自由文のAI出力と人間の短い応答だけでは、どのsnapshotとProposalを何の範囲で承認したかを機械的に確定できない。
- 固定フォーマットは比較可能性とprovenanceを与える一方、誤った分析を整然と提示するstructured wrongnessを生む可能性がある。

### Desired Outcome

- Belayの型付きtraceをversioned Route Inputへ変換し、外部AIによる意味的推論を、決定的にvalidation可能なAssessment、Proposal、Human Response、Materialization Preview、Reconciliation Resultとして扱える最小protocolを定義する。
- primary threadを指定してCLI runを開始し、中断後に再開し、人間が承認した範囲だけをBelayへ安全にmaterializeできるMVPを実装する。

### Success Signals

- Route protocolの各documentがschema validationを通り、Belay参照、分類、provenance、承認範囲を機械的に検査できる。
- `.belay/state/route/<run-id>/`から同一端末でrunを再開できる。
- stale、未承認、proposal mismatch、missing referenceをmaterialization前に拒否できる。
- Previewと実際のBelay変更をReconciliation Resultで照合できる。
- 6 documentを通るCLI end-to-end testがpassing Evidenceを残す。

### Constraints

- Tier 3としてGoal/Plan承認と実装開始を別Human Gateにする。
- Belay coreと既存CLIはLLMを呼ばず、Route reasoningは外部AIが担当する。
- Route protocolはBelayの公開された出力を入力とし、SQLiteやmanaged Markdownの内部構造へ直接依存しない。
- 決定的なrun管理、validation、preview、materialization、reconciliationはproduction `belay route` CLIとして実装する。
- Routeの推論結果は暫定情報であり、BelayのSource of Truthと競合させない。
- run bundleは`.belay/state/route/`に保存し、review artifactまたはGit共有対象にしない。
- Browse対応はGOAL-20260726T103644-001の後続作業とし、本Planに含めない。

### Non-goals

- モデル進化後の価値またはbaselineへの優位性を評価すること。
- Routeを独立サービスまたは法人向け製品として実装すること。
- 複数モデル、組織、権限、課金、ホスティングへ一般化すること。
- Product Map、Requirement、Route ProposalをBelayのfirst-class persistence modelへ追加すること。
- repository-wide task manager、Browse UI、MCP、承認UI、自動Issue、自動PR、自動実装を追加すること。
- 将来の高性能モデルに合わせたHuman Gate緩和条件を決めること。

### Assumptions

- **This is a hypothesis.** versionedな入出力契約と承認境界は、通常の自由対話よりblocking条件の検出と判断履歴の再構築を一貫させる。
- **This is a hypothesis.** `.belay/state/route/`のlocal resumeとBelay成果物からの再生成で、MVPに必要な中断耐性を満たせる。
- **This is a hypothesis.** 既存の`belay context compile`または公開read outputを基礎に、内部storageへ依存しないRoute Inputを構成できる。

### Unknowns / Decisions Needed

- 6 documentを個別fileにするか、一つのrun manifestとappend-only eventで保持するか。
- Route Inputへ必要な最小Belay read contract。
- materializationのatomicityとidempotency contract。
- `revise`を直接materialize可能な応答とするか、新Proposal revisionを要求するか。

## Scope and Approach

### 1. Fix the Route run boundary

- 1 runは一つの明示primary seedを持ち、必要な関連threadだけをsnapshotへ含める。
- repository-wideな委任、優先順位、WIP、通知、統合順序は後続toolへ分離する。
- run stateは`.belay/state/route/<run-id>/`に置き、正式なBelay entryと区別する。

### 2. Define stable protocol envelopes

- Route Input
- Route Assessment
- Route Proposal
- Human Response
- Materialization Preview
- Reconciliation Result

各documentはschema version、run ID、Belay reference、input fingerprint、artifact revision/hash、Fact、Human Observation、Assumption、Hypothesis、Unknown、Conflict、Proposal、provenance、limitations、authority stateを必要な範囲で型付けする。`stop`、`insufficient-context`、`no-safe-route`を正規の結果として扱う。

### 3. Generate and validate Route Input

- `belay route start --seed <display-id>`相当で、公開read contractからsnapshotとfingerprintを生成する。
- resume時と後続artifact取込時に、schema、reference、revision、fingerprint、stalenessを検査する。
- run bundle消失時は同じseedから新runを生成できるが、外部AI outputの同一性は保証しない。

### 4. Keep semantic reasoning external

- Agent Skillまたは外部agentがRoute Inputを読み、AssessmentとProposalを構造化してrunへ渡す。
- Belayは意味内容の正しさを判定せず、型、出典、参照、authority stateを決定的に検証する。
- AssessmentとProposalはadvisoryであり、単独では正式状態を変更しない。

### 5. Bind human response to an exact proposal

- 人間はAIチャットでProposalを選択、修正、却下または保留する。
- AIは応答をHuman Responseへ変換し、Belayは対象input fingerprint、proposal revision/hash、action、承認範囲を検証する。
- `revise`のauthority contractは実装前に確定する。

### 6. Preview, materialize, and reconcile

- Materialization Previewは予定するentry、link、status操作を副作用なしで列挙する。MVPのEvidenceはGoal Coverageとしてread-onlyで参照し、新規Evidence記録は既存の`belay verify` workflowに残す。
- materialization直前にもstale検査を行い、明示承認されたoperationだけを適用する。
- Reconciliation Resultは予定、成功、失敗、未適用、deferred、現在のBelay stateを照合し、部分失敗から安全に再開できる情報を残す。

## Delivery Map

| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |
| --- | --- | --- | --- | --- | --- |
| T-001 | SC-001, SC-006 | Route run lifecycle、artifact ownership、authority state、failure stateをdurable designとして確定する | AI + Human | verified | approved Goal/Plan; `docs/design/route.md`; EVD-20260729T215529-001 |
| T-002 | SC-001, SC-003, SC-004 | 6種類のversioned document schema、fingerprint、revision/hash、provenanceを実装する | AI | verified | `src/route.rs`; EVD-20260729T223748-001 |
| T-003 | SC-002, SC-005 | primary seedからRoute Inputを生成し、`.belay/state/route/`へatomicに保存・resumeする | AI | verified | start/status/template CLI tests; EVD-20260729T223748-001 |
| T-004 | SC-001, SC-002, SC-007 | 外部AIのAssessment/Proposal取込とschema、reference、stale validationを実装する | AI | verified | submit and stale tests; EVD-20260729T223748-001 |
| T-005 | SC-003, SC-007 | Human Response取込とproposal bindingを実装し、未承認・mismatch・staleを拒否する | AI | verified | exact hash/authority tests; EVD-20260729T223748-001 |
| T-006 | SC-003, SC-004 | 副作用のないMaterialization Previewと明示承認済みoperationのmaterializationを実装する | AI | verified | preview/apply CLI test; EVD-20260729T223748-001 |
| T-007 | SC-004, SC-005, SC-007 | Reconciliation Result、partial failure、idempotent resumeを実装する | AI | verified | focused recovery tests; EVD-20260730T061625-001 |
| T-008 | SC-001..SC-007 | CLI end-to-end、docs、Skill guidance、独立review、Evidenceを完成させる | AI + independent reviewer | verified | EVD-20260730T061625-001; REV-20260729T230114-001 approved |
| T-009 | SC-003, SC-004 | 保留Previewを一意に識別し、完全hashを内部検証したまま通常言語の承認を受けるCLI/contractを実装する | AI | verified | `route pending` binding contract; EVD-20260730T192220-001; REV-20260730T203035-001 |
| T-010 | SC-003, SC-007 | agent Skillへ単一pending-preview、明示OK、失効・再提示の会話規約を追加する | AI | verified | generated/installed Skill一致; `belay doctor`; REV-20260730T203035-001 |
| T-011 | SC-003, SC-004, SC-007 | 正常OK、曖昧OK、複数待機、Preview置換、stale Inputを含むend-to-end dogfoodを実施する | AI + Human | in-progress | automated replacement/freshness coverage passed; human操作性確認と明示受入れが未実施 |
| T-012 | SC-004, SC-007 | pre-Route schema DBで`belay rebuild`がreceipt table不在を理由に失敗しないよう互換性を修正する | AI | verified | Copilot PR #21 review thread; EVD-20260730T203912-001; legacy-schema rebuild test |

## Human Gates

- Gate 1: 本Planと改訂Goalの承認。承認しても実装は開始しない。
- Gate 2: 実装開始の明示承認。`jj new`、Work作成、source変更はこの後に行う。
- Gate 3: Materialization Previewで示された正式なBelay変更の承認。Route機能は既存repositoryのHuman Gateを代替しない。
- Gate 4: 実装結果の受入れ、Pull Request作成、mergeはそれぞれ既存ルールに従う。

## Validation Strategy

- Goal lintとPlan構造のfocused review
- contract sampleのschema validation
- invalid classification、missing provenance、unknown reference、unsupported version、unapproved materialization、stale inputのnegative tests
- pause/resume、run bundle消失後のInput再生成、partial failure recovery
- previewとmaterialized Belay stateの一致
- Belay coreがLLM、API key、network accessへ依存しないことのdiff review
- `belay sync`、`belay doctor`、`belay coverage`、`jj st`、`jj diff`

## Risks and Responses

- schema bloat: required fieldを最小化し、追加情報をoptional blockへ分離する。
- ceremonial Human Response: exact fingerprintとproposal revisionへ結び付かない応答を承認として扱わない。
- structured wrongness: Fact、Assumption、Hypothesis、Unknownを分離し、sourceなしFactをvalidatorで拒否する。
- TOCTOU: preview後、materialization直前にfingerprintを再検査する。
- partial write: apply前にmanifestを`applying`へ遷移し、各mutationと同一SQLite transactionでoperation receiptを記録する。

## Review Requirements

- 実装者とは別のfresh-context reviewerがGoal、Plan、contract、diff、validation結果をレビューする。
- security、privacy、外部送信が発生する実装へ変わる場合は、Planを更新し`requires_human_review: true`の専門reviewを追加する。

requires_human_review: true
