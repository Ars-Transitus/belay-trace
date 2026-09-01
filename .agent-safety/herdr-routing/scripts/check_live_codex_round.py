#!/usr/bin/env python3
"""Launch one Codex worker in a real Herdr pane and check what the fixture cannot.

`test_herdr_routing.py` proves the launch command stays small, that the role's
text is not on it, and that the runner refuses a round whose policy Notes it
cannot verify. None of that says the pane's terminal actually carried the
command, that Codex came up on the routed model, or that a worker handed only
two Note IDs can reach its instructions. Those need a running Herdr server, a
logged-in Codex, a belay store, and a model call — none of which CI has — so
this lives beside the fixture and is run by hand.

It uses `implement_high` deliberately. That role's instructions, inlined as
`-c developer_instructions=...`, are what pushed the launch command past the
1024 bytes a terminal accepts in canonical mode: the command arrived cut
mid-token, the shell never ran it, and the round died as
`agent startup timeout`. If that regresses, this is where it shows.

Run it from a project that has the bundle installed and its policy Notes
created, inside a Herdr pane:

    python3 .agent-safety/herdr-routing/scripts/check_live_codex_round.py
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
RUNNER = Path(__file__).resolve().parent / "route_worker.py"
ROLE = "implement_high"
PHASE, DIFFICULTY = "implementation", "high"
# A question the worker can only answer from the policy Notes it resolved. What
# is checked is the chain, not the wording: that it ran `belay show`, and that
# the precedence rule which exists only in the common Note came back. Matching a
# model's prose any more tightly than that tests the model, not the transport.
PROBE = ("Do not start any task. Resolve your policy Notes first, then answer in one "
         "sentence: what outranks them, and what do they grant you?")


def load_runner():
    spec = importlib.util.spec_from_file_location("route_worker", RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sh(*argv: str) -> subprocess.CompletedProcess:
    return subprocess.run(list(argv), capture_output=True, text=True)


def main() -> int:
    if os.environ.get("HERDR_ENV") != "1":
        print("not inside a Herdr pane (HERDR_ENV != 1); nothing to check")
        return 2

    runner = load_runner()
    routing = runner.load_routing(ROOT)
    route = routing[PHASE][DIFFICULTY]
    definitions = runner.agent_dir(ROOT, routing["herdr"]["kinds"]["codex"]["agent_definitions"])
    definition = Path(definitions) / f"{ROLE}.toml"
    declared = tomllib.loads(definition.read_text())["developer_instructions"]

    argv = runner.build_argv("codex", ROLE, PHASE, DIFFICULTY, routing, ROOT)
    command = runner.pane_command("codex", argv)
    print(f"role definition:     {definition}")
    print(f"launch command:      {len(command.encode()) + 1} bytes "
          f"(terminal accepts {runner.PANE_LINE_LIMIT})")
    print(f"                     {command}")

    failures = []
    try:
        notes = runner.resolve_policy_notes(ROOT, routing, ROLE, str(ROOT))
    except runner.Failure as error:
        print(f"FAIL: policy Notes — {error}")
        return 1

    # What the policy would cost if it rode on the command line the way the role
    # instructions used to: the number this design exists to keep off it.
    safety = runner.safety_dir(ROOT)
    config = routing["herdr"]["kinds"]["codex"]["policy_notes"]
    policy = "\n".join(
        (safety / runner.POLICY_NOTE_BODIES / f"{name}.md").read_text()
        for name in (config["common"], config["roles"][ROLE]))
    inlined = len(runner.pane_command("codex", [
        *argv[:6], "-c", "developer_instructions=" + runner.toml_basic_string(policy),
        *argv[8:]]).encode()) + 1
    for note in notes:
        print(f"policy Note:         {note['role']} {note['id']} rev {note['revision']}")
    print(f"inlined it would be: {inlined} bytes — "
          + ("over the limit, which is the case this design avoids"
             if inlined > runner.PANE_LINE_LIMIT else "under the limit today"))

    split = json.loads(sh("herdr", "pane", "split", "--current", "--direction", "right",
                          "--cwd", str(ROOT), "--no-focus").stdout)
    pane = split["result"]["pane"]["pane_id"]
    try:
        started = time.monotonic()
        try:
            runner.start_agent_when_ready("livecheck", "codex", pane, 120000, argv)
            print(f"agent start:         interactive Codex in {time.monotonic() - started:.1f}s")
        except runner.Failure as error:
            print(f"FAIL: agent start — {error}")
            print(sh("herdr", "pane", "read", pane).stdout[-800:])
            return 1

        prompt = runner.implementer_prompt("PLN-live-check#t-000", DIFFICULTY, "none", notes)
        prompt = prompt.split("Return over belay")[0] + PROBE
        answered = sh("herdr", "agent", "prompt", "livecheck", prompt,
                      "--wait", "--until", "idle", "--until", "done",
                      "--timeout", "240000")
        if answered.returncode != 0 or not answered.stdout.strip():
            print("FAIL: agent prompt --wait — "
                  f"{((answered.stdout or '') + (answered.stderr or '')).strip()[:400]}")
            return 1
        state = json.loads(answered.stdout)["result"]["agent"]["agent_status"]
        print(f"agent prompt --wait: settled as {state}")
        transcript = sh("herdr", "agent", "read", "livecheck").stdout
        if state == "blocked":
            # Usually an approval this transport did not cause — a Codex trust
            # prompt for an unfamiliar directory is the common one — so show the
            # screen rather than reporting a transport failure it may not be.
            failures.append("the worker ended blocked on a prompt; the screen is below")
            print("--- pane ---")
            print(transcript[-700:])
            print("--- end ---")

        flat = " ".join(transcript.split())
        lowered = flat.lower()
        if route["model"] in flat and route["reasoning_effort"] in flat:
            print(f"routed as:           {route['model']} {route['reasoning_effort']}")
        else:
            failures.append(f"Codex did not report {route['model']} "
                            f"{route['reasoning_effort']} in its header")
        # It ran the command the preamble told it to run...
        if "belay show" in lowered:
            print("worker ran belay show: yes")
        else:
            failures.append("the worker never ran `belay show`, so it never reached "
                            "its policy Notes")
        # ...and came back with the precedence rule that only the Notes state.
        if "agents.md" in lowered:
            print("precedence from Note:  yes")
        else:
            failures.append("the worker did not report the precedence rule its common "
                            "policy Note states")
    finally:
        sh("herdr", "pane", "close", pane)

    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("ok: a Codex worker launched, routed as configured, and read its policy Notes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
