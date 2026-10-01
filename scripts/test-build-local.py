#!/usr/bin/env python3
"""Exercise deployment failures without compiling Rust or touching user binaries."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class DeploymentTests(unittest.TestCase):
    def test_build_and_atomic_deployment(self):
        repository = Path(__file__).resolve().parent.parent
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            shutil.copy(repository / "Makefile", root)
            shutil.copy(repository / "scripts/build-local.py", root / "scripts")
            # These files must not suppress the PHONY prerequisites.
            (root / "build").touch()
            (root / "deploy").touch()
            source = root / "custom target/belay"
            source.parent.mkdir()
            source.write_bytes(b"new binary")
            fake = root / "tools"
            fake.mkdir()
            cargo = fake / "cargo"
            message = json.dumps({"reason": "compiler-artifact", "target": {
                "name": "belay", "kind": ["bin"]}, "executable": str(source)})
            cargo.write_text("#!/bin/sh\nprintf '%s\\n' '" + message + "'\n")
            cargo.chmod(0o755)
            env = dict(os.environ, PATH=str(fake) + os.pathsep + os.environ["PATH"])
            destination = root / "bin with spaces"

            def run(target="deploy"):
                return subprocess.run(
                    ["make", "-j", target, f"BINDIR={destination}"], cwd=root,
                    env=env, capture_output=True, text=True,
                )

            self.assertEqual(run("build").returncode, 0)
            self.assertFalse(destination.exists())
            for _ in range(2):
                result = run()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((destination / "belay").read_bytes(), b"new binary")
                self.assertEqual((destination / "belay").stat().st_mode & 0o777, 0o755)
            victim = root / "symlink referent"
            victim.write_bytes(b"untouched")
            (destination / "belay").unlink()
            (destination / "belay").symlink_to(victim)
            self.assertEqual(run().returncode, 0)
            self.assertFalse((destination / "belay").is_symlink())
            self.assertEqual(victim.read_bytes(), b"untouched")
            # Source-copy failure must retain the previously installed executable.
            source.unlink()
            self.assertNotEqual(run().returncode, 0)
            self.assertEqual((destination / "belay").read_bytes(), b"new binary")
            self.assertEqual(list(destination.glob(".belay-*")), [])
            cargo.write_text("#!/bin/sh\nexit 17\n")
            self.assertNotEqual(run().returncode, 0)
            self.assertEqual((destination / "belay").read_bytes(), b"new binary")


if __name__ == "__main__":
    unittest.main()
