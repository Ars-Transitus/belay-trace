#!/usr/bin/env python3
"""Read-only Omnia identity-ledger reconciliation.

This helper never writes a ledger or an external system.  A receipt JSON is not
proof of authenticity: callers must supply the exact output of both
``belay contract preview`` and ``belay contract show`` (or artifacts verified
equivalently by the core command).  The helper only checks their deterministic
structural and digest bindings and prints a proposed ledger/reconciliation.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_SPEC = importlib.util.spec_from_file_location(
    "omnia_contract_for_ledger", ROOT / "scripts" / "omnia_contract.py"
)
if CONTRACT_SPEC is None or CONTRACT_SPEC.loader is None:  # pragma: no cover
    raise RuntimeError("cannot load omnia_contract validator")
omnia_contract = importlib.util.module_from_spec(CONTRACT_SPEC)
CONTRACT_SPEC.loader.exec_module(omnia_contract)

SHA256 = re.compile(r"sha256:[0-9a-f]{64}")
CONTRACT_ID = re.compile(r"[A-Z][A-Z0-9-]{2,63}")
ENTRY_ID = re.compile(r"(?:GOAL|PLN)-[A-Za-z0-9-]{3,123}")


class LedgerError(ValueError):
    pass


class StdoutArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise LedgerError(f"arguments: {message}")


def canonical_bytes(value: Any) -> bytes:
    return omnia_contract.canonical_bytes(value)


def digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def _object(value: Any, path: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise LedgerError(f"{path}: expected object")
    missing = sorted(keys - set(value))
    unknown = sorted(set(value) - keys)
    if missing:
        raise LedgerError(f"{path}: missing required field(s): {', '.join(missing)}")
    if unknown:
        raise LedgerError(f"{path}: unknown field(s): {', '.join(unknown)}")
    return value


def _integer(value: Any, path: str, minimum: int = 1) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise LedgerError(f"{path}: expected integer >= {minimum}")
    return value


def _string(value: Any, path: str, pattern: re.Pattern[str] | None = None) -> str:
    if not isinstance(value, str) or not value or (pattern and not pattern.fullmatch(value)):
        raise LedgerError(f"{path}: invalid string")
    return value


def _projection(value: Any, path: str) -> dict[str, Any]:
    projection = _object(value, path, {"id", "relative_path", "sha256", "content"})
    _string(projection["id"], f"{path}.id", ENTRY_ID)
    _string(projection["relative_path"], f"{path}.relative_path")
    expected_prefix = "entries/goals/" if path.endswith("goal") else "entries/plans/"
    if not projection["relative_path"].startswith(expected_prefix):
        raise LedgerError(f"{path}.relative_path: unexpected projection location")
    _string(projection["sha256"], f"{path}.sha256", SHA256)
    content = _string(projection["content"], f"{path}.content")
    actual = "sha256:" + hashlib.sha256(content.encode("utf-8")).hexdigest()
    if projection["sha256"] != actual:
        raise LedgerError(f"{path}.sha256: does not match content")
    return projection


def validate_preview(value: Any) -> dict[str, Any]:
    preview = _object(value, "$preview", {
        "schema_version", "operation", "repository", "base_commit", "contract_id",
        "contract_revision", "contract_digest", "goal", "plan", "preview_digest",
    })
    if _integer(preview["schema_version"], "$preview.schema_version") != 1:
        raise LedgerError("$preview.schema_version: expected 1")
    if preview["operation"] != "contract-apply":
        raise LedgerError("$preview.operation: expected contract-apply")
    _string(preview["repository"], "$preview.repository")
    _string(preview["base_commit"], "$preview.base_commit", re.compile(r"[0-9a-f]{40}"))
    _string(preview["contract_id"], "$preview.contract_id", CONTRACT_ID)
    _integer(preview["contract_revision"], "$preview.contract_revision")
    _string(preview["contract_digest"], "$preview.contract_digest", SHA256)
    _projection(preview["goal"], "$preview.goal")
    _projection(preview["plan"], "$preview.plan")
    _string(preview["preview_digest"], "$preview.preview_digest", SHA256)
    unsigned = dict(preview)
    unsigned["preview_digest"] = ""
    if preview["preview_digest"] != digest(unsigned):
        raise LedgerError("$preview.preview_digest: does not match canonical preview")
    return preview


def validate_receipt(value: Any) -> dict[str, Any]:
    receipt = _object(value, "$receipt", {
        "schema_version", "contract_id", "contract_revision", "contract_digest",
        "preview_digest", "goal_id", "plan_id", "outcome",
    })
    if _integer(receipt["schema_version"], "$receipt.schema_version") != 1:
        raise LedgerError("$receipt.schema_version: expected 1")
    _string(receipt["contract_id"], "$receipt.contract_id", CONTRACT_ID)
    _integer(receipt["contract_revision"], "$receipt.contract_revision")
    _string(receipt["contract_digest"], "$receipt.contract_digest", SHA256)
    _string(receipt["preview_digest"], "$receipt.preview_digest", SHA256)
    _string(receipt["goal_id"], "$receipt.goal_id", ENTRY_ID)
    _string(receipt["plan_id"], "$receipt.plan_id", ENTRY_ID)
    # The durable receipt returned by `contract show` records the original
    # materialization outcome.  Transient apply results such as `unchanged`
    # (and recovery-path results) are not stable ledger evidence: hashing one
    # would conflict with the later durable `applied` receipt for the same
    # contract identity.
    if receipt["outcome"] != "applied":
        raise LedgerError("$receipt.outcome: expected durable applied receipt from contract show")
    return receipt


def validate_config(value: Any) -> dict[str, Any]:
    config = _object(value, "$config", {"schema_version", "repository"})
    if _integer(config["schema_version"], "$config.schema_version") != 1:
        raise LedgerError("$config.schema_version: expected 1")
    _string(config["repository"], "$config.repository")
    return config


def derive_record(contract_value: Any, receipt_value: Any, preview_value: Any,
                  expected_repository: str) -> dict[str, Any]:
    try:
        contract = omnia_contract.validate_contract(contract_value)
    except omnia_contract.ContractError as exc:
        raise LedgerError(f"$contract: {exc}") from exc
    preview = validate_preview(preview_value)
    receipt = validate_receipt(receipt_value)
    if not expected_repository or preview["repository"] != expected_repository:
        raise LedgerError("repository does not match independently expected repository")
    if contract["target"]["repository"] != expected_repository:
        raise LedgerError("Contract target does not match independently expected repository")
    bindings = {
        "contract_id": contract["contract_id"],
        "contract_revision": contract["revision"],
        "contract_digest": contract["contract_digest"],
    }
    for field, expected in bindings.items():
        if preview[field] != expected or receipt[field] != expected:
            raise LedgerError(f"{field} does not bind Contract, preview, and receipt")
    if preview["base_commit"] != contract["target"]["base_commit"]:
        raise LedgerError("preview base_commit does not bind Contract target")
    if receipt["preview_digest"] != preview["preview_digest"]:
        raise LedgerError("receipt preview_digest does not bind supplied preview")
    if receipt["goal_id"] != preview["goal"]["id"] or receipt["plan_id"] != preview["plan"]["id"]:
        raise LedgerError("receipt target IDs do not bind supplied preview")
    sources = [
        {"source_id": source["id"], "source_url": source["url"]}
        for source in sorted(contract["source_bundle"]["sources"], key=lambda item: item["id"])
    ]
    return {
        "repository": expected_repository,
        "contract_id": contract["contract_id"],
        "contract_revision": contract["revision"],
        "contract_digest": contract["contract_digest"],
        "preview_digest": preview["preview_digest"],
        "goal_id": receipt["goal_id"],
        "plan_id": receipt["plan_id"],
        "receipt_sha256": digest(receipt),
        "sources": sources,
    }


def _validate_record(value: Any, path: str) -> dict[str, Any]:
    record = _object(value, path, {
        "repository", "contract_id", "contract_revision", "contract_digest",
        "preview_digest", "goal_id", "plan_id", "receipt_sha256", "sources",
    })
    _string(record["repository"], f"{path}.repository")
    _string(record["contract_id"], f"{path}.contract_id", CONTRACT_ID)
    _integer(record["contract_revision"], f"{path}.contract_revision")
    for field in ("contract_digest", "preview_digest", "receipt_sha256"):
        _string(record[field], f"{path}.{field}", SHA256)
    _string(record["goal_id"], f"{path}.goal_id", ENTRY_ID)
    _string(record["plan_id"], f"{path}.plan_id", ENTRY_ID)
    if not isinstance(record["sources"], list) or not record["sources"]:
        raise LedgerError(f"{path}.sources: expected non-empty array")
    seen: set[str] = set()
    for index, source_value in enumerate(record["sources"]):
        source = _object(source_value, f"{path}.sources[{index}]", {"source_id", "source_url"})
        source_id = _string(source["source_id"], f"{path}.sources[{index}].source_id")
        _string(source["source_url"], f"{path}.sources[{index}].source_url")
        if source_id in seen:
            raise LedgerError(f"{path}.sources: duplicate source_id")
        seen.add(source_id)
    return record


def validate_ledger(value: Any, expected_repository: str) -> dict[str, Any]:
    ledger = _object(value, "$ledger", {"schema_version", "repository", "records", "external"})
    if _integer(ledger["schema_version"], "$ledger.schema_version") != 1:
        raise LedgerError("$ledger.schema_version: expected 1")
    if ledger["repository"] != expected_repository:
        raise LedgerError("existing ledger repository mismatch")
    if not isinstance(ledger["external"], dict):
        raise LedgerError("$ledger.external: expected object")
    # External IDs/URLs are opaque, caller-owned data.  Preserve exact JSON;
    # this helper never derives, normalizes, or overwrites them.
    if not isinstance(ledger["records"], list):
        raise LedgerError("$ledger.records: expected array")
    identities: set[tuple[str, int]] = set()
    for index, value in enumerate(ledger["records"]):
        record = _validate_record(value, f"$ledger.records[{index}]")
        identity = (record["contract_id"], record["contract_revision"])
        if identity in identities:
            raise LedgerError("existing ledger has duplicate contract identity")
        identities.add(identity)
    return ledger


def reconcile(record: dict[str, Any], existing: Any | None = None) -> dict[str, Any]:
    if existing is None:
        ledger = {"schema_version": 1, "repository": record["repository"],
                  "records": [record], "external": {}}
        return {"schema_version": 1, "action": "rebuild", "changed": True, "ledger": ledger}
    ledger = validate_ledger(existing, record["repository"])
    for current in ledger["records"]:
        identity = (current["contract_id"], current["contract_revision"])
        wanted = (record["contract_id"], record["contract_revision"])
        if identity == wanted:
            if current != record:
                raise LedgerError("contract identity conflict: same id/revision has different content")
            return {"schema_version": 1, "action": "no-op", "changed": False, "ledger": ledger}
    proposed = {"schema_version": 1, "repository": ledger["repository"],
                "records": [*ledger["records"], record], "external": ledger["external"]}
    proposed["records"].sort(key=lambda item: (item["contract_id"], item["contract_revision"]))
    return {"schema_version": 1, "action": "append", "changed": True, "ledger": proposed}


def _read(path: str) -> Any:
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def main(argv: list[str] | None = None) -> int:
    parser = StdoutArgumentParser(add_help=False)
    parser.add_argument("reconcile", choices=("reconcile",))
    parser.add_argument("--contract", required=True)
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--preview", required=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--ledger")
    try:
        args = parser.parse_args(argv)
        config = validate_config(_read(args.config))
        record = derive_record(_read(args.contract), _read(args.receipt), _read(args.preview),
                               config["repository"])
        result = reconcile(record, _read(args.ledger) if args.ledger else None)
        sys.stdout.write(canonical_bytes(result).decode("utf-8"))
        return 0
    except (LedgerError, OSError, json.JSONDecodeError) as exc:
        sys.stdout.write(canonical_bytes({"error": str(exc), "valid": False}).decode("utf-8"))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
