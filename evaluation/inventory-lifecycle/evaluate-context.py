#!/usr/bin/env python3
"""Frozen v1 scripted retrieval evaluation; no model-behavior claims."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import tarfile
import tempfile
import time

BASE = "9d4b16e341a23ee505fad1d2315fb389b892a298"
PLAN = "PLN-20260906T134828-001-context-cost-first-phase"
GOAL = "GOAL-20260906T134724-001-context-cost-reduction"
LIVE_GOAL = "GOAL-20260925T214740-001-belay-guidance-boundarie"
DECISION = "DEC-20260725T203527-001-use-cytoscape-double-click-window-for-explore-ar"
EVIDENCE = "EVD-20260926T004249-001"


def tokens(text):
    ascii_count = sum(ord(char) < 128 for char in text)
    return (ascii_count + 3) // 4 + len(text) - ascii_count


def run(binary, root, args):
    start = time.perf_counter()
    result = subprocess.run([str(binary), *args], cwd=root, capture_output=True, text=True)
    return result, time.perf_counter() - start


def normalize(text):
    return " ".join(text.split())


def boundary_lines(path):
    lines = path.read_text().splitlines()
    active = False
    depth = 0
    result = []
    for line in lines:
        heading = re.match(r"^(#+)\s+(.+)", line)
        if heading:
            title = heading[2].lower()
            if any(word in title for word in (
                "constraints", "non-goals", "assumptions", "unknowns", "stop condition"
            )):
                active, depth = True, len(heading[1])
            elif active and len(heading[1]) <= depth:
                active = False
        elif active and line.strip():
            result.append(normalize(line.lstrip("- ")))
    return result


def cases(root):
    boundaries = []
    for id, directory in [(PLAN, "plans"), (GOAL, "goals")]:
        boundaries.extend(boundary_lines(root / ".belay/entries" / directory / (id + ".md")))
    return [
        ("live", ["context", "compile", "--format", "agent", "--budget", "2500"],
         [LIVE_GOAL, PLAN, PLAN + "#t-002", "active"],
         [["show", LIVE_GOAL], ["show", PLAN]]),
        ("resume", ["context", "compile", "--focus", PLAN + "#t-001",
                    "--format", "agent", "--budget", "2500"],
         [PLAN + "#t-001", GOAL, "SC-001", "Objective:", "Scope:", "Steps:",
          "Acceptance:", "Verification:", "EVD-20260906T201311-001", *boundaries],
         [["show", PLAN], ["show", GOAL]]),
        ("decision", ["show", DECISION],
         [DECISION, "accepted", "Rationale:", "fulfills", "supports"], []),
        ("failure", ["show", EVIDENCE],
         [EVIDENCE, "fail", "60138f1314bc93a2f50d7b73313de06eaac82016",
          "Bounded repair required", "WRK-20260925T214846-001-implement-belay-guidance"], []),
        ("goal-evidence", ["coverage"],
         [LIVE_GOAL, "decision", "test", "monitoring"], []),
    ]


def trial(binary, root, case):
    name, args, required, fallback = case
    result, elapsed = run(binary, root, args)
    outputs = [{"argv": args, "returncode": result.returncode,
                "stdout": result.stdout, "stderr": result.stderr}]
    text = result.stdout
    if result.returncode or any(normalize(x) not in normalize(text) for x in required):
        for command in fallback:
            followup, duration = run(binary, root, command)
            elapsed += duration
            outputs.append({"argv": command, "returncode": followup.returncode,
                            "stdout": followup.stdout, "stderr": followup.stderr})
            text += "\n" + followup.stdout
    missing = [x for x in required if normalize(x) not in normalize(text)]
    return {"case": name, "seconds": elapsed,
            "tokens": sum(tokens(x["stdout"] + x["stderr"]) for x in outputs),
            "retrieval_commands": len(outputs), "missing": missing, "outputs": outputs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=Path, required=True)
    parser.add_argument("--new", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--archive-id", help="Apply the selected status change identically in isolated old/new snapshots")
    options = parser.parse_args()
    source = Path(__file__).resolve().parents[2]
    binaries = {"old": options.old.resolve()}
    if options.new:
        binaries["new"] = options.new.resolve()
    results = {label: [] for label in binaries}
    with tempfile.TemporaryDirectory(prefix="belay-context-v1-", dir=os.environ.get("TMPDIR")) as temp:
        roots = {}
        for label, binary in binaries.items():
            root = Path(temp) / label
            subprocess.run(["git", "clone", "--quiet", "--no-local", "--no-checkout",
                            str(source), str(root)], check=True, capture_output=True)
            payload = subprocess.check_output(["git", "archive", BASE, ".belay"], cwd=source)
            with tarfile.open(fileobj=io.BytesIO(payload)) as archive:
                archive.extractall(root, filter="data")
            initialized, _ = run(binary, root, ["init", "--reset-state"])
            if initialized.returncode:
                raise RuntimeError(initialized.stderr)
            if options.archive_id:
                changed, _ = run(binary, root, ["status", options.archive_id, "archived"])
                if changed.returncode:
                    raise RuntimeError(changed.stderr)
            roots[label] = root
        for iteration in range(18):
            labels = list(binaries) if iteration % 2 == 0 else list(reversed(binaries))
            for label in labels:
                runs = [trial(binaries[label], roots[label], case) for case in cases(roots[label])]
                if iteration >= 3:
                    results[label].append(runs)
    report = {"schema_version": 1, "evaluation": "evaluation-v1.md", "base_commit": BASE, "selected_archive_id": options.archive_id,
              "binary_sha256": {k: hashlib.sha256(v.read_bytes()).hexdigest() for k, v in binaries.items()},
              "results": results, "summary": {}}
    for label, trials in results.items():
        report["summary"][label] = {
            "median_seconds": statistics.median(sum(x["seconds"] for x in t) for t in trials),
            "tokens": sum(x["tokens"] for x in trials[0]),
            "retrieval_commands": sum(x["retrieval_commands"] for x in trials[0]),
            "missing": {x["case"]: x["missing"] for x in trials[0] if x["missing"]},
        }
    if "new" in report["summary"]:
        old, new = report["summary"]["old"], report["summary"]["new"]
        report["comparison"] = {
            "token_reduction": 1 - new["tokens"] / old["tokens"],
            "runtime_ratio": new["median_seconds"] / old["median_seconds"],
            "search_count_ok": new["retrieval_commands"] <= old["retrieval_commands"],
            "required_information_ok": not new["missing"],
            "limitation": "Scripted retrieval only; semantic/adversarial quality checks are separate gates.",
        }
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "results"}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
