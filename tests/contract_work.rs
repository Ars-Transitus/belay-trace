use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use belay_trace::contract;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

fn belay() -> Command {
    Command::new(env!("CARGO_BIN_EXE_belay"))
}

fn run(root: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .expect("run command")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn initialize_git_repository() -> (TempDir, String) {
    let temporary = tempdir().expect("create temporary repository");
    for args in [
        &["init", "-q"][..],
        &["config", "user.email", "test@example.invalid"][..],
        &["config", "user.name", "Contract Work Test"][..],
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/Ars-Transitus/belay-trace.git",
        ][..],
        &["commit", "--allow-empty", "-qm", "fixture"][..],
    ] {
        assert_success(&run(temporary.path(), "git", args));
    }
    let head = run(temporary.path(), "git", &["rev-parse", "HEAD"]);
    assert_success(&head);
    let head = String::from_utf8(head.stdout)
        .expect("HEAD is UTF-8")
        .trim()
        .to_owned();
    let initialized = belay()
        .arg("init")
        .current_dir(temporary.path())
        .output()
        .expect("initialize Belay");
    assert_success(&initialized);
    (temporary, head)
}

fn created_id(output: &Output) -> String {
    assert_success(output);
    String::from_utf8(output.stdout.clone())
        .expect("created ID is UTF-8")
        .trim()
        .strip_prefix("Created ")
        .expect("human create output")
        .to_owned()
}

fn add_goal(root: &Path, title: &str) -> String {
    created_id(
        &belay()
            .args([
                "add",
                "goal",
                "--title",
                title,
                "--body",
                "## Success Criteria\n\n- [SC-001] Work is linked.\n",
            ])
            .current_dir(root)
            .output()
            .expect("add Goal"),
    )
}

fn add_plan(root: &Path, title: &str) -> String {
    created_id(
        &belay()
            .args([
                "add",
                "plan",
                "--title",
                title,
                "--body",
                "## Delivery Map\n\n| ID | Goal item | Outcome / Task | State |\n| --- | --- | --- | --- |\n| T-001 | SC-001 | Create Work | not-started |\n\n## T-001\n\n- Objective: Create Work.\n",
            ])
            .current_dir(root)
            .output()
            .expect("add Plan"),
    )
}

fn link(root: &Path, plan: &str, goal: &str, relation: &str) {
    let linked = belay()
        .args(["link", plan, goal, "--relation", relation])
        .current_dir(root)
        .output()
        .expect("link Plan to Goal");
    assert_success(&linked);
}

fn create_work(root: &Path, plan: &str) -> Output {
    belay()
        .args([
            "work",
            "create",
            "--task",
            &format!("{plan}#t-001"),
            "--title",
            "Implement Contract task",
            "--body",
            "Implementation body",
            "--format",
            "id",
        ])
        .current_dir(root)
        .output()
        .expect("create Work")
}

fn contract_fixture(head: &str) -> Value {
    let mut value: Value =
        serde_json::from_str(include_str!("omnia_contract/golden/contract-v1.json"))
            .expect("parse Contract fixture");
    value["target"]["base_commit"] = Value::String(head.to_owned());
    value
        .as_object_mut()
        .expect("Contract object")
        .remove("contract_digest");
    let bytes = contract::canonical_bytes(&value).expect("canonical Contract without digest");
    value["contract_digest"] = Value::String(format!("sha256:{:x}", Sha256::digest(bytes)));
    value
}

#[test]
fn contract_apply_projection_supports_cli_work_create_without_rewriting_contract_state() {
    let (temporary, head) = initialize_git_repository();
    let contract_path = temporary.path().join("contract.json");
    fs::write(
        &contract_path,
        contract::canonical_bytes(&contract_fixture(&head)).expect("canonical Contract"),
    )
    .expect("write Contract fixture");

    let preview = belay()
        .args([
            "contract",
            "preview",
            "--file",
            contract_path.to_str().expect("Contract path is UTF-8"),
            "--source-freshness",
            "observed-current",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("preview Contract");
    assert_success(&preview);
    let preview: Value = serde_json::from_slice(&preview.stdout).expect("parse Preview");
    let preview_digest = preview["preview_digest"].as_str().expect("preview digest");
    let goal = preview["goal"]["id"].as_str().expect("Goal ID");
    let plan = preview["plan"]["id"].as_str().expect("Plan ID");

    let applied = belay()
        .args([
            "contract",
            "apply",
            "--file",
            contract_path.to_str().expect("Contract path is UTF-8"),
            "--source-freshness",
            "observed-current",
            "--approve",
            preview_digest,
            "--legacy-writers-quiesced",
        ])
        .current_dir(temporary.path())
        .output()
        .expect("apply Contract");
    assert_success(&applied);

    let plan_path = temporary
        .path()
        .join(".belay/entries/plans")
        .join(format!("{plan}.md"));
    let receipt_path = temporary
        .path()
        .join(".belay/state/contracts/OMNIA-DEMO-001/1/receipt.json");
    let plan_before = fs::read(&plan_path).expect("read Contract Plan projection");
    let receipt_before = fs::read(&receipt_path).expect("read Contract receipt");

    let created = create_work(temporary.path(), plan);
    assert_success(&created);
    let work = String::from_utf8(created.stdout)
        .expect("Work ID is UTF-8")
        .trim()
        .to_owned();
    let shown = belay()
        .args(["show", &work])
        .current_dir(temporary.path())
        .output()
        .expect("show Work");
    assert_success(&shown);
    let shown = String::from_utf8(shown.stdout).expect("shown Work is UTF-8");
    assert!(
        shown.contains(&format!("- implements {plan}#t-001")),
        "{shown}"
    );
    assert!(
        shown.contains(&format!("- fulfills {goal}#sc-001")),
        "{shown}"
    );
    assert_eq!(fs::read(plan_path).expect("reread Plan"), plan_before);
    assert_eq!(
        fs::read(receipt_path).expect("reread receipt"),
        receipt_before
    );
}

#[test]
fn work_create_deduplicates_identical_fulfills_and_implements_goal_references() {
    let (temporary, _head) = initialize_git_repository();
    let goal = add_goal(temporary.path(), "Shared Goal");
    let plan = add_plan(temporary.path(), "Shared Goal Plan");
    let goal_item = format!("{goal}#sc-001");
    link(temporary.path(), &plan, &goal_item, "fulfills");
    link(temporary.path(), &plan, &goal_item, "implements");

    let created = create_work(temporary.path(), &plan);
    assert_success(&created);
    let work = String::from_utf8(created.stdout)
        .expect("Work ID is UTF-8")
        .trim()
        .to_owned();
    let shown = belay()
        .args(["show", &work])
        .current_dir(temporary.path())
        .output()
        .expect("show Work");
    assert_success(&shown);
    let shown = String::from_utf8(shown.stdout).expect("shown Work is UTF-8");
    assert_eq!(shown.matches(&format!("- fulfills {goal_item}")).count(), 1);
}

#[test]
fn work_create_rejects_distinct_goal_targets_across_supported_relations() {
    let (temporary, _head) = initialize_git_repository();
    let goal_a = add_goal(temporary.path(), "First Goal");
    let goal_b = add_goal(temporary.path(), "Second Goal");
    let plan = add_plan(temporary.path(), "Ambiguous Goal Plan");
    link(temporary.path(), &plan, &goal_a, "implements");
    link(temporary.path(), &plan, &goal_b, "fulfills");

    let created = create_work(temporary.path(), &plan);
    assert_eq!(created.status.code(), Some(4), "{created:?}");
    assert!(
        String::from_utf8_lossy(&created.stderr).contains("has 2 Goal links"),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert_eq!(
        fs::read_dir(temporary.path().join(".belay/entries/work"))
            .expect("read Work mirrors")
            .count(),
        0
    );
}
