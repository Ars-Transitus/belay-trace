#!/usr/bin/env python3
"""Deterministic, read-only compiler for synthetic Omnia contracts.

The helper deliberately has no clock, network, Belay, or repository write path.
It is an OB-02/03 boundary prototype, not the complete Omnia evaluator.
Capsule validation establishes structural consistency only; it cannot establish
that a claimed human issuer is authentic or has authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

COMPILER_VERSION = "omnia-contract-compiler/1"
ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "tests" / "omnia_contract" / "schemas"


class ContractError(ValueError):
    def __init__(self, message: str, error_path: str | None = None) -> None:
        super().__init__(message)
        self.error_path = error_path


class StdoutArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise ContractError(f"arguments: {message}")


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True,
                       separators=(",", ":"), allow_nan=False) + "\n").encode("utf-8")


def _is_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _property_path(path: str, key: str) -> str:
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
        return f"{path}.{key}"
    return f"{path}[{json.dumps(key, ensure_ascii=False)}]"


def _timestamp(value: str, path: str) -> datetime:
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        raise ContractError(f"{path}: must be an RFC3339 UTC timestamp with second precision", path)
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise ContractError(f"{path}: invalid timestamp", path) from exc


def _capsule_timestamp(value: Any, path: str) -> datetime:
    if not isinstance(value, str) or not re.fullmatch(
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z", value):
        raise ContractError(f"{path}: must be an RFC3339 UTC timestamp", path)
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise ContractError(f"{path}: invalid timestamp", path) from exc


def _validate(value: Any, schema: dict[str, Any], path: str = "$") -> None:
    if "const" in schema and (type(value) is not type(schema["const"]) or
                              value != schema["const"]):
        raise ContractError(f"{path}: expected {schema['const']!r}", path)
    if "enum" in schema and value not in schema["enum"]:
        raise ContractError(f"{path}: invalid value {value!r}", path)
    kind = schema.get("type")
    valid = {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": _is_integer(value),
        "boolean": isinstance(value, bool),
    }.get(kind, True)
    if not valid:
        raise ContractError(f"{path}: expected {kind}", path)
    if kind == "object":
        properties = schema.get("properties", {})
        missing = [key for key in schema.get("required", []) if key not in value]
        if missing:
            raise ContractError(
                f"{path}: missing required field(s): {', '.join(missing)}",
                _property_path(path, missing[0]),
            )
        if schema.get("additionalProperties") is False:
            unknown = sorted(set(value) - set(properties))
            if unknown:
                raise ContractError(
                    f"{path}: unknown field(s): {', '.join(unknown)}",
                    _property_path(path, unknown[0]),
                )
        for key, child in value.items():
            if key in properties:
                _validate(child, properties[key], _property_path(path, key))
    elif kind == "array":
        if len(value) < schema.get("minItems", 0):
            raise ContractError(f"{path}: too few items", path)
        if schema.get("uniqueItems"):
            seen: set[bytes] = set()
            for index, child in enumerate(value):
                encoded = canonical_bytes(child)
                if encoded in seen:
                    raise ContractError(f"{path}: duplicate item", f"{path}[{index}]")
                seen.add(encoded)
        for index, child in enumerate(value):
            _validate(child, schema.get("items", {}), f"{path}[{index}]")
    elif kind == "string":
        if len(value) < schema.get("minLength", 0):
            raise ContractError(f"{path}: string is too short", path)
        if "pattern" in schema and not re.fullmatch(schema["pattern"], value):
            raise ContractError(f"{path}: invalid format", path)
        if schema.get("format") == "date-time":
            _timestamp(value, path)
    elif kind == "integer" and value < schema.get("minimum", value):
        raise ContractError(f"{path}: below minimum", path)


def load_schema(name: str) -> dict[str, Any]:
    with (SCHEMA_DIR / f"{name}.schema.json").open(encoding="utf-8") as stream:
        root = json.load(stream)
    return _expand_refs(root, root)


def _expand_refs(node: Any, root: dict[str, Any]) -> Any:
    if isinstance(node, list):
        return [_expand_refs(child, root) for child in node]
    if not isinstance(node, dict):
        return node
    if "$ref" in node:
        ref = node["$ref"]
        filename, _, fragment = ref.partition("#")
        document = root
        if filename:
            with (SCHEMA_DIR / filename).open(encoding="utf-8") as stream:
                document = json.load(stream)
        target: Any = document
        for component in filter(None, fragment.split("/")):
            target = target[component.replace("~1", "/").replace("~0", "~")]
        return _expand_refs(target, document)
    return {key: _expand_refs(value, root) for key, value in node.items()}


def _unique_ids(items: list[dict[str, Any]], path: str) -> None:
    ids = [item["id"] for item in items]
    duplicates = sorted({item_id for item_id in ids if ids.count(item_id) > 1})
    if duplicates:
        raise ContractError(f"{path}: duplicate id(s): {', '.join(duplicates)}", path)


def validate_draft(value: Any) -> dict[str, Any]:
    """Shape only; never establishes issuability, completeness or confirmation."""
    _validate(value, load_schema("confirmed-input"))
    if value["status"] != "draft":
        raise ContractError("$.status: draft validation requires draft status", "$.status")
    return value


def validate_input(value: Any) -> dict[str, Any]:
    _validate(value, load_schema("confirmed-input"))
    if value["status"] != "confirmed" or value["unknowns"]:
        path = "$.status" if value["status"] != "confirmed" else "$.unknowns"
        raise ContractError("$: unresolved or draft input cannot be issued", path)
    if value["compiler_version"] != COMPILER_VERSION:
        raise ContractError("$.compiler_version: unsupported compiler version", "$.compiler_version")
    if "expires_at" in value and _timestamp(value["expires_at"], "$.expires_at") <= _timestamp(value["issued_at"], "$.issued_at"):
        raise ContractError("$.expires_at: must be later than issued_at", "$.expires_at")
    sources = value["source_bundle"]["sources"]
    _unique_ids(sources, "$.source_bundle.sources")
    source_ids = {source["id"] for source in sources}
    for index, source in enumerate(sources):
        if not source["complete"]:
            raise ContractError(
                "$.source_bundle.sources: incomplete source cannot be issued",
                f"$.source_bundle.sources[{index}].complete",
            )
    bundle_payload = {key: child for key, child in value["source_bundle"].items()
                      if key != "bundle_digest"}
    bundle_digest = "sha256:" + hashlib.sha256(canonical_bytes(bundle_payload)).hexdigest()
    if value["source_bundle"]["bundle_digest"] != bundle_digest:
        raise ContractError("$.source_bundle.bundle_digest: does not match canonical manifest",
                            "$.source_bundle.bundle_digest")
    if set(value["context_refs"]) - source_ids:
        raise ContractError("$.context_refs: missing source ref", "$.context_refs")
    for group in ("requirements", "acceptance_criteria"):
        _unique_ids(value[group], f"$.{group}")
        for index, item in enumerate(value[group]):
            missing = sorted(set(item["source_refs"]) - source_ids)
            if missing:
                path = f"$.{group}[{index}].source_refs"
                raise ContractError(f"{path}: missing source ref(s): {', '.join(missing)}", path)
    return value


def _without_digest(contract: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in contract.items() if key != "contract_digest"}


def compile_contract(value: Any) -> dict[str, Any]:
    confirmed = validate_input(value)
    contract = {"schema_version": 1, **{key: confirmed[key] for key in confirmed if key != "status"}}
    contract["lifecycle"] = "active"
    digest = hashlib.sha256(canonical_bytes(contract)).hexdigest()
    contract["contract_digest"] = f"sha256:{digest}"
    _validate(contract, load_schema("contract-v1"))
    return contract


def validate_contract(value: Any) -> dict[str, Any]:
    _validate(value, load_schema("contract-v1"))
    confirmed_shape = {key: child for key, child in value.items()
                       if key not in ("contract_digest", "lifecycle")}
    confirmed_shape["status"] = "confirmed"
    validate_input(confirmed_shape)
    actual = "sha256:" + hashlib.sha256(canonical_bytes(_without_digest(value))).hexdigest()
    if value["contract_digest"] != actual:
        raise ContractError("$.contract_digest: does not match canonical payload", "$.contract_digest")
    return value


def validate_capsule(value: Any) -> dict[str, Any]:
    evaluated = _capsule_timestamp(
        value.get("evaluated_as_of") if isinstance(value, dict) else None,
        "$.evaluated_as_of",
    )
    schema_value = dict(value)
    schema_value["evaluated_as_of"] = evaluated.replace(microsecond=0).isoformat().replace("+00:00", "Z")
    _validate(schema_value, load_schema("capsule-v1"))
    mappings = value["criteria"]
    _unique_ids(mappings, "$.criteria")
    evidence_identity: dict[str, tuple[str, str]] = {}
    for index, criterion in enumerate(mappings):
        evidence_ids: set[str] = set()
        for evidence in criterion["evidence"]:
            if evidence["id"] in evidence_ids:
                path = f"$.criteria[{index}].evidence"
                raise ContractError(f"{path}: duplicate evidence id {evidence['id']}", path)
            evidence_ids.add(evidence["id"])
            identity = (evidence["artifact_sha256"], evidence["state"])
            previous = evidence_identity.setdefault(evidence["id"], identity)
            if previous != identity:
                raise ContractError(
                    f"$.criteria[{index}].evidence: inconsistent reuse of evidence id {evidence['id']}",
                    f"$.criteria[{index}].evidence",
                )
        states = {evidence["state"] for evidence in criterion["evidence"]}
        if criterion["verification"] == "verified" and (not states or states != {"passing"}):
            path = f"$.criteria[{index}].verification"
            raise ContractError(f"$.criteria[{index}]: verified requires only passing evidence", path)
        expected = ("failed" if "failed" in states else
                    "verified" if states == {"passing"} else
                    "partial" if "passing" in states else "unverified")
        if criterion["verification"] != expected:
            path = f"$.criteria[{index}].verification"
            raise ContractError(f"$.criteria[{index}]: expected {expected} from evidence states", path)
    criterion_states = {criterion["verification"] for criterion in mappings}
    if "failed" in criterion_states:
        aggregate_verification = "failed"
    elif criterion_states == {"verified"}:
        aggregate_verification = "verified"
    elif criterion_states == {"unverified"}:
        aggregate_verification = "unverified"
    else:
        aggregate_verification = "partial"
    if value["verification"] != aggregate_verification:
        raise ContractError(
            f"$.verification: expected {aggregate_verification!r} from criterion states",
            "$.verification",
        )

    acceptance = value["human_acceptance"]
    binding = value.get("human_acceptance_binding")
    if acceptance in {"accepted", "rejected"}:
        if binding is None:
            raise ContractError(f"$.human_acceptance_binding: required for {acceptance}",
                                "$.human_acceptance_binding")
        expected_operation = {"accepted": "accept", "rejected": "reject"}[acceptance]
        for field, expected in (
            ("contract_id", value["contract_id"]),
            ("contract_revision", value["contract_revision"]),
            ("contract_digest", value["contract_digest"]),
            ("operation", expected_operation),
        ):
            if binding[field] != expected:
                path = f"$.human_acceptance_binding.{field}"
                raise ContractError(f"{path}: does not bind current capsule", path)
        expected_evidence = sorted(
            (evidence["id"], evidence["artifact_sha256"], evidence["state"])
            for criterion in mappings for evidence in criterion["evidence"]
        )
        bound_evidence = sorted(
            (evidence["id"], evidence["artifact_sha256"], evidence["state"])
            for evidence in binding["evidence"]
        )
        if bound_evidence != expected_evidence:
            raise ContractError("$.human_acceptance_binding.evidence: does not bind current evidence set",
                                "$.human_acceptance_binding.evidence")
    elif binding is not None:
        raise ContractError("$.human_acceptance_binding: pending acceptance cannot carry a binding",
                            "$.human_acceptance_binding")
    return value


def validate_identity_set(contracts: list[Any]) -> None:
    identities: dict[tuple[str, int], str] = {}
    for contract in contracts:
        validated = validate_contract(contract)
        identity = (validated["contract_id"], validated["revision"])
        previous = identities.setdefault(identity, validated["contract_digest"])
        if previous != validated["contract_digest"]:
            raise ContractError(f"contract identity collision: {identity[0]} revision {identity[1]}",
                                "$.contract_digest")


def projection_preview(contract: Any) -> dict[str, Any]:
    contract = validate_contract(contract)
    provenance = {
        "contract_id": contract["contract_id"],
        "contract_revision": contract["revision"],
        "contract_digest": contract["contract_digest"],
        "source_bundle_digest": contract["source_bundle"]["bundle_digest"],
    }
    criteria = [{"success_criterion": f"SC-{index:03d}", "text": ac["text"],
                 "source_refs": ac["source_refs"]}
                for index, ac in enumerate(contract["acceptance_criteria"], start=1)]
    criterion_map = [
        {"acceptance_criterion": ac["id"],
         "success_criterion": criterion["success_criterion"],
         "source_refs": ac["source_refs"]}
        for ac, criterion in zip(contract["acceptance_criteria"], criteria)
    ]
    preview = {
        "schema_version": 1,
        "operation": "preview-only",
        "goal": {"title": contract["outcome"], "success_criteria": criteria,
                 "acceptance_to_success": criterion_map,
                 "provenance": provenance},
        "plan": {"requirements": contract["requirements"],
                 "assumptions": contract["assumptions"],
                 "context_refs": contract["context_refs"],
                 "constraints": contract["constraints"],
                 "non_goals": contract["non_goals"],
                 "delegation": contract["delegation"],
                 "verification": contract["verification"],
                 "stop_conditions": contract["stop_conditions"],
                 "target": contract["target"], "provenance": provenance},
    }
    preview["projection_digest"] = "sha256:" + hashlib.sha256(canonical_bytes(preview)).hexdigest()
    return preview


def _read(path: str) -> Any:
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def main(argv: list[str] | None = None) -> int:
    parser = StdoutArgumentParser(add_help=False)
    parser.add_argument("command", choices=("validate", "compile", "preview"))
    parser.add_argument("path")
    parser.add_argument("--kind", choices=("input", "draft", "contract", "capsule"), default="input")
    try:
        args = parser.parse_args(argv)
        value = _read(args.path)
        if args.command == "compile":
            result = compile_contract(value)
        elif args.command == "preview":
            result = projection_preview(value)
        else:
            validators = {"input": validate_input, "draft": validate_draft, "contract": validate_contract,
                          "capsule": validate_capsule}
            validators[args.kind](value)
            result = {"kind": args.kind, "valid": True}
            if args.kind == "draft":
                result.update(validation_scope="shape-only", issuable=False)
        sys.stdout.write(canonical_bytes(result).decode("utf-8"))
        return 0
    except (ContractError, OSError, json.JSONDecodeError) as exc:
        error_path = exc.error_path if isinstance(exc, ContractError) else None
        sys.stdout.write(canonical_bytes({"error": str(exc), "error_path": error_path,
                                          "valid": False}).decode("utf-8"))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
