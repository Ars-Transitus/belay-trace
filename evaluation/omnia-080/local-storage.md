# Approved trial storage

Human approval: EVD-5149dedd040e9dce8e56c7ac0187c746 (this chat).

- Originals: `.belay/local-sources/`.
- Recovery copies: `.belay/local-backups/`.
- Both are excluded from Git; retain all generations during this trial.
- Same disk: recovery from accidental editing only, not disk failure protection.
- Never overwrite an existing generation. Bind each acquisition to its source
  ID, retrieval time and content digest; verify original and backup hashes before
  issuing a real-source Contract. A collision with different bytes must stop.
- Check complete exact selected source and required dependencies before issuance.
  No secrets or unrelated page crawl. A local hash proves integrity, not current
  source state or human authority.
- Git carries the confirmed execution requirements, AC, constraints, forbidden
  operations, stop conditions, source identifiers and hashes needed to reconstruct
  the delegation. Keep raw page contents and source originals out of Git.
- During Notion outage stop changes/new workers. On recovery refetch the selected
  source and check changes, cancellation, supersede and expiry before resuming.

Paths are approved; this record does not claim a real-source acquisition or
backup has already completed.
