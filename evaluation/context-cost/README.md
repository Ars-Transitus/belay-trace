# Context-cost baseline fixture

This directory freezes the first-phase comparison input for context compiler
work. It measures deterministic CLI output; it does not measure model tokens,
prompt-cache behavior, API usage, or prices.

## Fixture

`fixtures/mixed-rich/` is a committed Belay repository input. It contains:

- a primary Goal and a similarly worded duplicate Goal;
- a working-set Plan with three tasks, including the focused T-001 task;
- Japanese and English text, long constraints, explicit Unknowns, and six
  Evidence records targeting the focused task.

The measured invocation is:

```sh
belay context compile --focus PLN-20260906T120000-001-context-packet-baseline#t-001 \
  --format agent --budget 256
```

The pre-defined retrieval procedure is one invocation of the command above.
It permits zero additional retrieval commands. Required strings are checked in
the emitted packet, not inferred from the fixture source. This makes a missing
string a recorded baseline result rather than an unverified judgment.

## Run

Build the pinned source revision, then pass that exact binary to the runner:

```sh
evaluation/context-cost/run-baseline.sh /path/to/belay
```

The script copies the committed fixture to a temporary directory, rebuilds its
local SQLite state from Markdown and NDJSON, writes the packet, and compares
bytes, the `src/markdown.rs` token estimate, SHA-256, required-string presence,
and additional-command count to `baseline.md`. It reports the supplied binary's
SHA-256 for provenance, but does not compare that hash: debug builds may differ
by worktree path while producing the same pinned-revision packet.

`baseline.md` identifies the source revision and binary hash used to establish
the baseline. Re-running with a compatible binary compiled from that revision
must match the packet measurements; its binary hash is retained in the run
output for provenance. The script deliberately fails on any measurement
mismatch.
