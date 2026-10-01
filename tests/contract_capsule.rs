use std::{fs, path::Path, process::Command};

use belay_trace::{
    capsule::{self, EvidenceState, Execution, HumanAcceptance, Verification},
    contract::{self, Contract, SourceFreshness},
    evidence::EvidenceRecord,
    repository,
};
use serde_json::{Value, json};
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
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn set_digest(value: &mut Value) {
    value.as_object_mut().unwrap().remove("contract_digest");
    value["contract_digest"] = Value::String(format!(
        "sha256:{:x}",
        Sha256::digest(contract::canonical_bytes(value).unwrap())
    ));
}

fn applied(two_acs: bool) -> (TempDir, repository::Repository, Contract, String) {
    let temp = TempDir::new().unwrap();
    run(temp.path(), &["git", "init", "-q"]);
    run(
        temp.path(),
        &["git", "config", "user.email", "test@example.invalid"],
    );
    run(temp.path(), &["git", "config", "user.name", "Capsule Test"]);
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
    value["target"]["base_commit"] = Value::String(head.clone());
    if two_acs {
        value["acceptance_criteria"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"AC-002","text":"second","source_refs":["SRC-001"]}));
    }
    set_digest(&mut value);
    let contract: Contract = serde_json::from_value(value).unwrap();
    let preview =
        contract::preview(&repository, &contract, SourceFreshness::ObservedCurrent).unwrap();
    let receipt = contract::apply(
        &repository,
        &contract,
        &preview.preview_digest,
        SourceFreshness::ObservedCurrent,
        true,
    )
    .unwrap();
    (temp, repository, contract, receipt.goal_id)
}

fn evidence(
    repository: &repository::Repository,
    id: &str,
    verdict: &str,
    commit: &str,
    captured: &str,
    targets: &[String],
) {
    let record = EvidenceRecord {
        schema_version: 1,
        display_id: id.into(),
        kind: "test".into(),
        verdict: verdict.into(),
        commit_sha: commit.into(),
        captured_at: captured.into(),
        source: "fixture".into(),
        issuer: "test".into(),
        summary: "fixture".into(),
        detail: json!({}),
        links: targets
            .iter()
            .map(|target| belay_trace::evidence::EvidenceLink {
                target: target.clone(),
                relation: "verifies".into(),
            })
            .collect(),
    };
    let path = repository
        .evidence_path()
        .join("records")
        .join(format!("{id}.json"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contract::canonical_bytes(&record).unwrap()).unwrap();
}

#[test]
fn missing_future_expiry_repeatability_and_pending_acceptance() {
    let (_temp, repository, contract, goal) = applied(false);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    evidence(
        &repository,
        "EVD-11111111111111111111111111111111",
        "pass",
        &head,
        "2026-10-10T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    let first = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-09T00:00:00Z",
        Execution::Paused,
    )
    .unwrap();
    let second = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-09T00:00:00Z",
        Execution::Paused,
    )
    .unwrap();
    assert_eq!(
        contract::canonical_bytes(&first).unwrap(),
        contract::canonical_bytes(&second).unwrap()
    );
    assert_eq!(first.criteria[0].verification, Verification::Unverified);
    assert!(first.criteria[0].evidence.is_empty());
    assert_eq!(first.lifecycle, contract::Lifecycle::Expired);
    assert_eq!(first.human_acceptance, HumanAcceptance::Pending);
    assert!(capsule::render_uncovered(&first).contains("AC-001: Unverified"));
}

#[test]
fn fractional_as_of_preserves_the_exact_decision_instant() {
    let (_temp, repository, contract, goal) = applied(false);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    evidence(
        &repository,
        "EVD-12121212121212121212121212121212",
        "pass",
        &head,
        "2026-10-01T00:00:00.500Z",
        &[format!("{goal}#sc-001")],
    );
    let before = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-01T00:00:00.400Z",
        Execution::Running,
    )
    .unwrap();
    let after = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-01T00:00:00.600Z",
        Execution::Running,
    )
    .unwrap();
    assert_eq!(before.evaluated_as_of, "2026-10-01T00:00:00.400Z");
    assert_eq!(after.evaluated_as_of, "2026-10-01T00:00:00.600Z");
    assert_eq!(before.verification, Verification::Unverified);
    assert_eq!(after.verification, Verification::Verified);
    let path = _temp.path().join("fractional-capsule.json");
    fs::write(&path, contract::canonical_bytes(&after).unwrap()).unwrap();
    let validator = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/omnia_contract.py");
    let output = Command::new("python3")
        .args([
            validator.as_os_str(),
            "validate".as_ref(),
            path.as_os_str(),
            "--kind".as_ref(),
            "capsule".as_ref(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn exact_sc_mapping_mixed_failure_stale_and_cross_ac_reuse() {
    let (_temp, repository, contract, goal) = applied(true);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    let both = [format!("{goal}#sc-001"), format!("{goal}#sc-002")];
    evidence(
        &repository,
        "EVD-22222222222222222222222222222222",
        "pass",
        &head,
        "2026-10-01T00:00:00Z",
        &both,
    );
    evidence(
        &repository,
        "EVD-33333333333333333333333333333333",
        "pass",
        "unknown",
        "2020-01-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    evidence(
        &repository,
        "EVD-44444444444444444444444444444444",
        "fail",
        &head,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-002")],
    );
    let result = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(result.criteria[0].verification, Verification::Partial);
    assert_eq!(result.criteria[1].verification, Verification::Failed);
    assert_eq!(result.verification, Verification::Failed);
    assert_eq!(
        result.criteria[0].evidence[0],
        result.criteria[1].evidence[0]
    );
    assert!(
        result.criteria[0]
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Stale)
    );
}

#[test]
fn warn_and_info_never_create_verified_criteria() {
    for (offset, verdict) in [(5, "warn"), (6, "info")] {
        let (_temp, repository, contract, goal) = applied(false);
        let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
        evidence(
            &repository,
            &format!("EVD-{offset:032}"),
            verdict,
            &head,
            "2026-10-01T00:00:00Z",
            &[format!("{goal}#sc-001")],
        );
        let result = capsule::produce(
            &repository,
            &contract.contract_id,
            1,
            "2026-10-02T00:00:00Z",
            Execution::Completed,
        )
        .unwrap();
        assert_eq!(result.criteria[0].verification, Verification::Unverified);
        assert_eq!(result.criteria[0].evidence[0].state, EvidenceState::Missing);
    }
}

#[test]
fn capsule_rejects_descendant_unrelated_and_dirty_product_but_allows_belay_changes() {
    // A clean ancestor-or-equal commit with only uncommitted .belay state passes.
    let (_temp, repository, contract, goal) = applied(false);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    fs::write(repository.belay_dir.join("local-only"), "state").unwrap();
    evidence(
        &repository,
        "EVD-77777777777777777777777777777777",
        "pass",
        &head,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    let clean = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(clean.verification, Verification::Verified);

    fs::write(repository.root.join("product.txt"), "dirty").unwrap();
    let dirty = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(dirty.criteria[0].evidence[0].state, EvidenceState::Stale);

    // A future descendant commit cannot prove the currently checked-out ancestor.
    let (_temp, repository, contract, goal) = applied(false);
    let ancestor = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    fs::write(repository.root.join("product.txt"), "future").unwrap();
    run(&repository.root, &["git", "add", "product.txt"]);
    run(&repository.root, &["git", "commit", "-qm", "future"]);
    let descendant = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    run(&repository.root, &["git", "checkout", "-q", &ancestor]);
    evidence(
        &repository,
        "EVD-88888888888888888888888888888888",
        "pass",
        &descendant,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    let future = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(future.criteria[0].evidence[0].state, EvidenceState::Stale);

    // A commit on an unrelated history is equally non-passing.
    let (_temp, repository, contract, goal) = applied(false);
    let original_branch = run(&repository.root, &["git", "branch", "--show-current"]);
    run(
        &repository.root,
        &["git", "checkout", "--orphan", "unrelated"],
    );
    run(
        &repository.root,
        &["git", "commit", "--allow-empty", "-qm", "unrelated"],
    );
    let unrelated = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    run(
        &repository.root,
        &["git", "checkout", "-q", &original_branch],
    );
    evidence(
        &repository,
        "EVD-99999999999999999999999999999999",
        "pass",
        &unrelated,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    let result = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(result.criteria[0].evidence[0].state, EvidenceState::Stale);
}

#[test]
fn capsule_checks_both_rename_paths_and_short_or_weird_names() {
    // Moving a tracked short product path into .belay remains a product change.
    let (_temp, repository, contract, goal) = applied(false);
    fs::write(repository.root.join("a"), "product").unwrap();
    run(&repository.root, &["git", "add", "a"]);
    run(&repository.root, &["git", "commit", "-qm", "product"]);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    evidence(
        &repository,
        "EVD-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "pass",
        &head,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    run(&repository.root, &["git", "mv", "a", ".belay/a"]);
    let moved = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(moved.criteria[0].evidence[0].state, EvidenceState::Stale);

    // A rename wholly within .belay and a UTF-8 unusual .belay name are allowed.
    let (_temp, repository, contract, goal) = applied(false);
    fs::write(repository.belay_dir.join("a"), "local").unwrap();
    run(&repository.root, &["git", "add", "-f", ".belay/a"]);
    run(&repository.root, &["git", "commit", "-qm", "local state"]);
    let head = run(&repository.root, &["git", "rev-parse", "HEAD"]);
    evidence(
        &repository,
        "EVD-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "pass",
        &head,
        "2026-10-01T00:00:00Z",
        &[format!("{goal}#sc-001")],
    );
    run(&repository.root, &["git", "mv", ".belay/a", ".belay/変"]);
    let local = capsule::produce(
        &repository,
        &contract.contract_id,
        1,
        "2026-10-02T00:00:00Z",
        Execution::Completed,
    )
    .unwrap();
    assert_eq!(local.verification, Verification::Verified);
}

#[cfg(unix)]
#[test]
fn stored_contract_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let (_temp, repository, contract, _goal) = applied(false);
    let original = repository
        .belay_dir
        .join("state/contracts")
        .join(&contract.contract_id)
        .join("1/original.json");
    let copy = repository.root.join("outside.json");
    fs::write(&copy, fs::read(&original).unwrap()).unwrap();
    fs::remove_file(&original).unwrap();
    symlink(copy, original).unwrap();
    assert!(
        capsule::produce(
            &repository,
            &contract.contract_id,
            1,
            "2026-10-02T00:00:00Z",
            Execution::Running
        )
        .is_err()
    );
}
