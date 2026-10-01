# OB04 independent review — pre-fix

Verdict FAIL. HEAD df48ef171f1c3f8e602546607af6029037b9097a plus scoped
six-file manifest b4a3213e494d0e2c3eb9d7841353082d0878ce82e935867079c89a16198c7f0f.

- Receipt identity/schema/outcome tampering accepted by retry/show.
- Existing original/intent/projection/receipt symlinks bypass managed NOFOLLOW.
- Schema3 blocks newly opened legacy writers only; explicit quiescence is required
  before migration because already-open old writers do not recheck config under lock.

Contract8, Python9, Route5 passed but did not cover these findings. Fresh fixer
assigned exact paths/conditions. Live root remains schema2 and released writer.
D3 editable/protected projection reconciliation is a separate OB05 dependency;
current whole-file drift rejection is not usable after permitted progress edits.
Source freshness input is caller assertion, not authenticated connector evidence.
