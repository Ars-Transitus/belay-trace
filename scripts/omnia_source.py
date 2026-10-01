#!/usr/bin/env python3
"""Validate and compile synthetic Omnia source envelopes.

This boundary helper is deliberately local and read-only.  A connector adapter
may produce the input envelope, but this module has no connector, credential,
clock, network, or persistence implementation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Protocol


class SourceError(ValueError):
    pass


class SourceAdapter(Protocol):
    """Seam for a caller that obtains envelopes outside the core compiler."""

    def fetch(self, source_ids: list[str]) -> list[dict[str, Any]]: ...


class StdoutArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise SourceError(f"arguments: {message}")


def canonical_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":"), allow_nan=False)
    except (TypeError, ValueError) as exc:
        raise SourceError(f"value is not canonical JSON: {exc}") from exc
    return (encoded + "\n").encode("utf-8")


def _digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def _object(value: Any, path: str, required: set[str], optional: set[str] = set()) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SourceError(f"{path}: expected object")
    missing = sorted(required - set(value))
    if missing:
        raise SourceError(f"{path}: missing required field(s): {', '.join(missing)}")
    unknown = sorted(set(value) - required - optional)
    if unknown:
        raise SourceError(f"{path}: unknown field(s): {', '.join(unknown)}")
    return value


def _string(value: Any, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise SourceError(f"{path}: expected non-empty string")
    return value


def _timestamp(value: Any, path: str) -> str:
    value = _string(value, path)
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        raise SourceError(f"{path}: must be an RFC3339 UTC timestamp with second precision")
    try:
        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise SourceError(f"{path}: invalid timestamp") from exc
    return value


def _strings(value: Any, path: str, *, nonempty: bool = False) -> list[str]:
    if not isinstance(value, list) or (nonempty and not value):
        raise SourceError(f"{path}: expected {'non-empty ' if nonempty else ''}array")
    for index, item in enumerate(value):
        _string(item, f"{path}[{index}]")
    if len(value) != len(set(value)):
        raise SourceError(f"{path}: duplicate item")
    return value


def _boolean(value: Any, path: str) -> bool:
    if not isinstance(value, bool):
        raise SourceError(f"{path}: expected boolean")
    return value


def _integer(value: Any, path: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise SourceError(f"{path}: expected non-negative integer")
    return value


def _validate_config(value: Any, path: str) -> dict[str, Any]:
    config = _object(value, path, {
        "area_id", "data_source_id", "repository", "connector",
    })
    for key in ("area_id", "data_source_id", "repository"):
        _string(config[key], f"{path}.{key}")
    if config["connector"] not in {"notion-mcp", "ntn"}:
        raise SourceError(f"{path}.connector: unsupported connector")
    return config


def validate_envelope(value: Any, expected_config: Any) -> dict[str, Any]:
    root = _object(value, "$", {
        "schema_version", "bundle_id", "retrieved_at", "config",
        "selected_source_ids", "sources",
    })
    if type(root["schema_version"]) is not int or root["schema_version"] != 1:
        raise SourceError("$.schema_version: expected 1")
    _string(root["bundle_id"], "$.bundle_id")
    _timestamp(root["retrieved_at"], "$.retrieved_at")
    config = _validate_config(root["config"], "$.config")
    expected_config = _validate_config(expected_config, "$expected_config")
    for key in ("area_id", "data_source_id", "repository", "connector"):
        if config[key] != expected_config[key]:
            raise SourceError(f"$.config.{key}: does not match expected configuration")
    selected = _strings(root["selected_source_ids"], "$.selected_source_ids", nonempty=True)
    if not isinstance(root["sources"], list) or not root["sources"]:
        raise SourceError("$.sources: expected non-empty array")

    ids: list[str] = []
    dependencies: set[str] = set()
    for index, raw_source in enumerate(root["sources"]):
        path = f"$.sources[{index}]"
        source = _object(raw_source, path, {
            "id", "page_id", "url", "retrieved_at", "raw_snapshot_ref",
            "raw_snapshot", "normalized_snapshot", "required_dependency_ids",
            "boundary", "completeness",
        }, {"edited_at"})
        source_id = _string(source["id"], f"{path}.id")
        ids.append(source_id)
        _string(source["page_id"], f"{path}.page_id")
        _string(source["url"], f"{path}.url")
        _timestamp(source["retrieved_at"], f"{path}.retrieved_at")
        if "edited_at" in source:
            _timestamp(source["edited_at"], f"{path}.edited_at")
        _string(source["raw_snapshot_ref"], f"{path}.raw_snapshot_ref")
        if not isinstance(source["raw_snapshot"], dict):
            raise SourceError(f"{path}.raw_snapshot: expected object")
        if source["raw_snapshot"].get("id") != source["page_id"]:
            raise SourceError(f"{path}.raw_snapshot.id: page identity mismatch")
        if not isinstance(source["normalized_snapshot"], (dict, list)):
            raise SourceError(f"{path}.normalized_snapshot: expected object or array")
        dependencies.update(_strings(source["required_dependency_ids"],
                                     f"{path}.required_dependency_ids"))
        boundary = _object(source["boundary"], f"{path}.boundary", {
            "area_id", "data_source_id", "repository", "connector", "source_id", "page_id",
        })
        expected = {**config, "source_id": source_id, "page_id": source["page_id"]}
        for key, expected_value in expected.items():
            if boundary[key] != expected_value:
                raise SourceError(f"{path}.boundary.{key}: source boundary mismatch")
        complete = _object(source["completeness"], f"{path}.completeness", {
            "truncated", "unknown_block_count", "unknown_block_ids",
            "pagination_complete", "continuation", "permission_denied",
            "deleted", "archived", "relation_cycle",
        })
        _boolean(complete["truncated"], f"{path}.completeness.truncated")
        count = _integer(complete["unknown_block_count"], f"{path}.completeness.unknown_block_count")
        unknown_ids = _strings(complete["unknown_block_ids"], f"{path}.completeness.unknown_block_ids")
        if count != len(unknown_ids):
            raise SourceError(f"{path}.completeness: unknown block count does not match ids")
        _boolean(complete["pagination_complete"], f"{path}.completeness.pagination_complete")
        if complete["continuation"] is not None:
            _string(complete["continuation"], f"{path}.completeness.continuation")
        for key in ("permission_denied", "deleted", "archived", "relation_cycle"):
            _boolean(complete[key], f"{path}.completeness.{key}")
        if complete["pagination_complete"] and complete["continuation"] is not None:
            raise SourceError(f"{path}.completeness: complete pagination cannot carry continuation")
        if not complete["pagination_complete"] and complete["continuation"] is None:
            raise SourceError(f"{path}.completeness: incomplete pagination requires continuation evidence")

    duplicates = sorted({item for item in ids if ids.count(item) > 1})
    if duplicates:
        raise SourceError(f"$.sources: duplicate source id(s): {', '.join(duplicates)}")
    actual = set(ids)
    expected = set(selected) | dependencies
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        details = []
        if missing:
            details.append("missing required source(s): " + ", ".join(missing))
        if extra:
            details.append("unselected source(s): " + ", ".join(extra))
        raise SourceError("$.sources: " + "; ".join(details))
    graph = {item["id"]: item["required_dependency_ids"] for item in root["sources"]}
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(source_id: str) -> None:
        if source_id in visiting:
            raise SourceError(f"$.sources: relation cycle detected at {source_id}")
        if source_id in visited:
            return
        visiting.add(source_id)
        for dependency_id in graph[source_id]:
            visit(dependency_id)
        visiting.remove(source_id)
        visited.add(source_id)

    for source_id in ids:
        visit(source_id)
    return root


def _incomplete_reasons(source: dict[str, Any]) -> list[str]:
    complete = source["completeness"]
    reasons = []
    if complete["truncated"]:
        reasons.append("truncated")
    if complete["unknown_block_count"]:
        reasons.append("unknown-blocks")
    if not complete["pagination_complete"]:
        reasons.append("pagination-incomplete")
    for key in ("permission_denied", "deleted", "archived", "relation_cycle"):
        if complete[key]:
            reasons.append(key.replace("_", "-"))
    return reasons


def compile_bundle(value: Any, expected_config: Any) -> dict[str, Any]:
    envelope = validate_envelope(value, expected_config)
    reports = []
    sources = []
    for source in envelope["sources"]:
        reasons = _incomplete_reasons(source)
        reports.append({"id": source["id"], "complete": not reasons, "reasons": reasons})
        sources.append({
            "id": source["id"],
            "url": source["url"],
            "retrieved_at": source["retrieved_at"],
            "raw_snapshot_ref": source["raw_snapshot_ref"],
            "raw_sha256": _digest(source["raw_snapshot"]),
            "normalized_sha256": _digest(source["normalized_snapshot"]),
            "complete": not reasons,
            **({"edited_at": source["edited_at"]} if "edited_at" in source else {}),
        })
    failed = [report["id"] for report in reports if not report["complete"]]
    if failed:
        detail = "; ".join(
            f"{report['id']} ({', '.join(report['reasons'])})"
            for report in reports if not report["complete"]
        )
        raise SourceError(f"$.sources: incomplete mandatory source(s): {detail}")
    acquisition_manifest = {
        "config": envelope["config"],
        "selected_source_ids": sorted(envelope["selected_source_ids"]),
        "sources": sorted(({
            "id": source["id"],
            "required_dependency_ids": sorted(source["required_dependency_ids"]),
        } for source in envelope["sources"]), key=lambda item: item["id"]),
    }
    acquisition_digest = _digest(acquisition_manifest)
    bundle = {
        "bundle_id": envelope["bundle_id"],
        "acquisition_digest": acquisition_digest,
        "sources": sources,
    }
    bundle["bundle_digest"] = _digest(bundle)
    return {
        "source_bundle": bundle,
        "completeness_report": {
            "complete": True,
            "acquisition_manifest": acquisition_manifest,
            "acquisition_digest": acquisition_digest,
            "sources": reports,
        },
    }


def _read(path: str) -> Any:
    with Path(path).open(encoding="utf-8") as stream:
        return json.load(stream)


def main(argv: list[str] | None = None) -> int:
    parser = StdoutArgumentParser(add_help=False)
    parser.add_argument("command", choices=("validate", "compile"))
    parser.add_argument("path")
    parser.add_argument("--config", required=True)
    try:
        args = parser.parse_args(argv)
        envelope = _read(args.path)
        expected_config = _read(args.config)
        if args.command == "validate":
            validate_envelope(envelope, expected_config)
            result = {"valid": True, "kind": "source-envelope"}
        else:
            result = compile_bundle(envelope, expected_config)
        sys.stdout.write(canonical_bytes(result).decode("utf-8"))
        return 0
    except (SourceError, OSError, json.JSONDecodeError) as exc:
        sys.stdout.write(canonical_bytes({"valid": False, "error": str(exc)}).decode("utf-8"))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
