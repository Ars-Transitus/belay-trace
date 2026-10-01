use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_belay"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("run belay")
}

fn success(root: &Path, args: &[&str]) -> String {
    let output = run(root, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn setup() -> (TempDir, String) {
    let temp = tempfile::tempdir().unwrap();
    success(temp.path(), &["init"]);
    let created = success(
        temp.path(),
        &[
            "add",
            "work",
            "--title",
            "cli-fixture",
            "--body",
            "Outcome: fixed fixture passed. Constraint: retain exact originals. Unknown: external logs are only references.",
        ],
    );
    let id = created.split_whitespace().last().unwrap().to_owned();
    (temp, id)
}

#[test]
fn inventory_cli_is_read_only_and_status_restore_is_explicit() {
    let (temp, id) = setup();
    let root = temp.path();
    let database = root.join(".belay/state/belay.sqlite");
    let before = fs::read(&database).unwrap();
    let report: Value = serde_json::from_str(&success(
        root,
        &[
            "inventory",
            "--format",
            "json",
            "--now",
            "2026-09-30T12:00:00Z",
        ],
    ))
    .unwrap();
    assert!(report["findings"].is_array());
    assert_eq!(before, fs::read(&database).unwrap());
    let preview = success(
        root,
        &["inventory", "preview", &id, "--status", "completed"],
    );
    assert!(success(root, &["show", &id]).contains("Status: in-progress"));
    let path = root.join("preview.json");
    fs::write(&path, preview).unwrap();
    let receipt = success(
        root,
        &["inventory", "apply", "--file", path.to_str().unwrap()],
    );
    assert!(success(root, &["show", &id]).contains("Status: completed"));
    success(
        root,
        &["inventory", "apply", "--file", path.to_str().unwrap()],
    );
    let receipt_path = root.join("receipt.json");
    fs::write(&receipt_path, receipt).unwrap();
    let restoration = success(
        root,
        &[
            "inventory",
            "restore",
            "--file",
            receipt_path.to_str().unwrap(),
        ],
    );
    assert!(success(root, &["show", &id]).contains("Status: completed"));
    fs::write(&path, restoration).unwrap();
    success(
        root,
        &["inventory", "apply", "--file", path.to_str().unwrap()],
    );
    assert!(success(root, &["show", &id]).contains("Status: in-progress"));
}

#[test]
fn summary_cli_preserves_provenance_and_marks_changed_sources_stale() {
    let (temp, id) = setup();
    let root = temp.path();
    success(root, &["status", &id, "completed"]);
    let bindings: Value =
        serde_json::from_str(&success(root, &["lifecycle", "summary", "sources", &id])).unwrap();
    let artifact = json!({
        "schema_version": 1,
        "artifact_id": "cli-summary",
        "artifact_revision": 1,
        "generator_version": "authored-fixture-v1",
        "kind": "derived-summary",
        "authority": "derived-only",
        "sources": bindings,
        "sections": [
            {"heading": "Outcomes", "text": "The fixed fixture passed; this does not verify a Goal.", "source_ids": [id]},
            {"heading": "Constraints", "text": "Retain exact originals.", "source_ids": [id]},
            {"heading": "Unresolved", "text": "External logs remain references only.", "source_ids": [id]}
        ],
        "limitations": ["Authored fixture, not a model quality evaluation."]
    });
    let file = root.join("summary.json");
    fs::write(&file, serde_json::to_vec(&artifact).unwrap()).unwrap();
    success(
        root,
        &[
            "lifecycle",
            "summary",
            "store",
            "--file",
            file.to_str().unwrap(),
        ],
    );
    let compiled = success(root, &["context", "compile", &id, "--budget", "10000"]);
    assert!(compiled.contains("Derived summary cli-summary"));
    assert!(compiled.contains("not verification"));
    assert!(compiled.contains("External logs remain references only."));
    let pack_preview = root.join("summary-pack-preview.json");
    fs::write(
        &pack_preview,
        success(root, &["lifecycle", "pack", "preview", "--id", &id]),
    )
    .unwrap();
    success(
        root,
        &[
            "lifecycle",
            "pack",
            "apply",
            "--file",
            pack_preview.to_str().unwrap(),
        ],
    );
    success(root, &["rebuild"]);
    let packed_context = success(root, &["context", "compile", &id, "--budget", "10000"]);
    assert!(packed_context.contains("Derived summary cli-summary"));
    let read = || -> Value {
        serde_json::from_str(&success(
            root,
            &["lifecycle", "summary", "show", "cli-summary"],
        ))
        .unwrap()
    };
    assert_eq!(read()["current"], true);
    success(root, &["status", &id, "in-progress"]);
    assert_eq!(read()["current"], false);
    let compiled = success(root, &["context", "compile", &id, "--budget", "10000"]);
    assert!(!compiled.contains("Derived summary cli-summary"));
    assert!(compiled.contains("retain exact originals"), "{compiled}");
    assert!(
        read()["authority"]
            .as_str()
            .unwrap()
            .starts_with("derived-only")
    );
    let config_before = fs::read(root.join(".belay/config.toml")).unwrap();
    success(root, &["lifecycle", "cache"]);
    assert_eq!(
        config_before,
        fs::read(root.join(".belay/config.toml")).unwrap()
    );
}

#[test]
fn lifecycle_cli_preserves_sources_across_pack_rebuild_restore_and_reopen() {
    let (temp, work) = setup();
    let root = temp.path();
    success(root, &["status", &work, "completed"]);
    let recorded = success(
        root,
        &[
            "verify",
            "record",
            "--kind",
            "test",
            "--verdict",
            "pass",
            "--source",
            "isolated-fixture",
            "--summary",
            "bounded fixture only",
            "--verifies",
            &work,
        ],
    );
    let evidence = recorded
        .split_whitespace()
        .find(|word| word.starts_with("EVD-"))
        .unwrap()
        .trim_end_matches('.')
        .to_owned();
    let evidence_payload = |text: String| -> String {
        text.lines()
            .filter(|line| !line.starts_with("Location:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let before = evidence_payload(success(root, &["show", &evidence]));
    let preview = success(
        root,
        &[
            "lifecycle",
            "pack",
            "preview",
            "--id",
            &work,
            "--id",
            &evidence,
        ],
    );
    let preview_path = root.join("pack-preview.json");
    fs::write(&preview_path, preview).unwrap();
    let packed: Value = serde_json::from_str(&success(
        root,
        &[
            "lifecycle",
            "pack",
            "apply",
            "--file",
            preview_path.to_str().unwrap(),
        ],
    ))
    .unwrap();
    assert_eq!(packed["records"], 2);
    let historical: Value = serde_json::from_str(&success(
        root,
        &["lifecycle", "pack", "original", &work, "--revision", "2"],
    ))
    .unwrap();
    assert_eq!(historical["revision"], 2);
    assert_eq!(historical["display_id"], work);
    assert!(
        historical["raw_payload"]
            .as_str()
            .unwrap()
            .contains("completed")
    );
    assert_eq!(
        before,
        evidence_payload(success(root, &["show", &evidence]))
    );
    assert!(success(root, &["show", &work]).contains("Status: completed"));
    success(root, &["rebuild"]);
    success(root, &["verify", "status", &work]);
    success(root, &["coverage", "--format", "json"]);
    success(root, &["export", "json", "--output", "export.json"]);
    success(root, &["inventory", "--format", "json"]);
    success(root, &["context", "compile", &work, "--budget", "10000"]);
    success(
        root,
        &[
            "lifecycle",
            "pack",
            "recover",
            packed["receipt"].as_str().unwrap(),
        ],
    );
    success(root, &["status", &work, "in-progress"]);
    success(
        root,
        &[
            "lifecycle",
            "pack",
            "restore",
            packed["pack_hash"].as_str().unwrap(),
        ],
    );
    assert!(success(root, &["show", &work]).contains("Status: in-progress"));
    assert_eq!(
        before,
        evidence_payload(success(root, &["show", &evidence]))
    );
    // Corruption must not turn cached assertions into successful verification.
    let pack_path = root
        .join(".belay/packs")
        .join(format!("{}.json", packed["pack_hash"].as_str().unwrap()));
    fs::write(&pack_path, b"{invalid").unwrap();
    for args in [
        vec!["coverage", "--format", "json"],
        vec!["export", "json", "--output", "export.json"],
        vec!["verify", "status", work.as_str()],
        vec!["context", "compile", work.as_str(), "--budget", "10000"],
    ] {
        assert!(
            !run(root, &args).status.success(),
            "corrupt pack unexpectedly accepted by {args:?}"
        );
    }
}
