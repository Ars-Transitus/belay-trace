#!/usr/bin/env python3
"""Measure sequential record/rebuild cost on a fixed synthetic corpus."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import tempfile
import time


def invoke(binary, root, args):
    start = time.perf_counter()
    result = subprocess.run([str(binary), *args], cwd=root, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise RuntimeError(f"{args}: {result.returncode}: {result.stderr}")
    return result.stdout, elapsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    binary = options.binary.resolve()
    runs = []
    for iteration in range(3):
        with tempfile.TemporaryDirectory(prefix="belay-storage-v1-", dir=os.environ.get("TMPDIR")) as temp:
            root = Path(temp)
            invoke(binary, root, ["init"])
            output, _ = invoke(binary, root, ["add", "goal", "--title", "storage-benchmark"])
            goal = re.search(r"GOAL-[A-Za-z0-9-]+", output)[0]
            times = []
            ids = []
            for n in range(100):
                text, duration = invoke(binary, root, [
                    "verify", "record", "--kind", "test", "--verdict", "pass",
                    "--source", "synthetic-fixture", "--summary", f"record {n}",
                    "--commit", "unknown", "--captured-at", "2026-09-30T12:00:00Z",
                    "--verifies", goal,
                ])
                times.append(duration)
                ids.append(re.search(r"EVD-[A-Za-z0-9-]+", text)[0])
            rebuild = []
            for _ in range(15):
                _, duration = invoke(binary, root, ["rebuild"])
                rebuild.append(duration)
            files = [p for p in (root / ".belay/evidence").rglob("*") if p.is_file()]
            runs.append({"iteration": iteration, "record_seconds": times,
                         "rebuild_seconds": rebuild, "unique_ids": len(set(ids)),
                         "loose_evidence_files": len(files),
                         "loose_evidence_bytes": sum(p.stat().st_size for p in files)})
    report = {"schema_version": 1, "evaluation": "evaluation-v1.md",
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "runs": runs,
              "median_record_seconds": statistics.median(t for r in runs for t in r["record_seconds"]),
              "median_rebuild_seconds": statistics.median(t for r in runs for t in r["rebuild_seconds"])}
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "runs"}, indent=2))


if __name__ == "__main__":
    main()
