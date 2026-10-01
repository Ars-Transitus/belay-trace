#!/usr/bin/env python3
"""Build Belay and optionally atomically install Cargo's reported executable."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bindir")
    args = parser.parse_args()
    executable = None
    with subprocess.Popen(
        ["cargo", "build", "--release", "--locked", "--bin", "belay",
         "--message-format=json-render-diagnostics"],
        stdout=subprocess.PIPE, text=True,
    ) as process:
        for line in process.stdout:
            message = json.loads(line)
            if message.get("reason") == "compiler-message":
                print(message["message"].get("rendered", ""), end="")
            if (message.get("reason") == "compiler-artifact"
                    and message["target"]["name"] == "belay"
                    and "bin" in message["target"]["kind"]
                    and message.get("executable")):
                executable = Path(message["executable"])
        if process.wait() != 0:
            raise SystemExit(process.returncode)
    if executable is None:
        raise SystemExit("Cargo did not report a belay executable; nothing installed")
    if args.bindir is None:
        return
    if not args.bindir:
        raise SystemExit("BINDIR must not be empty")
    destination = Path(args.bindir)
    destination.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".belay-", dir=destination)
    try:
        with os.fdopen(fd, "wb") as output, executable.open("rb") as source:
            shutil.copyfileobj(source, output)
            output.flush()
            os.fchmod(output.fileno(), 0o755)
            os.fsync(output.fileno())
        os.replace(temporary, destination / "belay")
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    print(f"Installed {destination / 'belay'}")


if __name__ == "__main__":
    main()
