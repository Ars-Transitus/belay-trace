use std::fs;
use std::path::Path;
use std::process::Command;

use belay_trace::contract::{self, ApplyFault, ApplyOutcome, Contract, Lifecycle, SourceFreshness};
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
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fixture_value() -> Value {
    serde_json::from_str(include_str!("omnia_contract/golden/contract-v1.json")).unwrap()
}

fn set_digest(value: &mut Value) {
    value.as_object_mut().unwrap().remove("contract_digest");
    let bytes = contract::canonical_bytes(value).unwrap();
    value["contract_digest"] = Value::String(format!("sha256:{:x}", Sha256::digest(bytes)));
}

fn repo_and_contract() -> (TempDir, repository::Repository, Contract) {
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
    let mut value = fixture_value();
    value["target"]["base_commit"] = Value::String(head);
    set_digest(&mut value);
    let contract: Contract = serde_json::from_value(value).unwrap();
    contract::validate(&contract).unwrap();
    (temp, repository, contract)
}

#[test]
fn rust_validation_preserves_python_canonical_contract_golden() {
    let expected = include_bytes!("omnia_contract/golden/contract-v1.json");
    let contract: Contract = serde_json::from_slice(expected).unwrap();
    contract::validate(&contract).unwrap();
    assert_eq!(contract::canonical_bytes(&contract).unwrap(), expected);
}

#[test]
fn preview_binds_target_contract_and_exact_projection_content() {
    let (_temp, repository, contract) = repo_and_contract();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    assert_eq!(preview.repository, "github.com/Ars-Transitus/belay-trace");
    assert_eq!(preview.contract_digest, contract.contract_digest);
    assert!(!preview.goal.content.contains("acceptance_criterion"));
    assert!(preview.goal.content.contains("AC-001"));
    assert!(preview.plan.content.contains("## T-001"));
    assert!(preview.goal.content.contains("projection_digest"));
    assert!(
        contract::apply(
            &repository,
            &contract,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            SourceFreshness::ObservedCurrent,
            true,
        )
        .is_err()
    );
}

#[test]
fn apply_is_idempotent_and_rejects_projection_drift() {
    let (_temp, repository, contract) = repo_and_contract();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    let first = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
    )
    .unwrap();
    assert_eq!(first.outcome, ApplyOutcome::Applied);
    let retry = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap();
    assert_eq!(retry.outcome, ApplyOutcome::Unchanged);
    let goal = repository.belay_dir.join(&preview.goal.relative_path);
    fs::write(&goal, "unexpected hand edit\n").unwrap();
    let error = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("unexpected bytes"));
}

#[test]
fn recovery_after_each_durable_boundary_reuses_ids_without_duplicates() {
    for fault in [
        ApplyFault::AfterSchemaUpgrade,
        ApplyFault::AfterOriginal,
        ApplyFault::AfterIntent,
        ApplyFault::AfterGoal,
        ApplyFault::AfterPlan,
        ApplyFault::AfterReceipt,
    ] {
        let (_temp, repository, contract) = repo_and_contract();
        let preview =
            contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
        assert!(
            contract::apply_with_fault(
                &repository,
                &contract,
                &preview.preview_digest,
                SourceFreshness::ObservedCurrent,
                true,
                Some(fault)
            )
            .is_err()
        );
        let receipt = contract::apply(
            &repository,
            &contract,
            &preview.preview_digest,
            SourceFreshness::ObservedCurrent,
            true,
        )
        .unwrap();
        assert_eq!(receipt.goal_id, preview.goal.id);
        assert_eq!(receipt.plan_id, preview.plan.id);
        let goals = fs::read_dir(repository.entries_path().join("goals"))
            .unwrap()
            .count();
        let plans = fs::read_dir(repository.entries_path().join("plans"))
            .unwrap()
            .count();
        assert_eq!((goals, plans), (1, 1));
        let shown = contract::show(&repository, &contract.contract_id, contract.revision).unwrap();
        assert_eq!(shown.preview_digest, preview.preview_digest);
    }
}

#[test]
fn apply_upgrades_schema_and_schema_two_only_writer_fails_closed() {
    let (_temp, repository, contract) = repo_and_contract();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    let config_path = repository.belay_dir.join("config.toml");
    let before = fs::read(&config_path).unwrap();
    let error = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("legacy writers"));
    assert_eq!(fs::read(&config_path).unwrap(), before);
    contract::apply_with_fault(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
        Some(ApplyFault::AfterSchemaUpgrade),
    )
    .unwrap_err();
    assert_eq!(
        belay_trace::config::Config::load(&config_path)
            .unwrap()
            .schema_version,
        3
    );
    let error = belay_trace::config::Config::load_compatible(&config_path, 2).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported config schema version 3")
    );
    assert!(!repository.belay_dir.join("state/contracts").exists());
    let recovered = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap();
    assert_eq!(recovered.outcome, ApplyOutcome::Applied);
}

#[test]
fn receipt_is_canonical_and_all_immutable_fields_are_verified() {
    for field in [
        "schema_version",
        "contract_id",
        "contract_revision",
        "contract_digest",
        "preview_digest",
        "goal_id",
        "plan_id",
        "outcome",
    ] {
        let (_temp, repository, contract) = repo_and_contract();
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
        let receipt_path = repository
            .belay_dir
            .join("state/contracts")
            .join(&contract.contract_id)
            .join(contract.revision.to_string())
            .join("receipt.json");
        let mut receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
        receipt[field] = match field {
            "schema_version" | "contract_revision" => Value::from(99),
            "outcome" => Value::String("recovered".into()),
            _ => Value::String("tampered".into()),
        };
        fs::write(&receipt_path, contract::canonical_bytes(&receipt).unwrap()).unwrap();
        let error =
            contract::show(&repository, &contract.contract_id, contract.revision).unwrap_err();
        assert!(error.to_string().contains("receipt"), "{field}: {error}");
    }
}

#[cfg(unix)]
#[test]
fn contract_artifact_symlinks_are_rejected_even_when_contents_match() {
    use std::os::unix::fs::symlink;

    let (_temp, repository, contract) = repo_and_contract();
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
    let revision_dir = repository
        .belay_dir
        .join("state/contracts")
        .join(&contract.contract_id)
        .join(contract.revision.to_string());
    let receipt = revision_dir.join("receipt.json");
    let external = repository.root.join("matching-receipt.json");
    fs::write(&external, fs::read(&receipt).unwrap()).unwrap();
    fs::remove_file(&receipt).unwrap();
    symlink(&external, &receipt).unwrap();
    assert!(contract::show(&repository, &contract.contract_id, contract.revision).is_err());

    fs::remove_file(&receipt).unwrap();
    fs::write(&receipt, fs::read(&external).unwrap()).unwrap();
    let moved = repository.root.join("matching-revision");
    fs::rename(&revision_dir, &moved).unwrap();
    symlink(&moved, &revision_dir).unwrap();
    assert!(contract::show(&repository, &contract.contract_id, contract.revision).is_err());
}

#[test]
fn lifecycle_freshness_target_and_identity_collisions_fail_before_mutation() {
    let (_temp, repository, contract) = repo_and_contract();
    assert!(contract::preview(&repository, &contract, SourceFreshness::Unknown).is_err());
    assert!(!repository.belay_dir.join("state/contracts").exists());

    let mut cancelled = contract.clone();
    cancelled.lifecycle = Lifecycle::Cancelled;
    let mut value = serde_json::to_value(cancelled).unwrap();
    set_digest(&mut value);
    let cancelled: Contract = serde_json::from_value(value).unwrap();
    assert!(contract::preview(&repository, &cancelled, SourceFreshness::ObservedCurrent).is_err());

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
    let mut changed = serde_json::to_value(&contract).unwrap();
    changed["outcome"] = Value::String("different payload at same identity".into());
    set_digest(&mut changed);
    let changed: Contract = serde_json::from_value(changed).unwrap();
    let changed_preview =
        contract::preview(&repository, &changed, SourceFreshness::ObservedCurrent).unwrap();
    let error = contract::apply(
        &repository,
        &changed,
        &changed_preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("different payload")
            || error.to_string().contains("unexpected bytes")
    );
}

#[test]
fn a_new_revision_gets_distinct_goal_and_plan_ids() {
    let (_temp, repository, contract) = repo_and_contract();
    let first =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    contract::apply(
        &repository,
        &contract,
        &first.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
    )
    .unwrap();
    let mut next = serde_json::to_value(&contract).unwrap();
    next["revision"] = Value::from(2);
    next["outcome"] = Value::String("Revised deterministic contract".into());
    set_digest(&mut next);
    let next: Contract = serde_json::from_value(next).unwrap();
    let second = contract::preview(&repository, &next, SourceFreshness::ObservedCurrent).unwrap();
    assert_ne!(first.goal.id, second.goal.id);
    assert_ne!(first.plan.id, second.plan.id);
    contract::apply(
        &repository,
        &next,
        &second.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap();
}

#[test]
fn completed_receipt_recovers_index_after_database_loss_and_orphans_fail_closed() {
    let (_temp, repository, contract) = repo_and_contract();
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
    let database = repository.database_path();
    fs::remove_file(&database).unwrap();
    let _ = fs::remove_file(database.with_extension("sqlite-wal"));
    let _ = fs::remove_file(database.with_extension("sqlite-shm"));
    let recovered = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        false,
    )
    .unwrap();
    assert_eq!(recovered.outcome, ApplyOutcome::Unchanged);

    let (_temp, orphan_repository, orphan_contract) = repo_and_contract();
    let orphan_preview = contract::preview(
        &orphan_repository,
        &orphan_contract,
        SourceFreshness::ObservedCurrent,
    )
    .unwrap();
    fs::write(
        orphan_repository
            .belay_dir
            .join(&orphan_preview.goal.relative_path),
        &orphan_preview.goal.content,
    )
    .unwrap();
    let error = contract::apply(
        &orphan_repository,
        &orphan_contract,
        &orphan_preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
    )
    .unwrap_err();
    assert!(error.to_string().contains("orphan Contract projection"));
}
