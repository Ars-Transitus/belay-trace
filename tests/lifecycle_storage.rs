use belay_trace::{
    entry::{EntryStatus, EntryType},
    evidence::{self, RecordInput},
    lifecycle, pack, reconcile, repository, store,
};
use serde_json::json;
use std::fs;
fn fixture() -> (tempfile::TempDir, repository::Repository, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository::initialize(dir.path()).unwrap().repository;
    let goal = store::create(
        &repo,
        EntryType::Goal,
        "test-goal".into(),
        "## Summary\nfixture".into(),
    )
    .unwrap();
    (dir, repo, goal.display_id)
}
fn record(
    repo: &repository::Repository,
    target: &str,
    summary: &str,
) -> evidence::RecordedEvidence {
    evidence::record(
        repo,
        RecordInput {
            kind: "test".into(),
            verdict: "pass".into(),
            commit_sha: Some("abc".into()),
            captured_at: Some("2026-09-01T12:00:00Z".into()),
            source: "fixture".into(),
            issuer: "test".into(),
            summary: summary.into(),
            detail: json!({"raw":true}),
            verifies: vec![target.into()],
        },
    )
    .unwrap()
}
#[test]
fn independent_ids_and_schema_upgrade() {
    let (_d, repo, id) = fixture();
    let a = record(&repo, &id, "a");
    let b = record(&repo, &id, "b");
    assert_ne!(a.display_id, b.display_id);
    assert_eq!(a.display_id.len(), 36);
    assert!(a.indexed());
    assert_eq!(
        belay_trace::config::Config::load(&repo.belay_dir.join("config.toml"))
            .unwrap()
            .schema_version,
        2
    );
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
}
#[test]
fn saved_unindexed_repairs_without_new_id() {
    let (_d, repo, id) = fixture();
    let conn = belay_trace::database::open(&repo.database_path()).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_evidence BEFORE INSERT ON evidence BEGIN SELECT RAISE(ABORT,'fixture index failure'); END;").unwrap();
    let saved = record(&repo, &id, "saved");
    assert!(!saved.indexed());
    assert!(
        saved
            .index_error
            .as_ref()
            .unwrap()
            .contains("fixture index failure")
    );
    assert_eq!(
        evidence::show(&repo, &saved.display_id)
            .unwrap()
            .record
            .summary,
        "saved"
    );
    assert_eq!(evidence::status(&repo, &id).unwrap().records.len(), 1);
    assert!(evidence::validate_index_sources(&repo).is_err());
    conn.execute_batch("DROP TRIGGER reject_evidence").unwrap();
    drop(conn);
    evidence::reindex(&repo).unwrap();
    evidence::validate_index_sources(&repo).unwrap();
}
#[test]
fn evidence_compaction_restore_and_rebuild() {
    let (_d, repo, id) = fixture();
    let a = record(&repo, &id, "a");
    let b = record(&repo, &id, "b");
    let before = fs::read(
        repo.evidence_path()
            .join("records")
            .join(format!("{}.json", a.display_id)),
    )
    .unwrap();
    let preview = pack::preview(&repo, &[]).unwrap();
    let packed = pack::apply(&repo, &preview).unwrap();
    assert_eq!(packed.retired, 2);
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
    assert_eq!(pack::recover(&repo, &packed.receipt).unwrap().retired, 0);
    fs::remove_file(repo.database_path()).unwrap();
    let rebuilt = reconcile::rebuild(&repo).unwrap();
    assert_eq!(rebuilt.evidence, 2);
    evidence::validate_index_sources(&repo).unwrap();
    assert_eq!(
        evidence::show(&repo, &b.display_id).unwrap().record.summary,
        "b"
    );
    pack::restore(&repo, &packed.pack_hash, None).unwrap();
    assert_eq!(
        fs::read(
            repo.evidence_path()
                .join("records")
                .join(format!("{}.json", a.display_id))
        )
        .unwrap(),
        before
    );
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
    assert_eq!(pack::apply(&repo, &preview).unwrap().retired, 0);
}
#[test]
fn live_work_reopens_without_overwriting_pack_revision() {
    let (_d, repo, _id) = fixture();
    let work = store::create(
        &repo,
        EntryType::Work,
        "test-work".into(),
        "original body".into(),
    )
    .unwrap();
    store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
    let terminal = store::show(&repo, &work.display_id).unwrap().entry;
    let original = fs::read(
        repo.belay_dir
            .join("entries/work")
            .join(format!("{}.md", work.display_id)),
    )
    .unwrap();
    let preview = pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap();
    let packed = pack::apply(&repo, &preview).unwrap();
    fs::remove_file(repo.database_path()).unwrap();
    reconcile::rebuild(&repo).unwrap();
    assert_eq!(
        store::show(&repo, &work.display_id).unwrap().entry.revision,
        terminal.revision
    );
    store::set_status(&repo, &work.display_id, EntryStatus::InProgress).unwrap();
    let live = store::show(&repo, &work.display_id).unwrap().entry;
    assert_eq!(live.revision, terminal.revision + 1);
    assert_eq!(live.status, EntryStatus::InProgress);
    let restored = pack::restore(&repo, &packed.pack_hash, None).unwrap();
    assert_eq!(restored.preserved.len(), 1);
    assert_eq!(
        store::show(&repo, &work.display_id).unwrap().entry.revision,
        live.revision
    );
    assert_eq!(
        pack::read_entries(&repo).unwrap()[0].raw_payload.as_bytes(),
        original
    );
    assert_eq!(
        pack::resolve_original(&repo, &work.display_id, terminal.revision)
            .unwrap()
            .raw_payload
            .as_bytes(),
        original
    );
    let live_path = repo
        .belay_dir
        .join(store::show(&repo, &work.display_id).unwrap().source_path);
    let live_bytes = fs::read(&live_path).unwrap();
    let synced = reconcile::synchronize(&repo, None, None).unwrap();
    assert!(synced.failures.is_empty(), "{:?}", synced.failures);
    reconcile::rebuild(&repo).unwrap();
    assert_eq!(
        store::show(&repo, &work.display_id).unwrap().entry.revision,
        live.revision
    );
    assert_eq!(fs::read(live_path).unwrap(), live_bytes);
}
#[test]
fn stale_preview_preserves_changed_original() {
    let (_d, repo, id) = fixture();
    let a = record(&repo, &id, "a");
    let preview = pack::preview(&repo, &[]).unwrap();
    let path = repo
        .evidence_path()
        .join("records")
        .join(format!("{}.json", a.display_id));
    let changed = fs::read_to_string(&path)
        .unwrap()
        .replace("\"summary\":\"a\"", "\"summary\":\"changed\"");
    fs::write(&path, &changed).unwrap();
    assert!(pack::apply(&repo, &preview).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), changed);
    assert!(!repo.belay_dir.join("packs").exists());
}
#[test]
fn raw_hash_conflict_and_identical_placement_dedup() {
    let (_d, repo, id) = fixture();
    let a = record(&repo, &id, "a");
    let path = repo
        .evidence_path()
        .join("records")
        .join(format!("{}.json", a.display_id));
    let preview = pack::preview(&repo, &[]).unwrap();
    let packed = pack::apply(&repo, &preview).unwrap();
    pack::restore(&repo, &packed.pack_hash, None).unwrap();
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 1);
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("\"summary\":\"a\"", "\"summary\":\"different\""),
    )
    .unwrap();
    assert!(
        evidence::read_located_mirrors(&repo)
            .unwrap_err()
            .to_string()
            .contains("conflicting")
    );
}
#[test]
fn corrupt_pack_and_missing_pack_fail_closed() {
    let (_d, repo, id) = fixture();
    record(&repo, &id, "a");
    let packed = pack::apply(&repo, &pack::preview(&repo, &[]).unwrap()).unwrap();
    let path = repo
        .belay_dir
        .join("packs")
        .join(format!("{}.json", packed.pack_hash));
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"{}").unwrap();
    assert!(
        evidence::read_located_mirrors(&repo)
            .unwrap_err()
            .to_string()
            .contains("hash mismatch")
    );
    fs::write(&path, &original).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(
        evidence::read_located_mirrors(&repo)
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
}
#[test]
fn new_writer_after_preview_is_not_retired() {
    let (_d, repo, id) = fixture();
    record(&repo, &id, "a");
    let preview = pack::preview(&repo, &[]).unwrap();
    let next = record(&repo, &id, "b");
    let packed = pack::apply(&repo, &preview).unwrap();
    assert_eq!(packed.retired, 1);
    assert!(
        repo.evidence_path()
            .join("records")
            .join(format!("{}.json", next.display_id))
            .exists()
    );
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
}
#[test]
fn active_work_and_goals_never_pack() {
    let (_d, repo, id) = fixture();
    let active = store::create(&repo, EntryType::Work, "active".into(), "active".into()).unwrap();
    assert!(pack::preview(&repo, &[id]).is_err());
    assert!(pack::preview(&repo, &[active.display_id]).is_err());
    assert!(pack::preview(&repo, &[]).unwrap().records.is_empty());
}
#[test]
fn writer_lock_is_recursive_and_thread_exclusive() {
    let (_d, repo, _id) = fixture();
    let outer = lifecycle::writer_lock(&repo).unwrap();
    let inner = lifecycle::writer_lock(&repo).unwrap();
    drop(inner);
    let cloned = repo.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _guard = lifecycle::writer_lock(&cloned).unwrap();
        tx.send(()).unwrap();
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );
    drop(outer);
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    handle.join().unwrap();
}

#[test]
fn unavailable_index_preserves_record_from_original_target() {
    let (_dir, repo, goal) = fixture();
    fs::remove_file(repo.database_path()).unwrap();
    let saved = record(&repo, &goal, "db unavailable");
    assert!(!saved.indexed());
    assert_eq!(
        evidence::show(&repo, &saved.display_id)
            .unwrap()
            .record
            .summary,
        "db unavailable"
    );
    reconcile::rebuild(&repo).unwrap();
    evidence::validate_index_sources(&repo).unwrap();
}

#[test]
fn legacy_full_and_partial_month_preserve_exact_bytes() {
    for partial in [true, false] {
        let (_dir, repo, goal) = fixture();
        let a = record(&repo, &goal, "a");
        let b = record(&repo, &goal, "b");
        let records = repo.evidence_path().join("records");
        let ap = records.join(format!("{}.json", a.display_id));
        let bp = records.join(format!("{}.json", b.display_id));
        let raw = format!(
            "\r\n{}\r\n\r\n{}\r\n",
            fs::read_to_string(&ap).unwrap(),
            fs::read_to_string(&bp).unwrap()
        );
        let month = repo.evidence_path().join("2026-09.ndjson");
        fs::write(&month, &raw).unwrap();
        fs::remove_file(ap).unwrap();
        fs::remove_file(bp).unwrap();
        let ids = if partial {
            vec![a.display_id.clone()]
        } else {
            vec![]
        };
        let p = pack::preview(&repo, &ids).unwrap();
        let result = pack::apply(&repo, &p).unwrap();
        assert_eq!(month.exists(), partial);
        assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
        pack::restore(&repo, &result.pack_hash, None).unwrap();
        assert_eq!(fs::read_to_string(&month).unwrap(), raw);
        assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
    }
}

#[cfg(unix)]
#[test]
fn symlink_original_and_pack_directory_rejected() {
    let (_dir, repo, goal) = fixture();
    let saved = record(&repo, &goal, "a");
    let path = repo
        .evidence_path()
        .join("records")
        .join(format!("{}.json", saved.display_id));
    let external = tempfile::NamedTempFile::new().unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(external.path(), &path).unwrap();
    assert!(evidence::read_located_mirrors(&repo).is_err());
    fs::remove_file(path).unwrap();
    std::os::unix::fs::symlink(external.path(), repo.belay_dir.join("packs")).unwrap();
    assert!(pack::read_entries(&repo).is_err());
}

#[test]
fn readers_only_see_complete_records_during_publication() {
    let (_dir, repo, goal) = fixture();
    let writer_repo = repo.clone();
    let writer_goal = goal.clone();
    let handle = std::thread::spawn(move || {
        for n in 0..25 {
            record(&writer_repo, &writer_goal, &format!("concurrent {n}"));
        }
    });
    let mut observed = 0;
    while !handle.is_finished() {
        let originals = evidence::read_located_mirrors(&repo).unwrap();
        assert!(originals.len() >= observed);
        observed = originals.len();
        for r in originals {
            assert!(r.record.summary.starts_with("concurrent "));
        }
    }
    handle.join().unwrap();
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 25);
}

#[test]
fn independent_compactors_serialize_and_retry_same_snapshot() {
    let (_dir, repo, goal) = fixture();
    record(&repo, &goal, "a");
    record(&repo, &goal, "b");
    let preview = pack::preview(&repo, &[]).unwrap();
    let one_repo = repo.clone();
    let one_preview = preview.clone();
    let two_repo = repo.clone();
    let two_preview = preview.clone();
    let one = std::thread::spawn(move || pack::apply(&one_repo, &one_preview).unwrap());
    let two = std::thread::spawn(move || pack::apply(&two_repo, &two_preview).unwrap());
    let a = one.join().unwrap();
    let b = two.join().unwrap();
    assert_eq!(a.pack_hash, b.pack_hash);
    assert_eq!(a.retired + b.retired, 2);
    assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
}

#[test]
fn appended_month_after_preview_remains_intact() {
    let (_dir, repo, goal) = fixture();
    let a = record(&repo, &goal, "a");
    let path = repo
        .evidence_path()
        .join("records")
        .join(format!("{}.json", a.display_id));
    let raw = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let month = repo.evidence_path().join("2026-09.ndjson");
    fs::write(&month, format!("{raw}\n")).unwrap();
    let preview = pack::preview(&repo, &[]).unwrap();
    let next = record(&repo, &goal, "next");
    let next_raw = fs::read_to_string(
        repo.evidence_path()
            .join("records")
            .join(format!("{}.json", next.display_id)),
    )
    .unwrap();
    let appended = format!("{raw}\n{next_raw}\n");
    fs::write(&month, &appended).unwrap();
    assert!(pack::apply(&repo, &preview).is_err());
    assert_eq!(fs::read_to_string(month).unwrap(), appended);
}

#[test]
fn unknown_pack_schema_rejected_and_external_logs_not_claimed_preserved() {
    let (_dir, repo, goal) = fixture();
    let external = tempfile::NamedTempFile::new().unwrap();
    fs::write(external.path(), b"raw external fixture").unwrap();
    let saved = evidence::record(
        &repo,
        RecordInput {
            kind: "test".into(),
            verdict: "pass".into(),
            commit_sha: Some("abc".into()),
            captured_at: None,
            source: external.path().to_string_lossy().into(),
            issuer: "test".into(),
            summary: "reference only".into(),
            detail: json!({}),
            verifies: vec![goal],
        },
    )
    .unwrap();
    let packed = pack::apply(&repo, &pack::preview(&repo, &[]).unwrap()).unwrap();
    assert_eq!(fs::read(external.path()).unwrap(), b"raw external fixture");
    let original = pack::resolve_original(&repo, &saved.display_id, 1).unwrap();
    assert!(!original.raw_payload.contains("raw external fixture"));
    let path = repo
        .belay_dir
        .join("packs")
        .join(format!("{}.json", packed.pack_hash));
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    document["schema_version"] = json!(999);
    let modified = serde_json::to_vec(&document).unwrap();
    let hash = lifecycle::hash(&modified);
    let newpath = repo.belay_dir.join("packs").join(format!("{hash}.json"));
    fs::write(&newpath, &modified).unwrap();
    assert!(
        evidence::read_located_mirrors(&repo)
            .unwrap_err()
            .to_string()
            .contains("unsupported pack schema")
    );
}

#[test]
fn sync_recovers_exact_packed_revision_without_resetting_history() {
    let (_dir, repo, _) = fixture();
    let work = store::create(&repo, EntryType::Work, "stable-work".into(), "body".into()).unwrap();
    store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
    let revision = store::show(&repo, &work.display_id).unwrap().entry.revision;
    pack::apply(
        &repo,
        &pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap(),
    )
    .unwrap();
    let connection = belay_trace::database::open(&repo.database_path()).unwrap();
    connection
        .execute(
            "UPDATE entries SET revision=1 WHERE display_id=?1",
            [&work.display_id],
        )
        .unwrap();
    drop(connection);
    let report = reconcile::synchronize(&repo, None, None).unwrap();
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        store::show(&repo, &work.display_id).unwrap().entry.revision,
        revision
    );
    assert!(
        !repo
            .belay_dir
            .join("entries/work")
            .join(format!("{}.md", work.display_id))
            .exists()
    );
}

#[test]
fn restored_work_twin_preserves_history_through_index_repair_and_reopen() {
    for rebuild in [false, true] {
        for missing in [false, true] {
            let (_dir, repo, _) = fixture();
            let work = store::create(
                &repo,
                EntryType::Work,
                "restored-work".into(),
                "completed original".into(),
            )
            .unwrap();
            store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
            let shown = store::show(&repo, &work.display_id).unwrap();
            let terminal = shown.entry;
            assert!(terminal.revision > 1);
            let path = repo.belay_dir.join(shown.source_path);
            let original = fs::read(&path).unwrap();
            let packed = pack::apply(
                &repo,
                &pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap(),
            )
            .unwrap();
            pack::restore(&repo, &packed.pack_hash, None).unwrap();
            assert_eq!(fs::read(&path).unwrap(), original);

            let connection = belay_trace::database::open(&repo.database_path()).unwrap();
            connection
                .execute(
                    if missing {
                        "DELETE FROM entries WHERE display_id=?1"
                    } else {
                        "UPDATE entries SET revision=1 WHERE display_id=?1"
                    },
                    [&work.display_id],
                )
                .unwrap();
            drop(connection);
            if rebuild {
                reconcile::rebuild(&repo).unwrap();
            } else {
                let report = reconcile::synchronize(&repo, None, None).unwrap();
                assert!(report.failures.is_empty(), "{:?}", report.failures);
            }
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(
                store::show(&repo, &work.display_id).unwrap().entry,
                terminal
            );

            store::set_status(&repo, &work.display_id, EntryStatus::InProgress).unwrap();
            let live = store::show(&repo, &work.display_id).unwrap().entry;
            assert_eq!(live.revision, terminal.revision + 1);
            let live_bytes = fs::read(&path).unwrap();
            let connection = belay_trace::database::open(&repo.database_path()).unwrap();
            connection
                .execute(
                    if missing {
                        "DELETE FROM entries WHERE display_id=?1"
                    } else {
                        "UPDATE entries SET revision=1 WHERE display_id=?1"
                    },
                    [&work.display_id],
                )
                .unwrap();
            drop(connection);
            let report = reconcile::synchronize(&repo, None, None).unwrap();
            assert!(report.failures.is_empty(), "{:?}", report.failures);
            assert_eq!(store::show(&repo, &work.display_id).unwrap().entry, live);
            assert_eq!(fs::read(&path).unwrap(), live_bytes);
            reconcile::rebuild(&repo).unwrap();
            assert_eq!(store::show(&repo, &work.display_id).unwrap().entry, live);
            assert_eq!(fs::read(&path).unwrap(), live_bytes);
            assert_eq!(
                pack::resolve_original(&repo, &work.display_id, terminal.revision)
                    .unwrap()
                    .raw_payload
                    .as_bytes(),
                original
            );
        }
    }
}

#[test]
fn reopened_work_markdown_edits_advance_revision_and_repack_without_conflict() {
    for prefer_markdown in [false, true] {
        let (_dir, repo, _) = fixture();
        let work = store::create(
            &repo,
            EntryType::Work,
            "editable-work".into(),
            "original body".into(),
        )
        .unwrap();
        store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
        let terminal = store::show(&repo, &work.display_id).unwrap();
        assert_eq!(terminal.entry.revision, 2);
        let path = repo.belay_dir.join(terminal.source_path);
        let original = fs::read(&path).unwrap();
        pack::apply(
            &repo,
            &pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap(),
        )
        .unwrap();
        store::set_status(&repo, &work.display_id, EntryStatus::InProgress).unwrap();
        assert_eq!(
            store::show(&repo, &work.display_id).unwrap().entry.revision,
            3
        );

        for (body, revision) in [("first edit", 4), ("second edit", 5)] {
            let mut edited =
                belay_trace::markdown::parse(&fs::read_to_string(&path).unwrap()).unwrap();
            edited.body = body.into();
            fs::write(&path, belay_trace::markdown::render(&edited).unwrap()).unwrap();
            let report = if prefer_markdown {
                reconcile::synchronize(
                    &repo,
                    Some(&work.display_id),
                    Some(reconcile::SyncPreference::Markdown),
                )
            } else {
                reconcile::synchronize(&repo, None, None)
            }
            .unwrap();
            assert!(report.failures.is_empty(), "{:?}", report.failures);
            let indexed = store::show(&repo, &work.display_id).unwrap().entry;
            assert_eq!(indexed.revision, revision);
            assert_eq!(indexed.status, EntryStatus::InProgress);
            assert_eq!(indexed.body, body);
            assert_eq!(
                belay_trace::markdown::parse(&fs::read_to_string(&path).unwrap()).unwrap(),
                indexed
            );
        }

        let live_bytes = fs::read(&path).unwrap();
        store::set_status(&repo, &work.display_id, EntryStatus::Completed).unwrap();
        let completed = store::show(&repo, &work.display_id).unwrap().entry;
        assert_eq!(completed.revision, 6);
        let current_bytes = fs::read(&path).unwrap();
        // A stale live mirror must not replace the newer SQLite revision.
        fs::write(&path, live_bytes).unwrap();
        let report = reconcile::synchronize(&repo, None, None).unwrap();
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert_eq!(
            store::show(&repo, &work.display_id).unwrap().entry,
            completed
        );
        assert_eq!(fs::read(&path).unwrap(), current_bytes);
        pack::apply(
            &repo,
            &pack::preview(&repo, std::slice::from_ref(&work.display_id)).unwrap(),
        )
        .unwrap();
        reconcile::rebuild(&repo).unwrap();
        assert_eq!(
            store::show(&repo, &work.display_id).unwrap().entry,
            completed
        );
        for (revision, bytes) in [(2, original), (6, current_bytes)] {
            assert_eq!(
                pack::resolve_original(&repo, &work.display_id, revision)
                    .unwrap()
                    .raw_payload
                    .as_bytes(),
                bytes
            );
        }
    }
}
