---
schema_version: 1
id: DEC-20260730T190623-001-conversational-preview-a
type: decision
title: conversational-preview-approval
status: accepted
created_at: 2026-07-30T19:06:23+09:00
updated_at: 2026-08-18T20:38:50+09:00
revision: 1
tags: []
links:
- relation: references
  id: PLN-20260726T104808-001-validate-the-belay-route-protocol-before-product
- relation: fulfills
  id: GOAL-20260726T014020-001-enable-responsible-traceable-evolutionary-develo
metadata: {}
---

## Decision

- Routeの人間向け承認は、完全hashの転記を必須にしない。
- AIが直前に提示した単一のMaterialization Previewに対し、人間が通常言語で明示承認した場合、AIはその保留Previewのrun ID、revision、完全hashへ承認を束縛できる。
- Belay coreはapply時に完全hashを引き続き検証する。
- 一つのrunには承認待ちPreviewを一つだけ許容する。Proposal、Response、InputまたはPreviewが変われば保留承認は失効する。
- 曖昧な会話、承認対象を明示していないOK、複数承認待ち、またはPreview提示後の意味的な会話変更は承認として扱わない。

## Rationale

- hashは差し替え・stale検査の機械的根拠であり、人間に転記を求めても人間の意図確認を強めない。
- AIは既に自然言語をHuman Responseへ変換するため、対象が一意な会話上のpending stateを明示的に扱う方が実用性と監査性を両立する。
- AIチャットだけでは、ユーザー発話をCLIが独立に署名検証することはできない。これはUnknownであり、将来の構造化UIまたは署名済み選択イベントの対象とする。

## Consequences

- Human ResponseとMaterialization Previewの完全hash bindingは維持する。
- agent Skillは、Preview要約と「このPreviewへの明示OKのみ承認扱い」とを明示する。
- coreは、conversation provenanceを真実として検証しない。会話解釈はagent layerの責務である。

## Source

- Human direction: 「AIが『この内容を適用してよいですか』と普通に聞き、人が『OK』と答えたときに、安全に確定できないことが気になる」および「うん。それでいこう。」
