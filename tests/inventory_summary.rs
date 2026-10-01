use belay_trace::{
    entry::{EntryStatus, EntryType},
    inventory::{self, InventoryOptions},
    repository, store,
    summary::{self, SummaryArtifact, SummarySection},
};
use chrono::Utc;
use std::fs;
use tempfile::tempdir;

fn fixture() -> (tempfile::TempDir, repository::Repository) {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let repo = repository::initialize(dir.path()).unwrap().repository;
    (dir, repo)
}
fn artifact(repo: &repository::Repository, id: &str) -> SummaryArtifact {
    SummaryArtifact {schema_version:1,artifact_id:"authored-summary".to_owned(),artifact_revision:1,generator_version:"fixture-human-v1".to_owned(),kind:"derived-summary".to_owned(),authority:"derived-only".to_owned(),sources:vec![summary::source_binding(repo,id).unwrap()],sections:vec![SummarySection {heading:"Outcome and unresolved boundary".to_owned(),text:"Recorded outcome remains subject to original verification; unresolved questions retained.".to_owned(),source_ids:vec![id.to_owned()]}],limitations:vec!["Derived prose cannot establish verification or completion".to_owned()]}
}

#[test]
fn read_only_deterministic_report_same_human_json_findings() {
    let (_dir, repo) = fixture();
    let first = store::create(
        &repo,
        EntryType::Note,
        "first".into(),
        "exact  body\ntext".into(),
    )
    .unwrap();
    store::create(
        &repo,
        EntryType::Note,
        "second".into(),
        "exact body text".into(),
    )
    .unwrap();
    store::create(
        &repo,
        EntryType::Note,
        "third".into(),
        "similar meaning different words".into(),
    )
    .unwrap();
    let source = store::show(&repo, &first.display_id).unwrap().source_path;
    let before = fs::read(repo.belay_dir.join(source)).unwrap();
    let db_before = fs::read(repo.database_path()).unwrap();
    let options = InventoryOptions {
        now: Utc::now(),
        long_active_days: 0,
    };
    let one = inventory::collect(&repo, options.clone()).unwrap();
    let two = inventory::collect(&repo, options).unwrap();
    assert_eq!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&two).unwrap()
    );
    assert_eq!(
        one.findings
            .iter()
            .filter(|f| f.rule == "normalized-exact-duplicate")
            .count(),
        1
    );
    let human = inventory::render_report(&one);
    for finding in &one.findings {
        assert!(human.contains(&finding.id));
        assert!(human.contains(&finding.reason));
        for source in &finding.references {
            assert!(human.contains(&source.id));
        }
    }
    assert_eq!(
        before,
        fs::read(
            repo.belay_dir
                .join(store::show(&repo, &first.display_id).unwrap().source_path)
        )
        .unwrap()
    );
    assert_eq!(db_before, fs::read(repo.database_path()).unwrap());
}

#[test]
fn status_preview_stale_retry_restore_and_tamper() {
    let (_dir, repo) = fixture();
    let entry = store::create(
        &repo,
        EntryType::Work,
        "work".into(),
        "retained history".into(),
    )
    .unwrap();
    let preview =
        inventory::preview_status_change(&repo, &entry.display_id, EntryStatus::Archived).unwrap();
    let receipt = inventory::apply_status_preview(&repo, &preview).unwrap();
    let retry = inventory::apply_status_preview(&repo, &preview).unwrap();
    assert!(retry.replayed);
    assert_eq!(receipt.post_revision, retry.post_revision);
    let restore = inventory::preview_restore(&repo, &receipt).unwrap();
    inventory::apply_status_preview(&repo, &restore).unwrap();
    assert_eq!(
        store::show(&repo, &entry.display_id).unwrap().entry.status,
        EntryStatus::InProgress
    );
    assert!(inventory::preview_restore(&repo, &receipt).is_err());
    let stale =
        inventory::preview_status_change(&repo, &entry.display_id, EntryStatus::Archived).unwrap();
    store::set_status(&repo, &entry.display_id, EntryStatus::Completed).unwrap();
    assert!(inventory::apply_status_preview(&repo, &stale).is_err());
    let mut tampered = stale;
    tampered.after = EntryStatus::Abandoned;
    assert!(inventory::apply_status_preview(&repo, &tampered).is_err());
}

#[test]
fn summary_no_clobber_stale_missing_and_authority() {
    let (_dir, repo) = fixture();
    let entry = store::create(
        &repo,
        EntryType::Work,
        "work".into(),
        "original facts".into(),
    )
    .unwrap();
    let authored = artifact(&repo, &entry.display_id);
    let path = summary::save(&repo, &authored).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(summary::get(&repo, "authored-summary").unwrap().current);
    assert!(summary::save(&repo, &authored).is_err());
    assert_eq!(bytes, fs::read(&path).unwrap());
    let mut authority = authored.clone();
    authority.authority = "verified".into();
    assert!(summary::validate_artifact(&authority).is_err());
    store::set_status(&repo, &entry.display_id, EntryStatus::Completed).unwrap();
    let stale = summary::get(&repo, "authored-summary").unwrap();
    assert!(!stale.current);
    assert_eq!(stale.sources[0].state, "stale");
    assert_eq!(summary::preview_regeneration(&repo).unwrap().len(), 1);
    let original = repo
        .belay_dir
        .join(store::show(&repo, &entry.display_id).unwrap().source_path);
    fs::remove_file(original).unwrap();
    assert_eq!(
        summary::get(&repo, "authored-summary").unwrap().sources[0].state,
        "missing"
    );
}

#[test]
fn summary_detects_raw_only_source_change() {
    let (_dir, repo) = fixture();
    let entry = store::create(&repo, EntryType::Note, "note".into(), "facts".into()).unwrap();
    let authored = artifact(&repo, &entry.display_id);
    let path = repo
        .belay_dir
        .join(store::show(&repo, &entry.display_id).unwrap().source_path);
    let payload = fs::read_to_string(&path).unwrap();
    fs::write(&path, payload.replace('\n', "\r\n")).unwrap();
    assert!(!summary::inspect_artifact(&repo, &authored).unwrap().current);
}

#[test]
fn missing_original_index_drift_and_unconfirmed_decision_are_distinct() {
    let (_dir, repo) = fixture();
    let missing = store::create(&repo, EntryType::Note, "missing".into(), "facts".into()).unwrap();
    let drift = store::create(&repo, EntryType::Note, "drift".into(), "old body".into()).unwrap();
    let decision = store::create(
        &repo,
        EntryType::Decision,
        "decision".into(),
        "rationale".into(),
    )
    .unwrap();
    store::set_status(&repo, &decision.display_id, EntryStatus::Accepted).unwrap();
    fs::remove_file(
        repo.belay_dir
            .join(store::show(&repo, &missing.display_id).unwrap().source_path),
    )
    .unwrap();
    let path = repo
        .belay_dir
        .join(store::show(&repo, &drift.display_id).unwrap().source_path);
    let payload = fs::read_to_string(&path).unwrap();
    fs::write(path, payload.replace("old body", "new body")).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert!(report.findings.iter().any(|f| f.rule == "missing-original"));
    assert!(report.findings.iter().any(|f| f.rule == "index-drift"));
    assert_eq!(report.decisions[0].state, "unconfirmed");
}

#[test]
fn cache_preview_excludes_originals_and_preserves_receipts_on_rebuild() {
    let (_dir, repo) = fixture();
    let entry = store::create(&repo, EntryType::Work, "work".into(), "facts".into()).unwrap();
    let preview =
        inventory::preview_status_change(&repo, &entry.display_id, EntryStatus::Archived).unwrap();
    inventory::apply_status_preview(&repo, &preview).unwrap();
    let candidates = inventory::cache_preview(&repo);
    assert_eq!(candidates.len(), 1);
    assert!(inventory::regenerate_cache(&repo, ".belay/state").is_err());
    inventory::regenerate_cache(&repo, &candidates[0].path).unwrap();
    assert!(
        inventory::apply_status_preview(&repo, &preview)
            .unwrap()
            .replayed
    );
}

#[test]
fn scoped_decisions_require_adoption_and_matching_scopes_only_propose_conflicts() {
    use belay_trace::{
        entry::{LinkRelation, MetadataValue},
        evidence::{self, RecordInput},
        markdown, reconcile,
    };
    let (_dir, repo) = fixture();
    let mut ids = Vec::new();
    for name in ["first-decision", "second-decision"] {
        let mut entry = store::create(
            &repo,
            EntryType::Decision,
            name.into(),
            "Distinct rationale; scope alone cannot prove contradiction".into(),
        )
        .unwrap();
        entry.metadata.insert(
            "scope".into(),
            MetadataValue::String("staging import".into()),
        );
        let path = repo
            .belay_dir
            .join(store::show(&repo, &entry.display_id).unwrap().source_path);
        fs::write(path, markdown::render(&entry).unwrap()).unwrap();
        reconcile::synchronize(&repo, Some(&entry.display_id), None).unwrap();
        store::set_status(&repo, &entry.display_id, EntryStatus::Accepted).unwrap();
        evidence::record(
            &repo,
            RecordInput {
                kind: "human-approval".into(),
                verdict: "pass".into(),
                commit_sha: Some("unknown".into()),
                captured_at: None,
                source: "fixture human acceptance".into(),
                issuer: "fixture".into(),
                summary: "Adopt staging import scope".into(),
                detail: serde_json::json!({}),
                verifies: vec![entry.display_id.clone()],
            },
        )
        .unwrap();
        ids.push(entry.display_id);
    }
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert!(
        report
            .decisions
            .iter()
            .all(|d| d.state == "conflict-candidate" && d.evidence_ids.len() == 1)
    );
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "decision-scope-conflict")
        .unwrap();
    assert_eq!(finding.class, "semantic-review-proposal");
    store::link(&repo, &ids[1], &ids[0], LinkRelation::Supersedes).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert_eq!(
        report
            .decisions
            .iter()
            .find(|d| d.id == ids[0])
            .unwrap()
            .state,
        "replaced"
    );
    assert_eq!(
        report
            .decisions
            .iter()
            .find(|d| d.id == ids[1])
            .unwrap()
            .state,
        "explicit-scope"
    );
}

#[test]
fn evidence_sources_bind_and_fixed_time_detects_stale_and_invalid_task_map() {
    use belay_trace::evidence::{self, RecordInput};
    let (_dir, repo) = fixture();
    let work = store::create(&repo, EntryType::Work, "work".into(), "facts".into()).unwrap();
    store::create(&repo,EntryType::Plan,"bad-plan".into(),"## Delivery Map\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n| T-001 | SC-999 | task | worker | done | None |\n".into()).unwrap();
    let record = evidence::record(
        &repo,
        RecordInput {
            kind: "test".into(),
            verdict: "fail".into(),
            commit_sha: Some("unknown".into()),
            captured_at: Some("2026-01-01T00:00:00+00:00".into()),
            source: "local fixture".into(),
            issuer: "fixture".into(),
            summary: "Prior failure retained".into(),
            detail: serde_json::json!({}),
            verifies: vec![work.display_id],
        },
    )
    .unwrap();
    let binding = summary::source_binding(&repo, &record.display_id).unwrap();
    assert_eq!(binding.revision, 1);
    let options = InventoryOptions {
        now: "2026-09-30T00:00:00+00:00".parse().unwrap(),
        long_active_days: 90,
    };
    let report = inventory::collect(&repo, options).unwrap();
    assert!(report.evidence[0].freshness.starts_with("stale"));
    assert_eq!(report.evidence[0].verdict, "fail");
    assert!(report.findings.iter().any(|f| f.rule == "invalid-task-map"));
}

#[test]
fn loose_original_missing_from_index_is_visible_without_mutation() {
    let (_dir, repo) = fixture();
    let entry = store::create(
        &repo,
        EntryType::Note,
        "unindexed".into(),
        "original outside index".into(),
    )
    .unwrap();
    let path = repo
        .belay_dir
        .join(store::show(&repo, &entry.display_id).unwrap().source_path);
    // Simulate a loose published original whose index update did not happen.
    let connection = rusqlite::Connection::open(repo.database_path()).unwrap();
    connection
        .execute(
            "DELETE FROM entries WHERE display_id=?1",
            [&entry.display_id],
        )
        .unwrap();
    drop(connection);
    let before = fs::read(&path).unwrap();
    let db = fs::read(repo.database_path()).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert_eq!(report.counts["indexed-entries"], 0);
    assert_eq!(report.counts["original-entries"], 1);
    assert_eq!(report.counts["unindexed-original-files"], 1);
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "unindexed-original")
        .unwrap();
    assert_eq!(finding.references[0].id, entry.display_id);
    assert_eq!(finding.references[0].sha256, summary::digest(&before));
    assert!(!report.entries[0].indexed);
    assert_eq!(before, fs::read(path).unwrap());
    assert_eq!(db, fs::read(repo.database_path()).unwrap());
}

/// Measurements are informational, not a performance-adoption claim.
#[test]
fn inventory_large_fixture_measurement() {
    let (_dir, repo) = fixture();
    for index in 0..1000 {
        store::create(
            &repo,
            EntryType::Note,
            format!("fixture-{index}"),
            format!("Original payload {index}"),
        )
        .unwrap();
    }
    let start = std::time::Instant::now();
    let report = inventory::collect(
        &repo,
        InventoryOptions {
            now: Utc::now(),
            long_active_days: 90,
        },
    )
    .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(report.entries.len(), 1000);
    let human = inventory::render_report(&report);
    let json = serde_json::to_string(&report).unwrap();
    assert!(
        report
            .findings
            .iter()
            .all(|f| human.contains(&f.id) && json.contains(&f.id))
    );
    println!(
        "inventory-large-fixture entries=1000 elapsed_us={} human_bytes={} json_bytes={} findings={}",
        elapsed.as_micros(),
        human.len(),
        json.len(),
        report.findings.len()
    );
}

fn adopted_decision(repo: &repository::Repository, name: &str) -> String {
    use belay_trace::{
        entry::MetadataValue,
        evidence::{self, RecordInput},
        markdown, reconcile,
    };
    let mut entry =
        store::create(repo, EntryType::Decision, name.into(), "rationale".into()).unwrap();
    entry
        .metadata
        .insert("scope".into(), MetadataValue::String(name.into()));
    let path = repo
        .belay_dir
        .join(store::show(repo, &entry.display_id).unwrap().source_path);
    fs::write(path, markdown::render(&entry).unwrap()).unwrap();
    reconcile::synchronize(repo, Some(&entry.display_id), None).unwrap();
    store::set_status(repo, &entry.display_id, EntryStatus::Accepted).unwrap();
    evidence::record(
        repo,
        RecordInput {
            kind: "human-approval".into(),
            verdict: "pass".into(),
            commit_sha: Some("unknown".into()),
            captured_at: None,
            source: "fixture human acceptance".into(),
            issuer: "fixture".into(),
            summary: "Adopt fixture scope".into(),
            detail: serde_json::json!({}),
            verifies: vec![entry.display_id.clone()],
        },
    )
    .unwrap();
    entry.display_id
}

#[test]
fn changed_deleted_or_invalid_decision_cannot_reuse_indexed_adoption() {
    use belay_trace::markdown;
    for change in ["rejected", "deleted", "invalid"] {
        let (_dir, repo) = fixture();
        let id = adopted_decision(&repo, "adopted-decision");
        let before = inventory::collect(&repo, InventoryOptions::default()).unwrap();
        assert_eq!(before.decisions[0].state, "explicit-scope");
        let indexed_binding = before.entries[0].source.clone();
        let path = repo
            .belay_dir
            .join(store::show(&repo, &id).unwrap().source_path);
        let payload = fs::read_to_string(&path).unwrap();
        match change {
            "rejected" => {
                let mut original = markdown::parse(&payload).unwrap();
                original.status = EntryStatus::Rejected;
                original.revision += 1;
                fs::write(&path, markdown::render(&original).unwrap()).unwrap();
            }
            "deleted" => fs::remove_file(&path).unwrap(),
            "invalid" => fs::write(&path, "invalid current original").unwrap(),
            _ => unreachable!(),
        }
        let db = fs::read(repo.database_path()).unwrap();
        let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
        assert_eq!(report.decisions[0].id, id);
        assert_eq!(report.decisions[0].state, "unconfirmed", "{change}");
        if change == "rejected" {
            let current = fs::read(&path).unwrap();
            let source = &report.entries[0].source;
            assert_eq!(report.entries[0].status, "rejected");
            assert_eq!(source.revision, indexed_binding.revision + 1);
            assert_eq!(source.sha256, summary::digest(&current));
            assert_eq!(report.counts["decision"], 1);
            assert_eq!(report.statuses["rejected"], 1);
            assert!(!report.statuses.contains_key("accepted"));
            let drift = report
                .findings
                .iter()
                .find(|f| f.rule == "index-drift")
                .unwrap();
            assert_eq!(drift.references[0], *source);
        } else {
            let rule = if change == "deleted" {
                "missing-original"
            } else {
                "invalid-original"
            };
            assert!(report.findings.iter().any(|f| f.rule == rule));
            assert_eq!(report.counts["original-entries"], 0);
            assert!(!report.counts.contains_key("decision"));
            assert!(report.statuses.is_empty());
            assert_eq!(report.entries[0].source.revision, indexed_binding.revision);
            if change == "invalid" {
                assert_ne!(
                    report.entries[0].source.sha256,
                    summary::digest(&fs::read(&path).unwrap())
                );
            }
        }
        assert_eq!(db, fs::read(repo.database_path()).unwrap());
    }
}

#[test]
fn original_revision_only_drift_has_current_binding_and_unconfirmed_adoption() {
    use belay_trace::markdown;
    let (_dir, repo) = fixture();
    let id = adopted_decision(&repo, "revision-drift");
    let path = repo
        .belay_dir
        .join(store::show(&repo, &id).unwrap().source_path);
    let mut original = markdown::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    let canonical_before = markdown::content_hash(&original).unwrap();
    original.revision += 1;
    let payload = markdown::render(&original).unwrap();
    assert_eq!(canonical_before, markdown::content_hash(&original).unwrap());
    fs::write(path, &payload).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert_eq!(report.decisions[0].state, "unconfirmed");
    assert_eq!(report.entries[0].source.revision, original.revision);
    assert_eq!(
        report.entries[0].source.sha256,
        summary::digest(payload.as_bytes())
    );
    let drift = report
        .findings
        .iter()
        .find(|f| f.rule == "index-drift")
        .unwrap();
    assert_eq!(drift.references[0], report.entries[0].source);
    assert!(drift.reason.contains("revision"));
}

#[test]
fn stale_or_missing_successor_cannot_replace_current_decision() {
    use belay_trace::{entry::LinkRelation, markdown};
    for change in ["rejected", "deleted", "revision-only"] {
        let (_dir, repo) = fixture();
        let predecessor = adopted_decision(&repo, "predecessor");
        let successor = adopted_decision(&repo, "successor");
        store::link(&repo, &successor, &predecessor, LinkRelation::Supersedes).unwrap();
        let before = inventory::collect(&repo, InventoryOptions::default()).unwrap();
        assert_eq!(
            before
                .decisions
                .iter()
                .find(|d| d.id == predecessor)
                .unwrap()
                .state,
            "replaced"
        );
        let path = repo
            .belay_dir
            .join(store::show(&repo, &successor).unwrap().source_path);
        if change == "deleted" {
            fs::remove_file(path).unwrap();
        } else {
            let mut original = markdown::parse(&fs::read_to_string(&path).unwrap()).unwrap();
            original.revision += 1;
            if change == "rejected" {
                original.status = EntryStatus::Rejected;
            }
            fs::write(path, markdown::render(&original).unwrap()).unwrap();
        }
        let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
        let previous = report
            .decisions
            .iter()
            .find(|d| d.id == predecessor)
            .unwrap();
        assert_eq!(previous.state, "explicit-scope", "{change}");
        assert!(previous.successor_ids.is_empty());
        assert_eq!(
            report
                .decisions
                .iter()
                .find(|d| d.id == successor)
                .unwrap()
                .state,
            "unconfirmed"
        );
    }
}

fn remove_indexed_entry(repo: &repository::Repository, id: &str) {
    let connection = rusqlite::Connection::open(repo.database_path()).unwrap();
    connection
        .execute("DELETE FROM entries WHERE display_id=?1", [id])
        .unwrap();
}

#[test]
fn unindexed_live_original_retains_type_status_age_and_finding_identity() {
    let (_dir, repo) = fixture();
    let note = store::create(
        &repo,
        EntryType::Note,
        "old-note".into(),
        "old live original".into(),
    )
    .unwrap();

    let path = repo
        .belay_dir
        .join(store::show(&repo, &note.display_id).unwrap().source_path);
    let options = InventoryOptions {
        now: note.updated_at.parse::<chrono::DateTime<Utc>>().unwrap()
            + chrono::Duration::days(100),
        long_active_days: 90,
    };
    let indexed = inventory::collect(&repo, options.clone()).unwrap();
    let prior_age = indexed
        .findings
        .iter()
        .find(|f| f.rule == "long-active")
        .unwrap();
    remove_indexed_entry(&repo, &note.display_id);
    let db = fs::read(repo.database_path()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let report = inventory::collect(&repo, options).unwrap();
    assert_eq!(report.counts["indexed-entries"], 0);
    assert_eq!(report.counts["note"], 1);
    assert_eq!(report.statuses[&note.status.to_string()], 1);
    let age = report
        .findings
        .iter()
        .find(|f| f.rule == "long-active")
        .unwrap();
    assert_eq!(age.id, prior_age.id);
    assert_eq!(age.reason, prior_age.reason);
    assert_eq!(age.references, prior_age.references);
    assert_eq!(age.references[0].sha256, summary::digest(&bytes));
    assert!(!report.entries[0].indexed);
    assert_eq!(db, fs::read(repo.database_path()).unwrap());
    assert_eq!(bytes, fs::read(path).unwrap());
}

#[test]
fn unindexed_plan_runs_original_task_map_validation() {
    let (_dir, repo) = fixture();
    let plan = store::create(&repo, EntryType::Plan, "loose-bad-plan".into(),
        "## Delivery Map\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n| T-001 | SC-999 | task | worker | done | None |\n".into()).unwrap();
    let before = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    let expected = before
        .findings
        .iter()
        .filter(|f| f.rule == "invalid-task-map")
        .map(|f| f.id.clone())
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());
    remove_indexed_entry(&repo, &plan.display_id);
    let db = fs::read(repo.database_path()).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    let actual = report
        .findings
        .iter()
        .filter(|f| f.rule == "invalid-task-map")
        .map(|f| f.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(report.counts["plan"], 1);
    assert!(
        report
            .findings
            .iter()
            .filter(|f| f.rule == "invalid-task-map")
            .all(|f| f.references[0] == report.entries[0].source)
    );
    assert_eq!(db, fs::read(repo.database_path()).unwrap());
}

#[test]
fn plan_graph_uses_raw_goal_even_when_goal_and_plan_are_unindexed() {
    use belay_trace::entry::LinkRelation;

    let (_dir, repo) = fixture();
    let goal = store::create(
        &repo,
        EntryType::Goal,
        "raw-goal".into(),
        "## Success Criteria\n- [SC-001] Original remains available.\n".into(),
    )
    .unwrap();
    let goal_ref = format!("{}#sc-001", goal.display_id);
    let plan = store::create(&repo, EntryType::Plan, "raw-plan".into(), format!(
        "## Delivery Map\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n| T-001 | {goal_ref} | Read original | worker | not-started | fixture |\n\n## T-001\n- Objective: Read original.\n- Scope: Original.\n- Steps: Read.\n- Acceptance: Available.\n- Verification: Fixture.\n"
    )).unwrap();
    store::link(&repo, &plan.display_id, &goal_ref, LinkRelation::Implements).unwrap();
    let goal_path = repo
        .belay_dir
        .join(store::show(&repo, &goal.display_id).unwrap().source_path);
    let plan_path = repo
        .belay_dir
        .join(store::show(&repo, &plan.display_id).unwrap().source_path);
    let goal_bytes = fs::read(&goal_path).unwrap();
    let plan_bytes = fs::read(&plan_path).unwrap();
    let before = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert!(!before.findings.iter().any(|f| f.rule == "invalid-task-map"));

    remove_indexed_entry(&repo, &goal.display_id);
    for remove_plan in [false, true] {
        if remove_plan {
            remove_indexed_entry(&repo, &plan.display_id);
        }
        let db = fs::read(repo.database_path()).unwrap();
        let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "unindexed-original")
        );
        assert!(
            !report.findings.iter().any(|f| matches!(
                f.rule.as_str(),
                "invalid-task-map"
                    | "missing-reference"
                    | "missing-body-reference"
                    | "invalid-task-reference"
                    | "invalid-body-fragment"
            )),
            "{:?}",
            report.findings
        );
        assert_eq!(fs::read(repo.database_path()).unwrap(), db);
        assert_eq!(fs::read(&goal_path).unwrap(), goal_bytes);
        assert_eq!(fs::read(&plan_path).unwrap(), plan_bytes);
    }

    // Readable but invalid fragments, and genuinely missing raw targets,
    // must still be reported without relying on the index.
    fs::write(
        &goal_path,
        String::from_utf8(goal_bytes)
            .unwrap()
            .replace("SC-001", "SC-002"),
    )
    .unwrap();
    let invalid = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert!(
        invalid
            .findings
            .iter()
            .any(|f| f.rule == "invalid-task-reference")
    );
    assert!(
        invalid
            .findings
            .iter()
            .any(|f| f.rule == "invalid-body-fragment")
    );
    fs::remove_file(goal_path).unwrap();
    let missing = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert!(
        missing
            .findings
            .iter()
            .any(|f| f.rule == "missing-reference")
    );
    assert!(
        missing
            .findings
            .iter()
            .any(|f| f.rule == "missing-body-reference")
    );
}

#[test]
fn packed_original_missing_from_index_retains_counts_and_provenance() {
    let (_dir, repo) = fixture();
    let work = store::create(
        &repo,
        EntryType::Work,
        "packed fixture".into(),
        "Finished bounded fixture.".into(),
    )
    .unwrap();
    store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
    let before = summary::source_binding(&repo, &work.display_id).unwrap();
    let preview =
        belay_trace::pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap();
    belay_trace::pack::apply(&repo, &preview).unwrap();
    let connection = belay_trace::database::open(&repo.database_path()).unwrap();
    connection
        .execute(
            "DELETE FROM entries WHERE display_id=?1",
            [&work.display_id],
        )
        .unwrap();
    drop(connection);
    let database_before = fs::read(repo.database_path()).unwrap();
    let report = inventory::collect(&repo, InventoryOptions::default()).unwrap();
    assert_eq!(report.counts["work"], 1);
    assert_eq!(report.statuses["completed"], 1);
    assert_eq!(report.counts["unindexed-packed-entries"], 1);
    assert_eq!(report.entries[0].source, before);
    assert!(!report.entries[0].indexed);
    assert_eq!(database_before, fs::read(repo.database_path()).unwrap());
}
