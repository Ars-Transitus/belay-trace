#!/usr/bin/env python3
"""Deterministic OB-10 operation ledger and read-back reconciliation helper.

The helper only validates caller-supplied JSON and prints a proposed next value.
It has no clock, network, persistence, authentication, or authorization path.
Persist the ``begin-dispatch`` result before making the external append request.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("omnia_contract", ROOT / "scripts" / "omnia_contract.py")
omnia_contract = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(omnia_contract)

SHA256 = re.compile(r"sha256:[0-9a-f]{64}")
OPERATION_ID = re.compile(r"OP-[0-9a-f]{32}")
STATES = {"pending", "unknown", "applied", "conflict"}
HELP_EPILOG = """JSON contracts:
  prepare request = {schema_version:1, target:{page_id,result_area_id},
    result:{summary:{execution,verification,human_acceptance,lifecycle,source_freshness},
    artifact_refs:[],unresolved:[],decision_candidates:[]},
    pre_observation_digest, permission_ref}
  observation = {schema_version:1, target:{page_id,result_area_id}, complete,
    snapshot_sha256, occurrences:[{operation_id,content}], observation_digest}
  observation_digest is sha256 of canonical observation JSON excluding that field.
  snapshot_sha256 binds the complete external result-area snapshot that occurrences
  summarize. Its authenticity and completeness remain caller assertions.
  authorize-retry stores the complete absent observation plus decision_ref and
  original_request_settled:true in the prior unknown attempt.
"""


class ConsolidateError(ValueError):
    pass


class StdoutArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise ConsolidateError(f"arguments: {message}")


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"),
                       allow_nan=False) + "\n").encode("utf-8")


def digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def _object(value: Any, path: str, fields: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ConsolidateError(f"{path}: expected object")
    missing = sorted(fields - set(value))
    unknown = sorted(set(value) - fields)
    if missing:
        raise ConsolidateError(f"{path}: missing required field(s): {', '.join(missing)}")
    if unknown:
        raise ConsolidateError(f"{path}: unknown field(s): {', '.join(unknown)}")
    return value


def _string(value: Any, path: str, pattern: re.Pattern[str] | None = None) -> str:
    if not isinstance(value, str) or not value:
        raise ConsolidateError(f"{path}: expected non-empty string")
    if pattern and not pattern.fullmatch(value):
        raise ConsolidateError(f"{path}: invalid format")
    return value


def _strings(value: Any, path: str) -> list[str]:
    if not isinstance(value, list):
        raise ConsolidateError(f"{path}: expected array")
    result = [_string(item, f"{path}[{index}]") for index, item in enumerate(value)]
    if len(set(result)) != len(result):
        raise ConsolidateError(f"{path}: duplicate item")
    return result


def _target(value: Any, path: str = "$target") -> dict[str, Any]:
    target = _object(value, path, {"page_id", "result_area_id"})
    _string(target["page_id"], f"{path}.page_id")
    _string(target["result_area_id"], f"{path}.result_area_id")
    return target


def _result(value: Any, path: str = "$result") -> dict[str, Any]:
    result = _object(value, path, {"summary", "artifact_refs", "unresolved", "decision_candidates"})
    summary = _object(result["summary"], f"{path}.summary", {
        "execution", "verification", "human_acceptance", "lifecycle", "source_freshness",
    })
    allowed = {
        "execution": {"not-started", "running", "paused", "completed", "cancelled"},
        "verification": {"unverified", "partial", "verified", "failed"},
        "human_acceptance": {"pending", "accepted", "rejected"},
        "lifecycle": {"active", "cancelled", "expired", "superseded"},
        "source_freshness": {"unknown", "observed-current", "changed"},
    }
    for field, choices in allowed.items():
        if summary[field] not in choices:
            raise ConsolidateError(f"{path}.summary.{field}: invalid value")
    _strings(result["artifact_refs"], f"{path}.artifact_refs")
    _strings(result["unresolved"], f"{path}.unresolved")
    _strings(result["decision_candidates"], f"{path}.decision_candidates")
    return result


def _content(value: Any, path: str, expected_operation_id: str | None = None,
             expected_capsule_digest: str | None = None) -> dict[str, Any]:
    content = _object(value, path, {"operation_id", "capsule_digest", "result"})
    operation_id = _string(content["operation_id"], f"{path}.operation_id", OPERATION_ID)
    capsule_digest = _string(content["capsule_digest"], f"{path}.capsule_digest", SHA256)
    if expected_operation_id is not None and operation_id != expected_operation_id:
        raise ConsolidateError(f"{path}.operation_id: does not bind operation")
    if expected_capsule_digest is not None and capsule_digest != expected_capsule_digest:
        raise ConsolidateError(f"{path}.capsule_digest: does not bind operation")
    _result(content["result"], f"{path}.result")
    return content


def _capsule_digest(capsule: Any) -> str:
    try:
        validated = omnia_contract.validate_capsule(capsule)
    except (omnia_contract.ContractError, AttributeError, TypeError) as exc:
        raise ConsolidateError(f"$capsule: {exc}") from exc
    return digest(validated)


def validate_operation(value: Any) -> dict[str, Any]:
    operation = _object(value, "$operation", {
        "schema_version", "operation_id", "target", "capsule_digest", "content",
        "pre_observation_digest", "permission_ref",
    })
    if type(operation["schema_version"]) is not int or operation["schema_version"] != 1:
        raise ConsolidateError("$operation.schema_version: expected 1")
    _string(operation["operation_id"], "$operation.operation_id", OPERATION_ID)
    _target(operation["target"], "$operation.target")
    _string(operation["capsule_digest"], "$operation.capsule_digest", SHA256)
    _content(operation["content"], "$operation.content", operation["operation_id"],
             operation["capsule_digest"])
    _string(operation["pre_observation_digest"], "$operation.pre_observation_digest", SHA256)
    _string(operation["permission_ref"], "$operation.permission_ref")
    unsigned = {key: child for key, child in operation.items() if key != "operation_id"}
    unsigned["content"] = {key: child for key, child in operation["content"].items()
                           if key != "operation_id"}
    expected = "OP-" + hashlib.sha256(canonical_bytes(unsigned)).hexdigest()[:32]
    if operation["operation_id"] != expected:
        raise ConsolidateError("$operation.operation_id: does not match canonical operation")
    return operation


def prepare(capsule: Any, request_value: Any) -> dict[str, Any]:
    request = _object(request_value, "$request", {
        "schema_version", "target", "result", "pre_observation_digest", "permission_ref",
    })
    if type(request["schema_version"]) is not int or request["schema_version"] != 1:
        raise ConsolidateError("$request.schema_version: expected 1")
    target = _target(request["target"])
    result = _result(request["result"])
    pre_digest = _string(request["pre_observation_digest"], "$request.pre_observation_digest", SHA256)
    permission = _string(request["permission_ref"], "$request.permission_ref")
    capsule_digest = _capsule_digest(capsule)
    expected_summary = {field: capsule[field] for field in (
        "execution", "verification", "human_acceptance", "lifecycle", "source_freshness",
    )}
    if result["summary"] != expected_summary:
        raise ConsolidateError("$request.result.summary: does not bind supplied Capsule")
    unsigned = {
        "schema_version": 1, "target": copy.deepcopy(target), "capsule_digest": capsule_digest,
        "content": {"capsule_digest": capsule_digest, "result": copy.deepcopy(result)},
        "pre_observation_digest": pre_digest, "permission_ref": permission,
    }
    operation_id = "OP-" + hashlib.sha256(canonical_bytes(unsigned)).hexdigest()[:32]
    operation = copy.deepcopy(unsigned)
    operation["operation_id"] = operation_id
    operation["content"]["operation_id"] = operation_id
    operation = validate_operation(operation)
    ledger = _new_ledger(operation)
    return {"schema_version": 1, "action": "prepare", "dispatch_allowed_after_persist": False,
            "persist_required": True, "operation": operation, "ledger": ledger}


def _ledger_digest(value: dict[str, Any]) -> str:
    unsigned = {key: child for key, child in value.items() if key != "ledger_digest"}
    return digest(unsigned)


def _new_ledger(operation: dict[str, Any]) -> dict[str, Any]:
    ledger = {"schema_version": 1, "writer_mode": "single", "operation": copy.deepcopy(operation),
              "state": "pending", "attempts": [{"number": 1, "state": "pending"}],
              "ledger_digest": ""}
    ledger["ledger_digest"] = _ledger_digest(ledger)
    return ledger


def validate_ledger(value: Any, operation_value: Any) -> dict[str, Any]:
    ledger = _object(value, "$ledger", {
        "schema_version", "writer_mode", "operation", "state", "attempts", "ledger_digest",
    })
    operation = validate_operation(operation_value)
    if type(ledger["schema_version"]) is not int or ledger["schema_version"] != 1:
        raise ConsolidateError("$ledger.schema_version: expected 1")
    if ledger["writer_mode"] != "single":
        raise ConsolidateError("$ledger.writer_mode: expected single")
    if validate_operation(ledger["operation"]) != operation:
        raise ConsolidateError("$ledger.operation: does not bind supplied operation")
    if ledger["state"] not in STATES:
        raise ConsolidateError("$ledger.state: invalid state")
    if not isinstance(ledger["attempts"], list) or not ledger["attempts"]:
        raise ConsolidateError("$ledger.attempts: expected non-empty array")
    attempt_count = len(ledger["attempts"])
    for index, attempt_value in enumerate(ledger["attempts"], start=1):
        fields = {"number", "state", "settlement"} if index < attempt_count else {"number", "state"}
        attempt = _object(attempt_value, f"$ledger.attempts[{index - 1}]", fields)
        if type(attempt["number"]) is not int or attempt["number"] != index:
            raise ConsolidateError("$ledger.attempts: attempt loss or invalid sequence")
        if attempt["state"] not in STATES:
            raise ConsolidateError(f"$ledger.attempts[{index - 1}].state: invalid state")
        if index < attempt_count:
            if attempt["state"] != "unknown":
                raise ConsolidateError("$ledger.attempts: only settled unknown attempts may precede a retry")
            settlement = _object(attempt["settlement"],
                                 f"$ledger.attempts[{index - 1}].settlement", {
                                     "observation", "decision",
                                 })
            settled_observation = validate_observation(settlement["observation"], operation)
            if not settled_observation["complete"] or _matches(settled_observation, operation):
                raise ConsolidateError(
                    "$ledger.attempts: retry settlement must bind complete operation absence"
                )
            validate_retry_decision(settlement["decision"], operation,
                                    f"$ledger.attempts[{index - 1}].settlement.decision")
    if ledger["attempts"][-1]["state"] != ledger["state"]:
        raise ConsolidateError("$ledger.state: does not match latest attempt")
    _string(ledger["ledger_digest"], "$ledger.ledger_digest", SHA256)
    if ledger["ledger_digest"] != _ledger_digest(ledger):
        raise ConsolidateError("$ledger.ledger_digest: does not match canonical ledger")
    return ledger


def _with_state(ledger: dict[str, Any], state: str) -> dict[str, Any]:
    proposed = copy.deepcopy(ledger)
    proposed["state"] = state
    proposed["attempts"][-1]["state"] = state
    proposed["ledger_digest"] = _ledger_digest(proposed)
    return proposed


def begin_dispatch(operation_value: Any, ledger_value: Any, observation_value: Any) -> dict[str, Any]:
    operation = validate_operation(operation_value)
    ledger = validate_ledger(ledger_value, operation)
    if ledger["state"] != "pending":
        raise ConsolidateError("dispatch requires pending latest attempt; blind retry refused")
    observation = validate_observation(observation_value, operation)
    if not observation["complete"]:
        raise ConsolidateError("pre-dispatch observation must be complete")
    matches = _matches(observation, operation)
    if len(matches) == 1 and matches[0]["content"] == operation["content"]:
        return _result_envelope("already-applied", _with_state(ledger, "applied"), False, True)
    if matches:
        return _result_envelope("pre-dispatch-conflict", _with_state(ledger, "conflict"), False, True)
    if observation["observation_digest"] != operation["pre_observation_digest"]:
        proposed = _with_state(ledger, "conflict")
        return _result_envelope("precondition-changed", proposed, False, True)
    proposed = _with_state(ledger, "unknown")
    return _result_envelope("begin-dispatch", proposed, True, True)


def validate_observation(value: Any, operation: dict[str, Any]) -> dict[str, Any]:
    observation = _object(value, "$observation", {
        "schema_version", "target", "complete", "snapshot_sha256", "occurrences",
        "observation_digest",
    })
    if type(observation["schema_version"]) is not int or observation["schema_version"] != 1:
        raise ConsolidateError("$observation.schema_version: expected 1")
    if _target(observation["target"], "$observation.target") != operation["target"]:
        raise ConsolidateError("$observation.target: foreign target")
    if type(observation["complete"]) is not bool:
        raise ConsolidateError("$observation.complete: expected boolean")
    _string(observation["snapshot_sha256"], "$observation.snapshot_sha256", SHA256)
    _string(observation["observation_digest"], "$observation.observation_digest", SHA256)
    if not isinstance(observation["occurrences"], list):
        raise ConsolidateError("$observation.occurrences: expected array")
    for index, occurrence_value in enumerate(observation["occurrences"]):
        occurrence = _object(occurrence_value, f"$observation.occurrences[{index}]",
                             {"operation_id", "content"})
        _string(occurrence["operation_id"], f"$observation.occurrences[{index}].operation_id",
                OPERATION_ID)
        _content(occurrence["content"], f"$observation.occurrences[{index}].content",
                 occurrence["operation_id"])
    unsigned = {key: child for key, child in observation.items() if key != "observation_digest"}
    if observation["observation_digest"] != digest(unsigned):
        raise ConsolidateError("$observation.observation_digest: does not match canonical observation")
    return observation


def _matches(observation: dict[str, Any], operation: dict[str, Any]) -> list[dict[str, Any]]:
    return [item for item in observation["occurrences"]
            if item["operation_id"] == operation["operation_id"]]


def reconcile(operation_value: Any, ledger_value: Any, observation_value: Any) -> dict[str, Any]:
    operation = validate_operation(operation_value)
    ledger = validate_ledger(ledger_value, operation)
    observation = validate_observation(observation_value, operation)
    if not observation["complete"]:
        if ledger["state"] in {"applied", "conflict"}:
            return _result_envelope("incomplete-terminal-readback", ledger, False, False)
        return _result_envelope("incomplete-readback", _with_state(ledger, "unknown"), False, True)
    matches = _matches(observation, operation)
    if ledger["state"] == "conflict":
        return _result_envelope("no-op", ledger, False, False)
    if len(matches) == 1 and matches[0]["content"] == operation["content"]:
        if ledger["state"] == "applied":
            return _result_envelope("no-op", ledger, False, False)
        return _result_envelope("readback-matched", _with_state(ledger, "applied"), False, True)
    if matches:
        return _result_envelope("readback-conflict", _with_state(ledger, "conflict"), False, True)
    if ledger["state"] == "applied":
        return _result_envelope("readback-conflict", _with_state(ledger, "conflict"), False, True)
    # Complete absence cannot settle an in-flight request by itself.
    next_state = "pending" if ledger["state"] == "pending" else "unknown"
    return _result_envelope("readback-absent", _with_state(ledger, next_state), False,
                            next_state != ledger["state"])


def authorize_retry(operation_value: Any, ledger_value: Any, observation_value: Any,
                    decision_value: Any) -> dict[str, Any]:
    operation = validate_operation(operation_value)
    ledger = validate_ledger(ledger_value, operation)
    if ledger["state"] != "unknown":
        raise ConsolidateError("retry requires unknown latest attempt")
    observation = validate_observation(observation_value, operation)
    if not observation["complete"]:
        raise ConsolidateError("retry requires complete readback")
    if _matches(observation, operation):
        raise ConsolidateError("retry refused: operation is present or conflicting")
    decision = validate_retry_decision(decision_value, operation)
    proposed = copy.deepcopy(ledger)
    proposed["attempts"][-1]["settlement"] = {
        "observation": copy.deepcopy(observation),
        "decision": copy.deepcopy(decision),
    }
    proposed["attempts"].append({"number": len(proposed["attempts"]) + 1, "state": "pending"})
    proposed["state"] = "pending"
    proposed["ledger_digest"] = _ledger_digest(proposed)
    return _result_envelope("retry-authorized", proposed, False, True)


def validate_retry_decision(decision_value: Any, operation: dict[str, Any],
                            path: str = "$decision") -> dict[str, Any]:
    decision = _object(decision_value, path, {
        "schema_version", "operation_id", "decision_ref", "original_request_settled",
    })
    if type(decision["schema_version"]) is not int or decision["schema_version"] != 1:
        raise ConsolidateError(f"{path}.schema_version: expected 1")
    if decision["operation_id"] != operation["operation_id"]:
        raise ConsolidateError(f"{path}.operation_id: does not bind operation")
    _string(decision["decision_ref"], f"{path}.decision_ref")
    if decision["original_request_settled"] is not True:
        raise ConsolidateError(f"{path}.original_request_settled: must be true")
    return decision


def _result_envelope(action: str, ledger: dict[str, Any], dispatch_allowed_after_persist: bool,
                     persist_required: bool) -> dict[str, Any]:
    return {"schema_version": 1, "action": action,
            "dispatch_allowed_after_persist": dispatch_allowed_after_persist,
            "persist_required": persist_required, "ledger": ledger}


def _read(path: str) -> Any:
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def _parser() -> argparse.ArgumentParser:
    parser = StdoutArgumentParser(description=__doc__, epilog=HELP_EPILOG,
                                  formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    prepare_parser = sub.add_parser("prepare", help="derive immutable operation and pending ledger")
    prepare_parser.add_argument("--capsule", required=True)
    prepare_parser.add_argument("--request", required=True)
    for name, help_text in (("begin-dispatch", "propose unknown state to persist before append"),
                            ("reconcile", "classify a complete or incomplete exact-target readback")):
        child = sub.add_parser(name, help=help_text)
        child.add_argument("--operation", required=True)
        child.add_argument("--ledger", required=True)
        child.add_argument("--observation", required=True)
    retry = sub.add_parser("authorize-retry", help="create pending retry after settled absence and Decision")
    retry.add_argument("--operation", required=True)
    retry.add_argument("--ledger", required=True)
    retry.add_argument("--observation", required=True)
    retry.add_argument("--decision", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    try:
        args = _parser().parse_args(argv)
        if args.command == "prepare":
            result = prepare(_read(args.capsule), _read(args.request))
        elif args.command == "begin-dispatch":
            result = begin_dispatch(_read(args.operation), _read(args.ledger), _read(args.observation))
        elif args.command == "reconcile":
            result = reconcile(_read(args.operation), _read(args.ledger), _read(args.observation))
        else:
            result = authorize_retry(_read(args.operation), _read(args.ledger),
                                     _read(args.observation), _read(args.decision))
        sys.stdout.write(canonical_bytes(result).decode("utf-8"))
        return 0
    except (ConsolidateError, OSError, json.JSONDecodeError) as exc:
        sys.stdout.write(canonical_bytes({"valid": False, "error": str(exc)}).decode("utf-8"))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
