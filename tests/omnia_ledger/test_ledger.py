import copy
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("omnia_ledger", ROOT / "scripts" / "omnia_ledger.py")
ledger = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(ledger)

CONTRACT = ROOT / "tests" / "omnia_contract" / "golden" / "contract-v1.json"
REPOSITORY = "github.com/Ars-Transitus/belay-trace"


def projection(kind, target_id):
    content = f"---\nid: {target_id}\ntype: {kind}\n---\n"
    return {
        "id": target_id,
        "relative_path": f"entries/{kind}s/{target_id}.md",
        "sha256": "sha256:" + hashlib.sha256(content.encode()).hexdigest(),
        "content": content,
    }


def artifacts():
    contract = json.loads(CONTRACT.read_text())
    goal_id = "GOAL-20261001T000000-001-ledger-fixture"
    plan_id = "PLN-20261001T000000-001-ledger-fixture"
    preview = {
        "schema_version": 1,
        "operation": "contract-apply",
        "repository": REPOSITORY,
        "base_commit": contract["target"]["base_commit"],
        "contract_id": contract["contract_id"],
        "contract_revision": contract["revision"],
        "contract_digest": contract["contract_digest"],
        "goal": projection("goal", goal_id),
        "plan": projection("plan", plan_id),
        "preview_digest": "",
    }
    preview["preview_digest"] = ledger.digest(preview)
    receipt = {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "contract_revision": contract["revision"],
        "contract_digest": contract["contract_digest"],
        "preview_digest": preview["preview_digest"],
        "goal_id": goal_id,
        "plan_id": plan_id,
        "outcome": "applied",
    }
    return contract, preview, receipt


class LedgerTests(unittest.TestCase):
    def record(self):
        contract, preview, receipt = artifacts()
        return ledger.derive_record(contract, receipt, preview, REPOSITORY)

    def test_missing_ledger_rebuilds_deterministically_and_retry_is_noop(self):
        record = self.record()
        rebuilt = ledger.reconcile(record)
        self.assertEqual(rebuilt["action"], "rebuild")
        self.assertTrue(rebuilt["changed"])
        self.assertEqual(rebuilt, ledger.reconcile(self.record()))
        retry = ledger.reconcile(record, rebuilt["ledger"])
        self.assertEqual((retry["action"], retry["changed"]), ("no-op", False))
        self.assertEqual(retry["ledger"], rebuilt["ledger"])

    def test_receipt_targets_are_checked_against_verified_preview(self):
        contract, preview, receipt = artifacts()
        receipt["goal_id"] = "GOAL-20261001T000000-002-attacker-alias"
        with self.assertRaisesRegex(ledger.LedgerError, "target IDs"):
            ledger.derive_record(contract, receipt, preview, REPOSITORY)

    def test_only_durable_applied_show_receipt_is_accepted(self):
        contract, preview, receipt = artifacts()
        for outcome in ("recovered", "unchanged"):
            candidate = copy.deepcopy(receipt)
            candidate["outcome"] = outcome
            with self.subTest(outcome=outcome):
                with self.assertRaisesRegex(ledger.LedgerError, "durable applied"):
                    ledger.derive_record(contract, candidate, preview, REPOSITORY)

    def test_rejects_missing_bad_or_aliased_receipt_fields(self):
        contract, preview, receipt = artifacts()
        for mutate, message in (
            (lambda value: value.pop("preview_digest"), "missing required"),
            (lambda value: value.__setitem__("contract_revision", True), "expected integer"),
            (lambda value: value.__setitem__("target_id", value["goal_id"]), "unknown field"),
            (lambda value: value.__setitem__("contract_digest", "sha256:" + "0" * 64), "does not bind"),
        ):
            candidate = copy.deepcopy(receipt)
            mutate(candidate)
            with self.subTest(message=message):
                with self.assertRaisesRegex(ledger.LedgerError, message):
                    ledger.derive_record(contract, candidate, preview, REPOSITORY)

    def test_rejects_tampered_preview_and_repository_mismatch(self):
        contract, preview, receipt = artifacts()
        bad = copy.deepcopy(preview)
        bad["goal"]["id"] = "GOAL-20261001T000000-999-tampered"
        with self.assertRaisesRegex(ledger.LedgerError, "preview_digest"):
            ledger.derive_record(contract, receipt, bad, REPOSITORY)
        with self.assertRaisesRegex(ledger.LedgerError, "expected repository"):
            ledger.derive_record(contract, receipt, preview, "github.com/example/other")

    def test_expected_repository_config_is_strict_and_independent(self):
        self.assertEqual(
            ledger.validate_config({"schema_version": 1, "repository": REPOSITORY})["repository"],
            REPOSITORY,
        )
        with self.assertRaisesRegex(ledger.LedgerError, "unknown field"):
            ledger.validate_config({
                "schema_version": 1, "repository": REPOSITORY, "repo": REPOSITORY,
            })

    def test_stale_ledger_appends_new_revision_and_then_retries_noop(self):
        record = self.record()
        old = copy.deepcopy(record)
        old["contract_revision"] = 1
        old["contract_id"] = "OMNIA-OLDER-001"
        existing = {"schema_version": 1, "repository": REPOSITORY,
                    "records": [old], "external": {}}
        result = ledger.reconcile(record, existing)
        self.assertEqual(result["action"], "append")
        self.assertEqual(len(result["ledger"]["records"]), 2)
        self.assertEqual(ledger.reconcile(record, result["ledger"])["action"], "no-op")

    def test_same_contract_id_revision_with_different_content_conflicts(self):
        record = self.record()
        conflicting = copy.deepcopy(record)
        conflicting["contract_digest"] = "sha256:" + "f" * 64
        existing = {"schema_version": 1, "repository": REPOSITORY,
                    "records": [conflicting], "external": {}}
        with self.assertRaisesRegex(ledger.LedgerError, "identity conflict"):
            ledger.reconcile(record, existing)

    def test_external_ids_and_urls_are_preserved_byte_for_byte_in_proposal(self):
        record = self.record()
        existing = {
            "schema_version": 1,
            "repository": REPOSITORY,
            "records": [],
            "external": {
                "SRC-001": {"Id": "notion-page-123", "URL": "https://example.invalid/Page?X=1"},
                "custom": ["opaque", {"Id": 7}],
            },
        }
        before = copy.deepcopy(existing["external"])
        result = ledger.reconcile(record, existing)
        self.assertEqual(result["ledger"]["external"], before)
        self.assertEqual(existing["external"], before)

    def test_cli_requires_exact_preview_and_only_prints_proposal(self):
        contract, preview, receipt = artifacts()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, value in (("contract", contract), ("preview", preview), ("receipt", receipt)):
                (root / f"{name}.json").write_text(json.dumps(value))
            (root / "config.json").write_text(json.dumps({
                "schema_version": 1, "repository": REPOSITORY,
            }))
            command = [sys.executable, str(ROOT / "scripts" / "omnia_ledger.py"), "reconcile",
                       "--contract", str(root / "contract.json"),
                       "--receipt", str(root / "receipt.json"),
                       "--config", str(root / "config.json")]
            missing = subprocess.run(command, text=True, capture_output=True, check=False)
            self.assertEqual(missing.returncode, 2)
            self.assertIn("--preview", missing.stdout)
            ok = subprocess.run(command + ["--preview", str(root / "preview.json")],
                                text=True, capture_output=True, check=False)
            self.assertEqual(ok.returncode, 0, ok.stdout)
            self.assertEqual(json.loads(ok.stdout)["action"], "rebuild")
            self.assertEqual(sorted(path.name for path in root.iterdir()),
                             ["config.json", "contract.json", "preview.json", "receipt.json"])

    def test_actual_core_preview_apply_show_yields_stable_ledger_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            scratch = Path(directory)
            repository = scratch / "repository"
            target = scratch / "cargo-target"
            repository.mkdir()

            def run(command, cwd=repository, env=None):
                result = subprocess.run(command, cwd=cwd, env=env, text=True,
                                        capture_output=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr or result.stdout)
                return result.stdout.strip()

            run(["git", "init", "-q"])
            run(["git", "config", "user.email", "ledger-test@example.invalid"])
            run(["git", "config", "user.name", "Ledger Test"])
            run(["git", "remote", "add", "origin",
                 "https://github.com/Ars-Transitus/belay-trace.git"])
            run(["git", "commit", "--allow-empty", "-qm", "fixture"])
            head = run(["git", "rev-parse", "HEAD"])

            contract = json.loads(CONTRACT.read_text())
            contract["target"]["base_commit"] = head
            unsigned = copy.deepcopy(contract)
            del unsigned["contract_digest"]
            contract["contract_digest"] = "sha256:" + hashlib.sha256(
                ledger.canonical_bytes(unsigned)
            ).hexdigest()
            contract_path = scratch / "contract.json"
            contract_path.write_bytes(ledger.canonical_bytes(contract))

            build_env = os.environ.copy()
            build_env["CARGO_TARGET_DIR"] = str(target)
            run(["cargo", "build", "--quiet", "--bin", "belay"], cwd=ROOT, env=build_env)
            belay = target / "debug" / "belay"
            run([str(belay), "init"])
            preview = json.loads(run([
                str(belay), "contract", "preview", "--file", str(contract_path),
                "--source-freshness", "observed-current",
            ]))
            applied = json.loads(run([
                str(belay), "contract", "apply", "--file", str(contract_path),
                "--source-freshness", "observed-current", "--approve",
                preview["preview_digest"], "--legacy-writers-quiesced",
            ]))
            shown = json.loads(run([
                str(belay), "contract", "show", contract["contract_id"],
                "--revision", str(contract["revision"]),
            ]))

            self.assertEqual(applied["outcome"], "applied")
            self.assertEqual(shown, applied)
            from_apply = ledger.derive_record(contract, applied, preview, REPOSITORY)
            from_show = ledger.derive_record(contract, shown, preview, REPOSITORY)
            self.assertEqual(from_show, from_apply)
            self.assertEqual(ledger.reconcile(from_show, ledger.reconcile(from_apply)["ledger"])["action"],
                             "no-op")

            unchanged = json.loads(run([
                str(belay), "contract", "apply", "--file", str(contract_path),
                "--source-freshness", "observed-current", "--approve",
                preview["preview_digest"],
            ]))
            self.assertEqual(unchanged["outcome"], "unchanged")
            with self.assertRaisesRegex(ledger.LedgerError, "durable applied"):
                ledger.derive_record(contract, unchanged, preview, REPOSITORY)


if __name__ == "__main__":
    unittest.main()
