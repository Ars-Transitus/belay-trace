import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("omnia_consolidate", ROOT / "scripts" / "omnia_consolidate.py")
consolidate = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(consolidate)
CAPSULE = ROOT / "tests" / "omnia_contract" / "fixtures" / "capsule-expired-missing.json"
SHA_A = "sha256:" + "a" * 64
SHA_B = "sha256:" + "b" * 64


def fixtures():
    capsule = json.loads(CAPSULE.read_text())
    target = {"page_id": "page-001", "result_area_id": "results-001"}
    pre_observation = {"schema_version": 1, "target": target, "complete": True,
                       "snapshot_sha256": SHA_A, "occurrences": []}
    request = {
        "schema_version": 1,
        "target": target,
        "result": {
            "summary": {field: capsule[field] for field in (
                "execution", "verification", "human_acceptance", "lifecycle", "source_freshness",
            )},
            "artifact_refs": ["artifact://capsule.json"],
            "unresolved": ["Acquire current evidence"],
            "decision_candidates": ["Decide whether to extend expiry"],
        },
        "pre_observation_digest": consolidate.digest(pre_observation),
        "permission_ref": "approval://human/append-results-001",
    }
    prepared = consolidate.prepare(capsule, request)
    return capsule, request, prepared["operation"], prepared["ledger"]


def observation(operation, *, complete=True, snapshot_sha256=SHA_A, occurrences=None, target=None):
    value = {"schema_version": 1, "target": target or copy.deepcopy(operation["target"]),
             "complete": complete, "snapshot_sha256": snapshot_sha256,
             "occurrences": [] if occurrences is None else occurrences}
    value["observation_digest"] = consolidate.digest(value)
    return value


class ConsolidateTests(unittest.TestCase):
    def unknown(self):
        _, _, operation, ledger = fixtures()
        result = consolidate.begin_dispatch(operation, ledger, observation(operation))
        self.assertTrue(result["persist_required"])
        self.assertTrue(result["dispatch_allowed_after_persist"])
        self.assertEqual(result["ledger"]["state"], "unknown")
        return operation, result["ledger"]

    def test_prepare_is_deterministic_and_binds_real_capsule(self):
        capsule, request, operation, ledger = fixtures()
        self.assertEqual(consolidate.prepare(capsule, request), consolidate.prepare(capsule, request))
        self.assertEqual(operation["capsule_digest"], consolidate.digest(capsule))
        self.assertEqual(operation["permission_ref"], request["permission_ref"])
        self.assertEqual(ledger["state"], "pending")
        tampered = copy.deepcopy(capsule)
        tampered["verification"] = "verified"
        with self.assertRaisesRegex(consolidate.ConsolidateError, "capsule"):
            consolidate.prepare(tampered, request)
        false_summary = copy.deepcopy(request)
        false_summary["result"]["summary"]["execution"] = "completed"
        with self.assertRaisesRegex(consolidate.ConsolidateError, "does not bind supplied Capsule"):
            consolidate.prepare(capsule, false_summary)

    def test_precondition_change_conflicts_before_dispatch(self):
        _, _, operation, ledger = fixtures()
        result = consolidate.begin_dispatch(operation, ledger,
                                            observation(operation, snapshot_sha256=SHA_B))
        self.assertEqual(result["ledger"]["state"], "conflict")
        self.assertFalse(result["dispatch_allowed_after_persist"])

    def test_response_loss_and_partial_readback_stay_unknown(self):
        operation, ledger = self.unknown()
        result = consolidate.reconcile(operation, ledger, observation(operation, complete=False))
        self.assertEqual(result["ledger"]["state"], "unknown")
        absent = consolidate.reconcile(operation, ledger, observation(operation))
        self.assertEqual(absent["ledger"]["state"], "unknown")
        self.assertFalse(absent["dispatch_allowed_after_persist"])

    def test_exact_single_readback_applies_and_then_noops(self):
        operation, ledger = self.unknown()
        seen = observation(operation, snapshot_sha256=SHA_B, occurrences=[{
            "operation_id": operation["operation_id"], "content": copy.deepcopy(operation["content"]),
        }])
        applied = consolidate.reconcile(operation, ledger, seen)
        self.assertEqual(applied["ledger"]["state"], "applied")
        noop = consolidate.reconcile(operation, applied["ledger"], seen)
        self.assertEqual(noop["action"], "no-op")
        self.assertFalse(noop["persist_required"])

    def test_duplicate_and_mismatched_content_conflict(self):
        for occurrences in (
            None,
            [{"operation_id": "placeholder", "content": {}}],
        ):
            operation, ledger = self.unknown()
            if occurrences is None:
                item = {"operation_id": operation["operation_id"], "content": operation["content"]}
                occurrences = [copy.deepcopy(item), copy.deepcopy(item)]
            else:
                occurrences[0]["operation_id"] = operation["operation_id"]
                occurrences[0]["content"] = copy.deepcopy(operation["content"])
                occurrences[0]["content"]["result"]["summary"]["execution"] = "completed"
            result = consolidate.reconcile(operation, ledger,
                                           observation(operation, occurrences=occurrences))
            self.assertEqual(result["ledger"]["state"], "conflict")

    def test_retry_requires_decision_settlement_and_conclusive_absence(self):
        operation, ledger = self.unknown()
        decision = {"schema_version": 1, "operation_id": operation["operation_id"],
                    "decision_ref": "DEC-RETRY-001", "original_request_settled": True}
        result = consolidate.authorize_retry(operation, ledger, observation(operation), decision)
        self.assertEqual(result["ledger"]["state"], "pending")
        self.assertEqual(len(result["ledger"]["attempts"]), 2)
        self.assertEqual(result["ledger"]["attempts"][0]["settlement"]["decision"], decision)
        self.assertEqual(result["ledger"]["attempts"][0]["settlement"]["observation"],
                         observation(operation))
        with self.assertRaisesRegex(consolidate.ConsolidateError, "complete"):
            consolidate.authorize_retry(operation, ledger,
                                        observation(operation, complete=False), decision)
        unsettled = copy.deepcopy(decision); unsettled["original_request_settled"] = False
        with self.assertRaisesRegex(consolidate.ConsolidateError, "must be true"):
            consolidate.authorize_retry(operation, ledger, observation(operation), unsettled)

    def test_rejects_foreign_target_unknown_fields_and_tampering(self):
        _, _, operation, ledger = fixtures()
        foreign = {"page_id": "page-OTHER", "result_area_id": "results-001"}
        with self.assertRaisesRegex(consolidate.ConsolidateError, "foreign target"):
            consolidate.begin_dispatch(operation, ledger, observation(operation, target=foreign))
        forged = copy.deepcopy(operation); forged["permission_ref"] = "approval://forged"
        with self.assertRaisesRegex(consolidate.ConsolidateError, "canonical operation"):
            consolidate.validate_operation(forged)
        forged_ledger = copy.deepcopy(ledger); forged_ledger["state"] = "applied"
        with self.assertRaisesRegex(consolidate.ConsolidateError, "does not match latest attempt"):
            consolidate.validate_ledger(forged_ledger, operation)
        unknown = copy.deepcopy(operation); unknown["network_authorized"] = True
        with self.assertRaisesRegex(consolidate.ConsolidateError, "unknown field"):
            consolidate.validate_operation(unknown)
        raw = copy.deepcopy(operation); raw["content"]["raw_logs"] = ["secret"]
        with self.assertRaisesRegex(consolidate.ConsolidateError, "unknown field"):
            consolidate.validate_operation(raw)
        bad_digest = observation(operation)
        bad_digest["snapshot_sha256"] = SHA_B
        with self.assertRaisesRegex(consolidate.ConsolidateError, "canonical observation"):
            consolidate.validate_observation(bad_digest, operation)

    def test_attempt_loss_is_rejected_even_with_recomputed_digest(self):
        operation, ledger = self.unknown()
        decision = {"schema_version": 1, "operation_id": operation["operation_id"],
                    "decision_ref": "DEC-RETRY-001", "original_request_settled": True}
        retried = consolidate.authorize_retry(operation, ledger, observation(operation), decision)["ledger"]
        retried["attempts"][1]["number"] = 3
        retried["ledger_digest"] = consolidate._ledger_digest(retried)
        with self.assertRaisesRegex(consolidate.ConsolidateError, "attempt loss"):
            consolidate.validate_ledger(retried, operation)

    def test_prior_applied_attempt_cannot_be_disguised_as_retry_history(self):
        operation, ledger = self.unknown()
        ledger["attempts"][0]["state"] = "applied"
        ledger["attempts"][0]["settlement"] = {
            "observation": observation(operation),
            "decision": {"schema_version": 1, "operation_id": operation["operation_id"],
                         "decision_ref": "DEC-FORGED", "original_request_settled": True},
        }
        ledger["attempts"].append({"number": 2, "state": "pending"})
        ledger["state"] = "pending"
        ledger["ledger_digest"] = consolidate._ledger_digest(ledger)
        with self.assertRaisesRegex(consolidate.ConsolidateError, "only settled unknown"):
            consolidate.validate_ledger(ledger, operation)

    def test_retry_history_revalidates_bound_complete_absence(self):
        operation, ledger = self.unknown()
        decision = {"schema_version": 1, "operation_id": operation["operation_id"],
                    "decision_ref": "DEC-RETRY-001", "original_request_settled": True}
        retried = consolidate.authorize_retry(operation, ledger, observation(operation), decision)["ledger"]
        item = {"operation_id": operation["operation_id"], "content": operation["content"]}
        retried["attempts"][0]["settlement"]["observation"] = observation(
            operation, occurrences=[item])
        retried["ledger_digest"] = consolidate._ledger_digest(retried)
        with self.assertRaisesRegex(consolidate.ConsolidateError, "complete operation absence"):
            consolidate.validate_ledger(retried, operation)

    def test_begin_dispatch_detects_existing_once_and_twice(self):
        _, _, operation, ledger = fixtures()
        item = {"operation_id": operation["operation_id"], "content": operation["content"]}
        once = consolidate.begin_dispatch(operation, ledger,
                                          observation(operation, occurrences=[copy.deepcopy(item)]))
        self.assertEqual((once["action"], once["ledger"]["state"]),
                         ("already-applied", "applied"))
        self.assertFalse(once["dispatch_allowed_after_persist"])
        twice = consolidate.begin_dispatch(
            operation, ledger, observation(operation,
                                           occurrences=[copy.deepcopy(item), copy.deepcopy(item)]))
        self.assertEqual(twice["ledger"]["state"], "conflict")
        mismatched = copy.deepcopy(item)
        mismatched["content"]["result"]["summary"]["execution"] = "completed"
        mismatch = consolidate.begin_dispatch(operation, ledger,
                                              observation(operation, occurrences=[mismatched]))
        self.assertEqual(mismatch["ledger"]["state"], "conflict")

    def test_applied_state_still_detects_late_duplicate_content_drift_and_absence(self):
        operation, ledger = self.unknown()
        item = {"operation_id": operation["operation_id"], "content": operation["content"]}
        applied = consolidate.reconcile(operation, ledger,
                                        observation(operation, occurrences=[copy.deepcopy(item)]))["ledger"]
        duplicate = consolidate.reconcile(
            operation, applied, observation(operation,
                                            occurrences=[copy.deepcopy(item), copy.deepcopy(item)]))
        self.assertEqual(duplicate["ledger"]["state"], "conflict")
        drift = copy.deepcopy(item)
        drift["content"]["result"]["summary"]["execution"] = "completed"
        drifted = consolidate.reconcile(operation, applied,
                                        observation(operation, occurrences=[drift]))
        self.assertEqual(drifted["ledger"]["state"], "conflict")
        absent = consolidate.reconcile(operation, applied, observation(operation))
        self.assertEqual(absent["ledger"]["state"], "conflict")

    def test_actual_capsule_superseded_lifecycle_is_accepted_but_failed_execution_is_not(self):
        capsule, request, _, _ = fixtures()
        capsule["lifecycle"] = "superseded"
        request["result"]["summary"]["lifecycle"] = "superseded"
        self.assertEqual(consolidate.prepare(capsule, request)["operation"]["content"]
                         ["result"]["summary"]["lifecycle"], "superseded")
        request["result"]["summary"]["execution"] = "failed"
        with self.assertRaisesRegex(consolidate.ConsolidateError, "invalid value"):
            consolidate.prepare(capsule, request)

    def test_cli_stdout_only_and_help_documents_protocol(self):
        capsule, request, _, _ = fixtures()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "capsule.json").write_text(json.dumps(capsule))
            (root / "request.json").write_text(json.dumps(request))
            command = [sys.executable, str(ROOT / "scripts" / "omnia_consolidate.py")]
            result = subprocess.run(command + ["prepare", "--capsule", str(root / "capsule.json"),
                                     "--request", str(root / "request.json")], text=True,
                                    capture_output=True, check=False)
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertEqual(result.stderr, "")
            self.assertEqual(sorted(path.name for path in root.iterdir()), ["capsule.json", "request.json"])
            help_result = subprocess.run(command + ["--help"], text=True, capture_output=True,
                                         check=False)
            self.assertEqual(help_result.returncode, 0)
            self.assertIn("begin-dispatch", help_result.stdout)
            self.assertIn("authorize-retry", help_result.stdout)
            self.assertIn("snapshot_sha256", help_result.stdout)
            self.assertIn("excluding that field", help_result.stdout)


if __name__ == "__main__":
    unittest.main()
