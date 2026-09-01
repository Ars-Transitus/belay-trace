#!/usr/bin/env python3
"""Select the worker transport before any routed launch.

This deliberately has no Herdr side effects.  Eligibility is owned by the
approved Delivery Map and explicit human opt-out is an input, not an inference.
"""

from __future__ import annotations

import argparse
import os
import sys


def boolean(value: str) -> bool:
    values = {"true": True, "false": False}
    try:
        return values[value.lower()]
    except KeyError as error:
        raise argparse.ArgumentTypeError("must be true or false") from error


def select_transport(routed_eligible: bool, human_opt_out: bool, herdr_env: str | None) -> str:
    if not routed_eligible:
        return "root/single-agent"
    if not human_opt_out and herdr_env == "1":
        return "herdr"
    return "in-process-subagent"


def main() -> int:
    parser = argparse.ArgumentParser(description="Select a deterministic routed-worker transport.")
    parser.add_argument("--routed-eligible", required=True, type=boolean)
    parser.add_argument("--human-opt-out", required=True, type=boolean)
    args = parser.parse_args()
    print(select_transport(args.routed_eligible, args.human_opt_out, os.environ.get("HERDR_ENV")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
