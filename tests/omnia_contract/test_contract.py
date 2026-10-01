import copy
import importlib.util
import io
import json
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("omnia_contract", ROOT / "scripts" / "omnia_contract.py")
omnia = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(omnia)
FIXTURES = Path(__file__).parent / "fixtures"
GOLDEN = Path(__file__).parent / "golden"


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.confirmed = json.loads((FIXTURES / "confirmed-input.json").read_text())

    def assert_invalid(self, value, text):
        with self.assertRaisesRegex(omnia.ContractError, text):
            omnia.validate_input(value)

    def test_compile_matches_golden_and_is_deterministic(self):
        first = omnia.canonical_bytes(omnia.compile_contract(self.confirmed))
        second = omnia.canonical_bytes(omnia.compile_contract(copy.deepcopy(self.confirmed)))
        self.assertEqual(first, second)
        self.assertEqual(first, (GOLDEN / "contract-v1.json").read_bytes())

    def test_preview_preserves_full_boundary_and_matches_golden(self):
        preview = omnia.projection_preview(omnia.compile_contract(self.confirmed))
        self.assertEqual(preview["plan"]["constraints"], self.confirmed["constraints"])
        self.assertEqual(preview["plan"]["non_goals"], self.confirmed["non_goals"])
        self.assertEqual(preview["plan"]["stop_conditions"], self.confirmed["stop_conditions"])
        self.assertEqual(preview["goal"]["success_criteria"][0]["success_criterion"], "SC-001")
        self.assertEqual(preview["goal"]["acceptance_to_success"][0], {
            "acceptance_criterion": "AC-001", "success_criterion": "SC-001",
            "source_refs": ["SRC-001"],
        })
        self.assertEqual(omnia.canonical_bytes(preview), (GOLDEN / "projection-preview.json").read_bytes())

    def test_rejects_unknown_missing_draft_and_unresolved(self):
        unknown = copy.deepcopy(self.confirmed); unknown["surprise"] = True
        self.assert_invalid(unknown, "unknown field")
        missing = copy.deepcopy(self.confirmed); del missing["target"]
        self.assert_invalid(missing, "missing required")
        draft = copy.deepcopy(self.confirmed); draft["status"] = "draft"
        self.assert_invalid(draft, "unresolved or draft")
        unresolved = copy.deepcopy(self.confirmed); unresolved["unknowns"] = ["Who approves?"]
        self.assert_invalid(unresolved, "unresolved or draft")

    def test_rejects_bool_integer_duplicate_ids_and_bad_timestamp(self):
        for schema_name, validator, value in (
            ("input", omnia.validate_input, copy.deepcopy(self.confirmed)),
            ("contract", omnia.validate_contract, omnia.compile_contract(self.confirmed)),
            ("capsule", omnia.validate_capsule,
             json.loads((FIXTURES / "capsule-expired-missing.json").read_text())),
        ):
            for invalid_version in (True, 1.0):
                candidate = copy.deepcopy(value); candidate["schema_version"] = invalid_version
                with self.subTest(schema=schema_name, version=invalid_version):
                    with self.assertRaisesRegex(omnia.ContractError, "schema_version"):
                        validator(candidate)
        boolean = copy.deepcopy(self.confirmed); boolean["revision"] = True
        self.assert_invalid(boolean, "expected integer")
        duplicate = copy.deepcopy(self.confirmed); duplicate["requirements"].append(copy.deepcopy(duplicate["requirements"][0]))
        self.assert_invalid(duplicate, "duplicate id")
        timestamp = copy.deepcopy(self.confirmed); timestamp["issued_at"] = "2026-10-01T00:00:00+09:00"
        self.assert_invalid(timestamp, "RFC3339 UTC")

    def test_rejects_missing_source_ref_and_incomplete_source(self):
        missing = copy.deepcopy(self.confirmed); missing["requirements"][0]["source_refs"] = ["SRC-404"]
        self.assert_invalid(missing, "missing source ref")
        incomplete = copy.deepcopy(self.confirmed); incomplete["source_bundle"]["sources"][0]["complete"] = False
        self.assert_invalid(incomplete, "incomplete source")
        bad_bundle = copy.deepcopy(self.confirmed); bad_bundle["source_bundle"]["bundle_digest"] = "sha256:" + "0" * 64
        self.assert_invalid(bad_bundle, "does not match canonical manifest")

    def test_contract_digest_binds_id_revision_and_content(self):
        contract = omnia.compile_contract(self.confirmed)
        for field, value in (("contract_id", "OMNIA-DEMO-002"), ("revision", 2), ("outcome", "changed")):
            altered = copy.deepcopy(contract); altered[field] = value
            with self.assertRaisesRegex(omnia.ContractError, "does not match"):
                omnia.validate_contract(altered)
        other_input = copy.deepcopy(self.confirmed)
        other_input["outcome"] = "A different meaning"
        other = omnia.compile_contract(other_input)
        with self.assertRaisesRegex(omnia.ContractError, "identity collision"):
            omnia.validate_identity_set([contract, other])

    def test_capsule_keeps_axes_and_negative_evidence_separate(self):
        for name in ("capsule-cancelled.json", "capsule-expired-missing.json", "capsule-failed.json"):
            capsule = json.loads((FIXTURES / name).read_text())
            self.assertIs(omnia.validate_capsule(capsule), capsule)
        false_verified = json.loads((FIXTURES / "capsule-expired-missing.json").read_text())
        false_verified["criteria"][0]["verification"] = "verified"
        with self.assertRaisesRegex(omnia.ContractError, "verified requires only passing"):
            omnia.validate_capsule(false_verified)

        false_top_level = json.loads((FIXTURES / "capsule-expired-missing.json").read_text())
        false_top_level["verification"] = "verified"
        with self.assertRaisesRegex(omnia.ContractError, "expected 'unverified'"):
            omnia.validate_capsule(false_top_level)

        understated = json.loads((FIXTURES / "capsule-expired-missing.json").read_text())
        understated["criteria"][0]["verification"] = "verified"
        understated["criteria"][0]["evidence"][0]["state"] = "passing"
        with self.assertRaisesRegex(omnia.ContractError, "expected 'verified'"):
            omnia.validate_capsule(understated)

        rejected = json.loads((FIXTURES / "capsule-failed.json").read_text())
        for mutation, error in (
            (lambda value: value.pop("human_acceptance_binding"), "required for rejected"),
            (lambda value: value["human_acceptance_binding"].__setitem__("contract_revision", 2), "does not bind current capsule"),
            (lambda value: value["human_acceptance_binding"].__setitem__("operation", "accept"), "does not bind current capsule"),
            (lambda value: value["human_acceptance_binding"].__setitem__("evidence", []), "too few items"),
        ):
            candidate = copy.deepcopy(rejected); mutation(candidate)
            with self.assertRaisesRegex(omnia.ContractError, error):
                omnia.validate_capsule(candidate)

        accepted = copy.deepcopy(rejected)
        accepted["human_acceptance"] = "accepted"
        accepted["human_acceptance_binding"]["operation"] = "accept"
        self.assertIs(omnia.validate_capsule(accepted), accepted)

    def test_evidence_state_binding_and_requirements(self):
        capsule = json.loads((FIXTURES / "capsule-failed.json").read_text())
        capsule["human_acceptance_binding"]["evidence"][0]["state"] = "passing"
        with self.assertRaisesRegex(omnia.ContractError, "current evidence set"):
            omnia.validate_capsule(capsule)
        for evidence_state, label in (("missing", "failed"), ("failed", "partial"), ("passing", "unverified")):
            capsule = json.loads((FIXTURES / "capsule-expired-missing.json").read_text())
            capsule["criteria"][0]["evidence"][0]["state"] = evidence_state
            capsule["criteria"][0]["verification"] = label
            capsule["verification"] = label
            with self.assertRaisesRegex(omnia.ContractError, "from evidence states"):
                omnia.validate_capsule(capsule)
        preview = omnia.projection_preview(omnia.compile_contract(self.confirmed))
        self.assertEqual(preview["plan"]["requirements"], self.confirmed["requirements"])

    def test_capsule_allows_exact_cross_criterion_evidence_reuse(self):
        capsule = json.loads((FIXTURES / "capsule-cancelled.json").read_text())
        capsule["criteria"].append(copy.deepcopy(capsule["criteria"][0]))
        capsule["criteria"][1]["id"] = "AC-002"
        self.assertIs(omnia.validate_capsule(capsule), capsule)
        capsule["criteria"][1]["evidence"].append(copy.deepcopy(capsule["criteria"][1]["evidence"][0]))
        with self.assertRaisesRegex(omnia.ContractError, "duplicate evidence id"):
            omnia.validate_capsule(capsule)

    def test_capsule_fractional_as_of_does_not_widen_contract_timestamps(self):
        capsule = json.loads((FIXTURES / "capsule-cancelled.json").read_text())
        capsule["evaluated_as_of"] = "2026-10-02T00:00:00.123456Z"
        self.assertIs(omnia.validate_capsule(capsule), capsule)
        confirmed = copy.deepcopy(self.confirmed)
        confirmed["issued_at"] = "2026-10-01T00:00:00.123Z"
        with self.assertRaisesRegex(omnia.ContractError, "second precision"):
            omnia.validate_input(confirmed)
        capsule = json.loads((FIXTURES / "capsule-cancelled.json").read_text())
        capsule["criteria"].append(copy.deepcopy(capsule["criteria"][0]))
        capsule["criteria"][1]["id"] = "AC-002"
        capsule["criteria"][1]["evidence"][0]["artifact_sha256"] = "sha256:" + "a" * 64
        with self.assertRaisesRegex(omnia.ContractError, "inconsistent reuse"):
            omnia.validate_capsule(capsule)

    def test_draft_shape_is_not_issuance(self):
        draft = copy.deepcopy(self.confirmed)
        draft.update(status="draft", unknowns=["Goal choice pending"])
        self.assertIs(omnia.validate_draft(draft), draft)
        with self.assertRaises(omnia.ContractError):
            omnia.compile_contract(draft)
        draft["unexpected"] = True
        with self.assertRaises(omnia.ContractError):
            omnia.validate_draft(draft)

    def test_cli_emits_machine_output_on_stdout_only(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = omnia.main(["validate", str(FIXTURES / "confirmed-input.json")])
        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(stdout.getvalue(), '{"kind":"input","valid":true}\n')
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = omnia.main(["validate"])
        self.assertEqual(result, 2)
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(json.loads(stdout.getvalue())["error_path"], None)

    def test_cli_reports_exact_structured_validation_paths(self):
        cases = []

        incomplete = copy.deepcopy(self.confirmed)
        incomplete["source_bundle"]["sources"][0]["complete"] = False
        cases.append((
            incomplete,
            "$.source_bundle.sources: incomplete source cannot be issued",
            "$.source_bundle.sources[0].complete",
        ))

        missing = copy.deepcopy(self.confirmed)
        del missing["source_bundle"]["sources"][0]["complete"]
        cases.append((
            missing,
            "$.source_bundle.sources[0]: missing required field(s): complete",
            "$.source_bundle.sources[0].complete",
        ))

        unknown = copy.deepcopy(self.confirmed)
        unknown["target"]["surprise"] = True
        cases.append((
            unknown,
            "$.target: unknown field(s): surprise",
            "$.target.surprise",
        ))

        for value, error, error_path in cases:
            stdout, stderr = io.StringIO(), io.StringIO()
            with self.subTest(error_path=error_path), \
                    mock.patch.object(omnia, "_read", return_value=value), \
                    redirect_stdout(stdout), redirect_stderr(stderr):
                result = omnia.main(["validate", "synthetic.json"])
            self.assertEqual(result, 2)
            self.assertEqual(stderr.getvalue(), "")
            self.assertEqual(json.loads(stdout.getvalue()), {
                "error": error,
                "error_path": error_path,
                "valid": False,
            })

    def test_unknown_key_paths_quote_ambiguous_json_names(self):
        for key, error_path in (
            ("child.name", '$.target["child.name"]'),
            ("child[0]", '$.target["child[0]"]'),
            ('child"name', '$.target["child\\"name"]'),
            ("child\\name", '$.target["child\\\\name"]'),
            ("", '$.target[""]'),
        ):
            value = copy.deepcopy(self.confirmed)
            value["target"][key] = True
            stdout, stderr = io.StringIO(), io.StringIO()
            with self.subTest(key=key), \
                    mock.patch.object(omnia, "_read", return_value=value), \
                    redirect_stdout(stdout), redirect_stderr(stderr):
                result = omnia.main(["validate", "synthetic.json"])
            self.assertEqual(result, 2)
            self.assertEqual(stderr.getvalue(), "")
            diagnostic = json.loads(stdout.getvalue())
            self.assertEqual(diagnostic["error"], f"$.target: unknown field(s): {key}")
            self.assertEqual(diagnostic["error_path"], error_path)


if __name__ == "__main__":
    unittest.main()
