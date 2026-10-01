import copy
import importlib.util
import io
import json
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader
    spec.loader.exec_module(module)
    return module


source = load_module("omnia_source", ROOT / "scripts" / "omnia_source.py")
contract = load_module("omnia_contract_for_source", ROOT / "scripts" / "omnia_contract.py")
FIXTURES = Path(__file__).parent / "fixtures"


class SourceBundleTests(unittest.TestCase):
    def setUp(self):
        self.envelope = json.loads((FIXTURES / "valid-envelope.json").read_text())
        self.config = json.loads((FIXTURES / "expected-config.json").read_text())

    def test_compile_is_deterministic_redacted_and_contract_compatible(self):
        first = source.compile_bundle(self.envelope, self.config)
        second = source.compile_bundle(copy.deepcopy(self.envelope), copy.deepcopy(self.config))
        self.assertEqual(source.canonical_bytes(first), source.canonical_bytes(second))
        serialized = source.canonical_bytes(first)
        self.assertNotIn(b'"raw_snapshot":', serialized)
        self.assertNotIn(b"Synthetic outcome", serialized)
        self.assertTrue(first["completeness_report"]["complete"])
        self.assertEqual(len(first["source_bundle"]["sources"]), 2)

        confirmed = json.loads((ROOT / "tests/omnia_contract/fixtures/confirmed-input.json").read_text())
        confirmed["source_bundle"] = first["source_bundle"]
        confirmed["requirements"][0]["source_refs"] = ["SRC-ROOT"]
        confirmed["acceptance_criteria"][0]["source_refs"] = ["SRC-DEPENDENCY"]
        confirmed["context_refs"] = ["SRC-ROOT"]
        self.assertIs(contract.validate_input(confirmed), confirmed)

    def test_ntn_requires_explicit_matching_connector(self):
        candidate = copy.deepcopy(self.envelope)
        candidate["config"]["connector"] = "ntn"
        for item in candidate["sources"]:
            item["boundary"]["connector"] = "ntn"
        with self.assertRaisesRegex(source.SourceError, "expected configuration"):
            source.compile_bundle(candidate, self.config)
        expected = {**self.config, "connector": "ntn"}
        self.assertTrue(source.compile_bundle(candidate, expected)["completeness_report"]["complete"])

    def test_hashes_bind_raw_and_normalized_snapshots_separately(self):
        original = source.compile_bundle(self.envelope, self.config)["source_bundle"]["sources"][0]
        raw_changed = copy.deepcopy(self.envelope)
        raw_changed["sources"][0]["raw_snapshot"]["extra"] = "raw only"
        raw = source.compile_bundle(raw_changed, self.config)["source_bundle"]["sources"][0]
        self.assertNotEqual(original["raw_sha256"], raw["raw_sha256"])
        self.assertEqual(original["normalized_sha256"], raw["normalized_sha256"])

        normalized_changed = copy.deepcopy(self.envelope)
        normalized_changed["sources"][0]["normalized_snapshot"]["text"] = "changed"
        normalized = source.compile_bundle(normalized_changed, self.config)["source_bundle"]["sources"][0]
        self.assertEqual(original["raw_sha256"], normalized["raw_sha256"])
        self.assertNotEqual(original["normalized_sha256"], normalized["normalized_sha256"])

    def test_rejects_unknown_and_missing_completeness_fields(self):
        unknown = copy.deepcopy(self.envelope)
        unknown["sources"][0]["completeness"]["probably_complete"] = True
        with self.assertRaisesRegex(source.SourceError, "unknown field"):
            source.compile_bundle(unknown, self.config)
        for field in ("truncated", "pagination_complete", "permission_denied", "relation_cycle"):
            candidate = copy.deepcopy(self.envelope)
            del candidate["sources"][0]["completeness"][field]
            with self.subTest(field=field), self.assertRaisesRegex(source.SourceError, "missing required"):
                source.compile_bundle(candidate, self.config)

    def test_fails_closed_for_each_incomplete_condition(self):
        cases = (
            ("truncated", True, "truncated"),
            ("permission_denied", True, "permission-denied"),
            ("deleted", True, "deleted"),
            ("archived", True, "archived"),
            ("relation_cycle", True, "relation-cycle"),
        )
        for field, value, reason in cases:
            candidate = copy.deepcopy(self.envelope)
            candidate["sources"][0]["completeness"][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(source.SourceError, reason):
                source.compile_bundle(candidate, self.config)
        unknown = copy.deepcopy(self.envelope)
        unknown["sources"][0]["completeness"].update(
            unknown_block_count=1, unknown_block_ids=["block-unknown"])
        with self.assertRaisesRegex(source.SourceError, "unknown-blocks"):
            source.compile_bundle(unknown, self.config)
        paginated = copy.deepcopy(self.envelope)
        paginated["sources"][0]["completeness"].update(
            pagination_complete=False, continuation="cursor-synthetic")
        with self.assertRaisesRegex(source.SourceError, "pagination-incomplete"):
            source.compile_bundle(paginated, self.config)

    def test_rejects_missing_dependencies_extras_and_boundary_mismatch(self):
        missing = copy.deepcopy(self.envelope)
        missing["sources"].pop()
        with self.assertRaisesRegex(source.SourceError, "missing required source"):
            source.compile_bundle(missing, self.config)
        extra = copy.deepcopy(self.envelope)
        extra["sources"][1]["required_dependency_ids"] = []
        extra["sources"][0]["required_dependency_ids"] = []
        with self.assertRaisesRegex(source.SourceError, "unselected source"):
            source.compile_bundle(extra, self.config)
        mismatch = copy.deepcopy(self.envelope)
        mismatch["sources"][0]["boundary"]["repository"] = "github.com/example/other"
        with self.assertRaisesRegex(source.SourceError, "source boundary mismatch"):
            source.compile_bundle(mismatch, self.config)
        ref_mismatch = copy.deepcopy(self.envelope)
        ref_mismatch["sources"][0]["boundary"]["source_id"] = "SRC-WRONG"
        with self.assertRaisesRegex(source.SourceError, "source boundary mismatch"):
            source.compile_bundle(ref_mismatch, self.config)
        page_mismatch = copy.deepcopy(self.envelope)
        page_mismatch["sources"][0]["raw_snapshot"]["id"] = "PAGE-WRONG"
        with self.assertRaisesRegex(source.SourceError, "page identity mismatch"):
            source.compile_bundle(page_mismatch, self.config)

    def test_no_blind_cycle_and_pagination_evidence_consistency(self):
        cycle = copy.deepcopy(self.envelope)
        cycle["sources"][1]["required_dependency_ids"] = ["SRC-ROOT"]
        with self.assertRaisesRegex(source.SourceError, "relation cycle detected"):
            source.compile_bundle(cycle, self.config)
        contradictory = copy.deepcopy(self.envelope)
        contradictory["sources"][0]["completeness"]["continuation"] = "cursor"
        with self.assertRaisesRegex(source.SourceError, "complete pagination"):
            source.compile_bundle(contradictory, self.config)
        no_evidence = copy.deepcopy(self.envelope)
        no_evidence["sources"][0]["completeness"]["pagination_complete"] = False
        with self.assertRaisesRegex(source.SourceError, "requires continuation evidence"):
            source.compile_bundle(no_evidence, self.config)

    def test_rejects_coherent_wrong_boundary_against_expected_config(self):
        for field, wrong in (
            ("area_id", "AREA-WRONG"),
            ("data_source_id", "DS-WRONG"),
            ("repository", "github.com/example/wrong"),
            ("connector", "other-mcp"),
        ):
            candidate = copy.deepcopy(self.envelope)
            candidate["config"][field] = wrong
            for item in candidate["sources"]:
                item["boundary"][field] = wrong
            with self.subTest(field=field), self.assertRaisesRegex(
                    source.SourceError, "expected configuration|unsupported connector"):
                source.compile_bundle(candidate, self.config)

        before_sources = copy.deepcopy(self.envelope)
        before_sources["config"]["repository"] = "github.com/example/wrong"
        before_sources["sources"] = "not-an-array"
        with self.assertRaisesRegex(source.SourceError, r"\$\.config\.repository"):
            source.compile_bundle(before_sources, self.config)

    def test_acquisition_digest_binds_config_roots_and_dependency_edges(self):
        original = source.compile_bundle(self.envelope, self.config)
        manifest = original["completeness_report"]["acquisition_manifest"]
        self.assertEqual(
            original["source_bundle"]["acquisition_digest"],
            source._digest(manifest),
        )

        changed_root = copy.deepcopy(self.envelope)
        changed_root["selected_source_ids"] = ["SRC-DEPENDENCY"]
        changed_root["sources"][0]["required_dependency_ids"] = []
        changed_root["sources"][1]["required_dependency_ids"] = ["SRC-ROOT"]
        root_result = source.compile_bundle(changed_root, self.config)
        self.assertNotEqual(
            original["source_bundle"]["acquisition_digest"],
            root_result["source_bundle"]["acquisition_digest"],
        )
        self.assertNotEqual(
            original["source_bundle"]["bundle_digest"],
            root_result["source_bundle"]["bundle_digest"],
        )

        changed_edges = copy.deepcopy(self.envelope)
        changed_edges["selected_source_ids"] = ["SRC-ROOT", "SRC-DEPENDENCY"]
        changed_edges["sources"][0]["required_dependency_ids"] = []
        edge_result = source.compile_bundle(changed_edges, self.config)
        self.assertNotEqual(
            original["source_bundle"]["acquisition_digest"],
            edge_result["source_bundle"]["acquisition_digest"],
        )
        self.assertNotEqual(
            original["source_bundle"]["bundle_digest"],
            edge_result["source_bundle"]["bundle_digest"],
        )

    def test_cli_uses_stdout_only_and_does_not_write(self):
        before = sorted(path.relative_to(FIXTURES) for path in FIXTURES.rglob("*"))
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = source.main([
                "compile", str(FIXTURES / "valid-envelope.json"),
                "--config", str(FIXTURES / "expected-config.json"),
            ])
        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        self.assertTrue(json.loads(stdout.getvalue())["source_bundle"])
        after = sorted(path.relative_to(FIXTURES) for path in FIXTURES.rglob("*"))
        self.assertEqual(before, after)


if __name__ == "__main__":
    unittest.main()
