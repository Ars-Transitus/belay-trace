#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
binary=${1:?usage: evaluation/context-cost/run-baseline.sh /path/to/belay}
fixture="$root/evaluation/context-cost/fixtures/mixed-rich"
expected="$root/evaluation/context-cost/baseline.md"

if [ ! -x "$binary" ]; then
  echo "belay binary is not executable: $binary" >&2
  exit 2
fi
binary_dir=$(CDPATH= cd -- "$(dirname -- "$binary")" && pwd)
binary="$binary_dir/$(basename -- "$binary")"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/belay-context-cost.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
cp -R "$fixture/." "$scratch"
mkdir -p "$scratch/.git"
(
  cd "$scratch"
  "$binary" init --reset-state >/dev/null
  "$binary" context compile --focus \
    PLN-20260906T120000-001-context-packet-baseline#t-001 \
    --format agent --budget 256 > packet.md
)

bytes=$(wc -c < "$scratch/packet.md" | tr -d ' ')
tokens=$(python3 - "$scratch/packet.md" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8").read()
ascii_bytes = sum(1 for byte in text.encode("utf-8") if byte < 128)
non_ascii_scalars = sum(1 for char in text if not char.isascii())
print((ascii_bytes + 3) // 4 + non_ascii_scalars)
PY
)
sha256=$(shasum -a 256 "$scratch/packet.md" | awk '{print $1}')
binary_sha256=$(shasum -a 256 "$binary" | awk '{print $1}')

required_present=0
required_total=7
required_results=
while IFS='|' read -r name required; do
  if grep -Fq "$required" "$scratch/packet.md"; then
    required_present=$((required_present + 1))
    required_results="${required_results}${name}=present,"
  else
    required_results="${required_results}${name}=absent,"
  fi
done <<'REQUIRED'
constraints|### Constraints
non_goals|### Non-goals
assumptions|### Assumptions
unknowns|Unknowns / Decisions Needed
acceptance|Acceptance: preserve every required packet field
goal_item|SC-001
evidence|EVD-20260906T120006-001
REQUIRED
required_results=${required_results%,}

actual=$(printf 'binary_sha256=%s\nbytes=%s\ntokens=%s\nsha256=%s\nrequired_present=%s/%s\nrequired_results=%s\nadditional_retrieval_commands=0\n' \
  "$binary_sha256" "$bytes" "$tokens" "$sha256" "$required_present" "$required_total" "$required_results")
# The source revision pins compiler behavior.  A debug executable hash is
# recorded for provenance, but it is not an acceptance value: rustc embeds
# build-path information, so an equivalent rebuild in another worktree can
# legitimately have a different executable hash.
expected_values=$(awk '/^bytes=|^tokens=|^sha256=|^required_present=|^required_results=|^additional_retrieval_commands=/' "$expected")
actual_values=$(printf '%s\n' "$actual" | awk '/^bytes=|^tokens=|^sha256=|^required_present=|^required_results=|^additional_retrieval_commands=/')

printf '%s\n' "$actual"
if [ "$actual_values" != "$expected_values" ]; then
  echo "baseline mismatch; packet retained only until script exit" >&2
  exit 1
fi
