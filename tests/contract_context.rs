use std::fs;
use std::path::Path;
use std::process::Command;

use belay_trace::context::{self, ContextFormat};
use belay_trace::contract::{self, ApplyFault, Contract, Lifecycle, SourceFreshness};
use belay_trace::export::{self, ExportFilter, ExportFormat};
use belay_trace::markdown;
use belay_trace::reconcile::{self, SyncPreference};
use belay_trace::repository;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(args[0])
        .args(&args[1..])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn set_digest(value: &mut Value) {
    value.as_object_mut().unwrap().remove("contract_digest");
    let bytes = contract::canonical_bytes(value).unwrap();
    value["contract_digest"] = Value::String(format!("sha256:{:x}", Sha256::digest(bytes)));
}

fn fixture() -> (TempDir, repository::Repository, Contract) {
    let temp = TempDir::new().unwrap();
    run(temp.path(), &["git", "init", "-q"]);
    run(
        temp.path(),
        &["git", "config", "user.email", "test@example.invalid"],
    );
    run(
        temp.path(),
        &["git", "config", "user.name", "Contract Test"],
    );
    run(
        temp.path(),
        &[
            "git",
            "remote",
            "add",
            "origin",
            "https://github.com/Ars-Transitus/belay-trace.git",
        ],
    );
    run(
        temp.path(),
        &["git", "commit", "--allow-empty", "-qm", "fixture"],
    );
    let head = run(temp.path(), &["git", "rev-parse", "HEAD"]);
    let repository = repository::initialize(temp.path()).unwrap().repository;
    let mut value: Value =
        serde_json::from_str(include_str!("omnia_contract/golden/contract-v1.json")).unwrap();
    value["target"]["base_commit"] = Value::String(head);
    set_digest(&mut value);
    let contract = serde_json::from_value(value).unwrap();
    (temp, repository, contract)
}

fn applied() -> (
    TempDir,
    repository::Repository,
    Contract,
    contract::ContractPreview,
) {
    let (temp, repository, contract) = fixture();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
    )
    .unwrap();
    (temp, repository, contract, preview)
}

#[test]
fn editable_task_progress_survives_sync_retry_rebuild_and_export() {
    let (_temp, repository, contract, preview) = applied();
    let path = repository.belay_dir.join(&preview.plan.relative_path);
    let original = fs::read_to_string(&path).unwrap();
    let edited = original.replace("| not-started |", "| implemented |")
        + "\n## T-900\n- Objective: locally decomposed follow-up\n- State: in-progress\n";
    fs::write(&path, edited).unwrap();
    let report = reconcile::synchronize(
        &repository,
        Some(&preview.plan.id),
        Some(SyncPreference::Markdown),
    )
    .unwrap();
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let retried = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap();
    assert_eq!(retried.outcome, contract::ApplyOutcome::Unchanged);
    let packet = context::compile_focus(
        &repository,
        &format!("{}#t-001", preview.plan.id),
        ContextFormat::Agent,
        10_000,
    )
    .unwrap();
    assert!(packet.text.contains("## Contract boundary"));
    assert!(packet.text.find("## Contract boundary") < packet.text.find("## Intent Brief"));
    assert!(packet.text.contains("AC-001 -> SC-001"));
    assert!(packet.text.contains("Source freshness: Unknown"));

    fs::remove_file(repository.database_path()).unwrap();
    let rebuilt = reconcile::rebuild(&repository).unwrap();
    assert_eq!(rebuilt.entries, 2);
    let after = context::compile_focus(
        &repository,
        &format!("{}#t-001", preview.plan.id),
        ContextFormat::Agent,
        10_000,
    )
    .unwrap();
    assert!(after.text.contains("AC-001 -> SC-001"));

    let export_path = repository.root.join("contract-plan.json");
    export::write(
        &repository,
        ExportFormat::Json,
        &export_path,
        &ExportFilter {
            display_id: Some(preview.plan.id.clone()),
            ..ExportFilter::default()
        },
    )
    .unwrap();
    let exported = fs::read_to_string(export_path).unwrap();
    assert!(exported.contains("AC-001"));
    assert!(exported.contains("contract_digest"));
    assert!(exported.contains("implemented"));
}

#[test]
fn protected_edits_and_marker_injection_are_rejected() {
    for replacement in [
        ("Read local synthetic snapshots only", "weakened constraint"),
        (
            "## Delivery Map",
            "<!-- belay-contract:editable-tasks-v1 -->\n## Delivery Map",
        ),
    ] {
        let (_temp, repository, contract, preview) = applied();
        let path = repository.belay_dir.join(&preview.plan.relative_path);
        let original = fs::read_to_string(&path).unwrap();
        assert!(
            original.contains(replacement.0),
            "missing protected fixture text: {}",
            replacement.0
        );
        fs::write(&path, original.replace(replacement.0, replacement.1)).unwrap();
        let error =
            contract::show(&repository, &contract.contract_id, contract.revision).unwrap_err();
        assert!(
            error.to_string().contains("protected Contract region")
                || error
                    .to_string()
                    .contains("more than one editable task delimiter")
        );
    }
}

#[test]
fn ordinary_full_id_compile_emits_contract_boundary_first() {
    let (_temp, repository, _contract, preview) = applied();
    for id in [&preview.goal.id, &preview.plan.id] {
        let packet =
            context::compile(&repository, id, ContextFormat::Agent, 10_000, &[], false).unwrap();
        let boundary = packet.text.find("## Contract boundary").unwrap();
        assert!(boundary < packet.text.find("## Selection").unwrap());
        assert!(boundary < packet.text.find("## Goals").unwrap_or(usize::MAX));
        assert!(boundary < packet.text.find("## Plans").unwrap_or(usize::MAX));
    }
}

#[test]
fn every_context_surface_carries_complete_immutable_intent() {
    let (_temp, repository, contract, preview) = applied();
    let focus = format!("{}#t-001", preview.plan.id);
    let packets = [
        context::generate(
            &repository,
            &preview.plan.id,
            ContextFormat::Agent,
            10_000,
            false,
        )
        .unwrap(),
        context::compile(
            &repository,
            &preview.plan.id,
            ContextFormat::Agent,
            10_000,
            &[],
            false,
        )
        .unwrap(),
        context::compile_focus(&repository, &focus, ContextFormat::Agent, 10_000).unwrap(),
        context::compile_working_set(&repository, ContextFormat::Agent, 10_000, false).unwrap(),
    ];
    for packet in packets {
        assert!(packet.text.contains("## Contract boundary"));
        assert!(
            packet
                .text
                .contains("REQ-001: Compilation is deterministic")
        );
        assert!(
            packet
                .text
                .contains("Assumptions:\n- All source references are synthetic")
        );
        assert!(packet.text.contains("Context references:\n- SRC-001"));
        assert!(
            packet
                .text
                .contains(&format!("Input: {} revision 1", contract.input_id))
        );
        assert!(
            packet
                .text
                .contains(&format!("Confirmation: {}", contract.confirmation_ref))
        );
        assert!(packet.text.contains(&format!(
            "Target repository: {}",
            contract.target.repository
        )));
        assert!(packet.text.contains(&format!(
            "Target base commit: {}",
            contract.target.base_commit
        )));
    }
}

#[test]
fn working_set_finds_active_contract_after_task_states_leave_active_set() {
    let (_temp, repository, _contract, preview) = applied();
    let path = repository.belay_dir.join(&preview.plan.relative_path);
    let original = fs::read_to_string(&path).unwrap();
    assert!(original.contains("| not-started |"));
    fs::write(&path, original.replace("| not-started |", "| verified |")).unwrap();
    let report = reconcile::synchronize(
        &repository,
        Some(&preview.plan.id),
        Some(SyncPreference::Markdown),
    )
    .unwrap();
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let packet =
        context::compile_working_set(&repository, ContextFormat::Agent, 10_000, false).unwrap();
    assert!(packet.text.contains("## Contract boundary"));
    assert!(
        packet
            .text
            .contains("REQ-001: Compilation is deterministic")
    );
    assert!(packet.text.find("## Contract boundary") < packet.text.find("## Goals / Plans"));
}

#[test]
fn working_set_fails_closed_for_each_missing_durable_contract_artifact() {
    for artifact in ["intent.json", "receipt.json", "original.json"] {
        let (_temp, repository, contract, _preview) = applied();
        let path = repository
            .belay_dir
            .join("state/contracts")
            .join(&contract.contract_id)
            .join(contract.revision.to_string())
            .join(artifact);
        fs::remove_file(path).unwrap();
        let error = context::compile_working_set(&repository, ContextFormat::Agent, 10_000, false)
            .unwrap_err();
        let message = error.to_string();
        match artifact {
            "intent.json" => assert!(message.contains("missing intent.json"), "{message}"),
            "receipt.json" | "original.json" => {
                assert!(message.contains(artifact), "{message}")
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn original_only_pre_intent_state_remains_recoverable() {
    let (_temp, repository, contract) = fixture();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    contract::apply_with_fault(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
        Some(ApplyFault::AfterOriginal),
    )
    .unwrap_err();

    let packet =
        context::compile_working_set(&repository, ContextFormat::Agent, 10_000, false).unwrap();
    assert!(!packet.text.contains("## Contract boundary"));
}

#[test]
fn protected_acceptance_tamper_blocks_sync_and_export() {
    let (_temp, repository, _contract, preview) = applied();
    let path = repository.belay_dir.join(&preview.goal.relative_path);
    let original = fs::read_to_string(&path).unwrap();
    assert!(original.contains("Repeated compilation yields identical UTF-8 bytes"));
    fs::write(
        &path,
        original.replace(
            "Repeated compilation yields identical UTF-8 bytes",
            "weakened acceptance",
        ),
    )
    .unwrap();

    let report = reconcile::synchronize(
        &repository,
        Some(&preview.goal.id),
        Some(SyncPreference::Markdown),
    )
    .unwrap();
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].message.contains("unexpected bytes"));

    let export_path = repository.root.join("tampered.json");
    let error = export::write(
        &repository,
        ExportFormat::Json,
        &export_path,
        &ExportFilter {
            display_id: Some(preview.goal.id.clone()),
            ..ExportFilter::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("unexpected bytes"));
    assert!(!export_path.exists());
}

#[test]
fn stripped_projection_metadata_cannot_bypass_focus_or_rebuild() {
    let (_temp, repository, _contract, preview) = applied();
    let path = repository.belay_dir.join(&preview.plan.relative_path);
    let mut entry = markdown::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    entry.metadata.remove("contract_id");
    entry.metadata.remove("contract_revision");
    fs::write(&path, markdown::render(&entry).unwrap()).unwrap();

    let focus = format!("{}#t-001", preview.plan.id);
    let error =
        context::compile_focus(&repository, &focus, ContextFormat::Agent, 10_000).unwrap_err();
    assert!(
        error.to_string().contains("protected frontmatter drifted"),
        "{error}"
    );

    fs::remove_file(repository.database_path()).unwrap();
    let rebuild = reconcile::rebuild(&repository).unwrap_err();
    assert!(
        rebuild
            .to_string()
            .contains("protected frontmatter drifted")
    );
}

#[test]
fn contract_context_fails_closed_for_tiny_budget_and_missing_original() {
    let (_temp, repository, contract, preview) = applied();
    let focus = format!("{}#t-001", preview.plan.id);
    let error = context::compile_focus(&repository, &focus, ContextFormat::Agent, 64).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("shortfall="));
    assert!(message.contains("contract-boundary"));

    let original = repository
        .belay_dir
        .join("state/contracts")
        .join(&contract.contract_id)
        .join(contract.revision.to_string())
        .join("original.json");
    fs::remove_file(original).unwrap();
    let missing =
        context::compile_focus(&repository, &focus, ContextFormat::Agent, 10_000).unwrap_err();
    assert!(missing.to_string().contains("original.json"));
}

#[test]
fn lifecycle_states_and_local_freshness_are_reported_without_online_claims() {
    let (_temp, _repository, contract) = fixture();
    for (state, expected) in [
        (
            Lifecycle::Cancelled,
            "cancelled; mutation and dispatch blocked",
        ),
        (Lifecycle::Expired, "expired; mutation and dispatch blocked"),
        (
            Lifecycle::Superseded,
            "superseded; mutation and dispatch blocked",
        ),
    ] {
        let mut value = serde_json::to_value(&contract).unwrap();
        value["lifecycle"] = Value::String(
            serde_json::to_value(state)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
        );
        set_digest(&mut value);
        let changed: Contract = serde_json::from_value(value).unwrap();
        let rendered = contract::render_context_boundary(&changed).unwrap();
        assert!(rendered.contains(expected));
        assert!(rendered.contains("Source freshness: Unknown"));
        assert!(!rendered.contains("observed-current"));
    }
}
