use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, SecondsFormat};
use rusqlite::{Connection, OpenFlags, params};
#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};

use crate::agent;
use crate::database;
use crate::entry::{Entry, EntryType, begin_immediate, parse_display_id, parse_entry_reference_id};
use crate::error::BelayError;
use crate::markdown;
use crate::repository::Repository;
use crate::store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPreference {
    Markdown,
    Sqlite,
}

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub display_id: String,
    pub action: &'static str,
}

#[derive(Debug, Clone)]
pub struct SyncFailure {
    pub subject: String,
    pub message: String,
    pub exit_code: u8,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub outcomes: Vec<SyncOutcome>,
    pub failures: Vec<SyncFailure>,
}

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: String,
    pub status: &'static str,
    pub detail: String,
}

#[derive(Debug)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
    pub has_drift: bool,
    pub has_invalid: bool,
}

#[derive(Debug, Clone)]
struct MirrorRecord {
    path: PathBuf,
    source_path: String,
    entry: Entry,
    hash: String,
}

#[derive(Debug, Default)]
struct MirrorInventory {
    entries: BTreeMap<String, MirrorRecord>,
    temporary_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct Baseline {
    entry_id: i64,
    source_path: String,
    sqlite_hash: String,
    mirror_hash: String,
}

#[derive(Debug, Clone)]
struct DatabaseRecord {
    internal_id: i64,
    source_path: Option<String>,
    hash: String,
    baseline: Option<Baseline>,
}

pub fn synchronize(
    repository: &Repository,
    target: Option<&str>,
    preference: Option<SyncPreference>,
) -> Result<SyncReport, BelayError> {
    if let Some(target) = target {
        parse_display_id(target)?;
    }
    if preference.is_some() && target.is_none() {
        return validation("a sync preference requires one display ID");
    }

    let inventory = discover_mirrors(repository)?;
    let database_path = repository.database_path();
    let connection = database::open(&database_path)?;
    let mut records = load_database_records(&connection, &database_path)?;
    let mut stale_baselines = load_stale_baselines(&connection, &database_path)?;
    drop(connection);

    let mut ids = BTreeSet::new();
    ids.extend(records.keys().cloned());
    ids.extend(inventory.entries.keys().cloned());
    if let Some(target) = target {
        ids.retain(|id| id == target);
        if ids.is_empty() {
            return validation(format!(
                "entry {target} was not found in SQLite or Markdown"
            ));
        }
    }

    let mut report = SyncReport::default();
    let restored_database_ids = missing_database_dependency_closure(&ids, &records, &inventory)?;
    if !restored_database_ids.is_empty() {
        if let Err(error) = restore_missing_database_entries(
            repository,
            &inventory,
            &restored_database_ids,
            &stale_baselines,
        ) {
            report.failures.push(SyncFailure {
                subject: restored_database_ids
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                message: error.to_string(),
                exit_code: error.exit_code(),
            });
            return Ok(report);
        }
        report
            .outcomes
            .extend(
                restored_database_ids
                    .iter()
                    .cloned()
                    .map(|display_id| SyncOutcome {
                        display_id,
                        action: "imported Markdown and restored SQLite",
                    }),
            );
    }

    if !restored_database_ids.is_empty() {
        let connection = match database::open(&database_path) {
            Ok(connection) => connection,
            Err(error) => {
                report.failures.push(SyncFailure {
                    subject: "reload restored SQLite entries".to_owned(),
                    message: error.to_string(),
                    exit_code: error.exit_code(),
                });
                return Ok(report);
            }
        };
        records = match load_database_records(&connection, &database_path) {
            Ok(records) => records,
            Err(error) => {
                report.failures.push(SyncFailure {
                    subject: "reload restored SQLite entries".to_owned(),
                    message: error.to_string(),
                    exit_code: error.exit_code(),
                });
                return Ok(report);
            }
        };
        stale_baselines = match load_stale_baselines(&connection, &database_path) {
            Ok(stale_baselines) => stale_baselines,
            Err(error) => {
                report.failures.push(SyncFailure {
                    subject: "reload sync baselines".to_owned(),
                    message: error.to_string(),
                    exit_code: error.exit_code(),
                });
                return Ok(report);
            }
        };
        drop(connection);
        if let Err(error) =
            restore_links_to_targets(repository, &inventory, &records, &restored_database_ids)
        {
            report.failures.push(SyncFailure {
                subject: "restored relationships".to_owned(),
                message: error.to_string(),
                exit_code: error.exit_code(),
            });
            return Ok(report);
        }
        let connection = match database::open(&database_path) {
            Ok(connection) => connection,
            Err(error) => {
                report.failures.push(SyncFailure {
                    subject: "reload repaired SQLite entries".to_owned(),
                    message: error.to_string(),
                    exit_code: error.exit_code(),
                });
                return Ok(report);
            }
        };
        records = match load_database_records(&connection, &database_path) {
            Ok(records) => records,
            Err(error) => {
                report.failures.push(SyncFailure {
                    subject: "reload repaired SQLite entries".to_owned(),
                    message: error.to_string(),
                    exit_code: error.exit_code(),
                });
                return Ok(report);
            }
        };
        drop(connection);
    }

    for display_id in ids {
        if restored_database_ids.contains(&display_id) {
            continue;
        }
        match sync_one(
            repository,
            &display_id,
            records.get(&display_id),
            inventory.entries.get(&display_id),
            stale_baselines.iter().find(|baseline| {
                inventory
                    .entries
                    .get(&display_id)
                    .is_some_and(|mirror| mirror.source_path == baseline.source_path)
            }),
            preference,
        ) {
            Ok(action) => report.outcomes.push(SyncOutcome { display_id, action }),
            Err(error) => report.failures.push(SyncFailure {
                subject: display_id,
                message: error.to_string(),
                exit_code: error.exit_code(),
            }),
        }
    }

    if target.is_none() {
        for baseline in stale_baselines {
            let restored = inventory
                .entries
                .values()
                .any(|mirror| mirror.source_path == baseline.source_path);
            if !restored {
                report.failures.push(SyncFailure {
                    subject: baseline.source_path.clone(),
                    message: format!(
                        "stale sync baseline {} has no SQLite row or Markdown counterpart",
                        baseline.source_path
                    ),
                    exit_code: 5,
                });
            }
        }
    }

    if let Err(error) = crate::evidence::reindex(repository) {
        report.failures.push(SyncFailure {
            subject: "evidence".to_owned(),
            message: error.to_string(),
            exit_code: error.exit_code(),
        });
    }
    Ok(report)
}

fn missing_database_dependency_closure(
    requested_ids: &BTreeSet<String>,
    records: &BTreeMap<String, DatabaseRecord>,
    inventory: &MirrorInventory,
) -> Result<BTreeSet<String>, BelayError> {
    let mut pending = requested_ids.iter().cloned().collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut missing = BTreeSet::new();
    while let Some(display_id) = pending.pop() {
        if !visited.insert(display_id.clone()) {
            continue;
        }
        let Some(mirror) = inventory.entries.get(&display_id) else {
            continue;
        };
        if !records.contains_key(&display_id) {
            missing.insert(display_id);
        }
        pending.extend(
            mirror
                .entry
                .links
                .iter()
                .filter_map(|link| {
                    let target = link_target_entry(&link.id);
                    if records.contains_key(target) {
                        None
                    } else if inventory.entries.contains_key(target) {
                        Some(Ok(target.to_owned()))
                    } else {
                        Some(validation(format!(
                            "entry {} links to missing entry {}",
                            mirror.entry.display_id, link.id
                        )))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    Ok(missing)
}

fn restore_missing_database_entries(
    repository: &Repository,
    inventory: &MirrorInventory,
    display_ids: &BTreeSet<String>,
    stale_baselines: &[Baseline],
) -> Result<(), BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    let mut imported = BTreeMap::new();

    for display_id in display_ids {
        let mirror = inventory
            .entries
            .get(display_id)
            .ok_or_else(|| BelayError::Validation {
                message: format!(
                    "entry {display_id} is linked from managed Markdown but has no mirror"
                ),
            })?;
        if let Some(stale_baseline) = stale_baselines
            .iter()
            .find(|baseline| mirror.source_path == baseline.source_path)
        {
            cleanup_orphaned_entry_rows(&transaction, &database_path, stale_baseline.entry_id)?;
            transaction
                .execute(
                    "DELETE FROM sync_state WHERE source_path = ?1",
                    [&stale_baseline.source_path],
                )
                .map_err(|source| BelayError::sqlite(&database_path, source))?;
        }
        let mut entry = mirror.entry.clone();
        entry.revision = 1;
        entry.updated_at = import_timestamp(&entry.created_at)?;
        let entry = entry.normalized()?;
        let internal_id =
            store::insert_entry(&transaction, &database_path, &entry, &mirror.source_path)?;
        imported.insert(display_id.clone(), (internal_id, entry));
    }

    for (display_id, (internal_id, entry)) in &imported {
        let mirror = &inventory.entries[display_id];
        store::replace_links(&transaction, &database_path, *internal_id, &entry.links)?;
        let hash = markdown::content_hash(entry)?;
        store::upsert_sync_state(
            &transaction,
            &database_path,
            *internal_id,
            &mirror.source_path,
            &hash,
            &entry.updated_at,
        )?;
        let rendered = markdown::render(entry)?;
        store::replace_file(repository, &mirror.path, rendered.as_bytes(), &mirror.hash)?;
    }
    transaction.commit()
}

fn restore_links_to_targets(
    repository: &Repository,
    inventory: &MirrorInventory,
    records: &BTreeMap<String, DatabaseRecord>,
    restored_targets: &BTreeSet<String>,
) -> Result<(), BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    for mirror in inventory.entries.values() {
        let Some(baseline) = records
            .get(&mirror.entry.display_id)
            .and_then(|record| record.baseline.as_ref())
        else {
            continue;
        };
        if mirror.hash != baseline.mirror_hash {
            continue;
        }
        let source_id = match store::resolve_internal_id(
            &transaction,
            &database_path,
            &mirror.entry.display_id,
        ) {
            Ok(source_id) => source_id,
            Err(BelayError::Validation { .. }) => continue,
            Err(error) => return Err(error),
        };
        for link in &mirror.entry.links {
            let target = parse_entry_reference_id(&link.id)?;
            if !restored_targets.contains(&target.display_id) {
                continue;
            }
            let target_id =
                store::resolve_internal_id(&transaction, &database_path, &target.display_id)?;
            store::validate_reference_fragment(&transaction, &database_path, &target)?;
            let metadata =
                serde_json::to_string(&link.metadata).map_err(|source| BelayError::Validation {
                    message: format!("could not serialize link metadata: {source}"),
                })?;
            transaction
                .execute(
                    "
                    INSERT INTO entry_links(
                        from_entry_id, to_entry_id, to_fragment, relation, metadata_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(from_entry_id, to_entry_id, to_fragment, relation)
                    DO UPDATE SET metadata_json = excluded.metadata_json
                    ",
                    params![
                        source_id,
                        target_id,
                        target.fragment.as_deref().unwrap_or(""),
                        link.relation.to_string(),
                        metadata
                    ],
                )
                .map_err(|source| BelayError::sqlite(&database_path, source))?;
        }
    }
    transaction.commit()
}

fn sync_one(
    repository: &Repository,
    display_id: &str,
    database_record: Option<&DatabaseRecord>,
    mirror_record: Option<&MirrorRecord>,
    stale_baseline: Option<&Baseline>,
    preference: Option<SyncPreference>,
) -> Result<&'static str, BelayError> {
    match (database_record, mirror_record) {
        (None, None) => Err(BelayError::Conflict {
            message: format!("entry {display_id} is missing from both SQLite and Markdown"),
        }),
        (None, Some(mirror)) => {
            import_markdown(repository, None, mirror, stale_baseline)?;
            Ok("imported Markdown and restored SQLite")
        }
        (Some(database), None) => {
            write_sqlite(repository, database, None)?;
            Ok("rendered SQLite and restored Markdown")
        }
        (Some(database), Some(mirror)) => {
            if let Some(preference) = preference {
                return match preference {
                    SyncPreference::Markdown => {
                        import_markdown(repository, Some(database), mirror, None)?;
                        Ok("kept Markdown")
                    }
                    SyncPreference::Sqlite => {
                        write_sqlite(repository, database, Some(mirror))?;
                        Ok("kept SQLite")
                    }
                };
            }

            match &database.baseline {
                None if database.hash == mirror.hash => {
                    establish_baseline(repository, database, mirror)?;
                    Ok("established matching baseline")
                }
                None => Err(BelayError::Conflict {
                    message: format!(
                        "entry {display_id} exists independently in SQLite and Markdown with different content; use an explicit --prefer"
                    ),
                }),
                Some(baseline) => {
                    let sqlite_changed = database.hash != baseline.sqlite_hash;
                    let markdown_changed = mirror.hash != baseline.mirror_hash;
                    match (sqlite_changed, markdown_changed) {
                        (false, false) => {
                            if database.source_path.as_deref() != Some(&mirror.source_path) {
                                establish_baseline(repository, database, mirror)?;
                                Ok("accepted path-only rename")
                            } else {
                                Ok("unchanged")
                            }
                        }
                        (false, true) => {
                            import_markdown(repository, Some(database), mirror, None)?;
                            Ok("imported Markdown")
                        }
                        (true, false) => {
                            write_sqlite(repository, database, Some(mirror))?;
                            Ok("rendered SQLite")
                        }
                        (true, true) if database.hash == mirror.hash => {
                            establish_baseline(repository, database, mirror)?;
                            Ok("established converged baseline")
                        }
                        (true, true) => Err(BelayError::Conflict {
                            message: format!(
                                "entry {display_id} changed in both SQLite and Markdown; use `belay sync --prefer markdown {display_id}` or `belay sync --prefer sqlite {display_id}`"
                            ),
                        }),
                    }
                }
            }
        }
    }
}

fn import_markdown(
    repository: &Repository,
    database_record: Option<&DatabaseRecord>,
    mirror: &MirrorRecord,
    stale_baseline: Option<&Baseline>,
) -> Result<(), BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    let sync_time = now();

    let (internal_id, entry) = if let Some(database_record) = database_record {
        let current = store::load_entry(&transaction, &database_path, database_record.internal_id)?;
        ensure_database_snapshot(&transaction, &database_path, database_record, &current)?;
        let mut imported = mirror.entry.clone();
        imported.created_at = current.created_at;
        imported.revision =
            current
                .revision
                .checked_add(1)
                .ok_or_else(|| BelayError::Validation {
                    message: format!("entry {} revision overflowed", current.display_id),
                })?;
        imported.updated_at = sync_time;
        let imported = imported.normalized()?;
        store::replace_entry(
            &transaction,
            &database_path,
            database_record.internal_id,
            &imported,
            &mirror.source_path,
        )?;
        store::replace_links(
            &transaction,
            &database_path,
            database_record.internal_id,
            &imported.links,
        )?;
        (database_record.internal_id, imported)
    } else {
        if let Some(stale_baseline) = stale_baseline {
            cleanup_orphaned_entry_rows(&transaction, &database_path, stale_baseline.entry_id)?;
            transaction
                .execute(
                    "DELETE FROM sync_state WHERE source_path = ?1",
                    [&stale_baseline.source_path],
                )
                .map_err(|source| BelayError::sqlite(&database_path, source))?;
        }
        let mut imported = mirror.entry.clone();
        imported.revision = 1;
        imported.updated_at = import_timestamp(&imported.created_at)?;
        let imported = imported.normalized()?;
        let internal_id =
            store::insert_entry(&transaction, &database_path, &imported, &mirror.source_path)?;
        store::replace_links(&transaction, &database_path, internal_id, &imported.links)?;
        (internal_id, imported)
    };

    let hash = markdown::content_hash(&entry)?;
    let rendered = markdown::render(&entry)?;
    store::upsert_sync_state(
        &transaction,
        &database_path,
        internal_id,
        &mirror.source_path,
        &hash,
        &entry.updated_at,
    )?;
    store::replace_file(repository, &mirror.path, rendered.as_bytes(), &mirror.hash)?;
    transaction.commit()
}

fn write_sqlite(
    repository: &Repository,
    database_record: &DatabaseRecord,
    mirror: Option<&MirrorRecord>,
) -> Result<(), BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    let entry = store::load_entry(&transaction, &database_path, database_record.internal_id)?;
    let current_source_path = transaction
        .query_row(
            "SELECT source_path FROM entries WHERE id = ?1",
            [database_record.internal_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let hash = markdown::content_hash(&entry)?;
    let rendered = markdown::render(&entry)?;
    let source_path = mirror
        .map(|mirror| mirror.source_path.clone())
        .or(current_source_path)
        .unwrap_or_else(|| {
            store::path_to_storage_string(&store::mirror_relative_path(repository, &entry))
                .expect("default mirror path is UTF-8")
        });
    let destination = mirror
        .map(|mirror| mirror.path.clone())
        .unwrap_or(store::managed_source_path(repository, &source_path)?);

    store::replace_entry(
        &transaction,
        &database_path,
        database_record.internal_id,
        &entry,
        &source_path,
    )?;
    store::replace_links(
        &transaction,
        &database_path,
        database_record.internal_id,
        &entry.links,
    )?;
    store::upsert_sync_state(
        &transaction,
        &database_path,
        database_record.internal_id,
        &source_path,
        &hash,
        &now(),
    )?;
    if let Some(mirror) = mirror {
        store::replace_file(repository, &destination, rendered.as_bytes(), &mirror.hash)?;
    } else {
        store::write_new_file(repository, &destination, rendered.as_bytes())?;
    }
    transaction.commit()
}

fn establish_baseline(
    repository: &Repository,
    database_record: &DatabaseRecord,
    mirror: &MirrorRecord,
) -> Result<(), BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    let current = store::load_entry(&transaction, &database_path, database_record.internal_id)?;
    ensure_database_snapshot(&transaction, &database_path, database_record, &current)?;
    let current_mirror = markdown::parse(&store::read_managed_file(repository, &mirror.path)?)?;
    if markdown::content_hash(&current_mirror)? != mirror.hash {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {} Markdown changed during sync; retry after reviewing the latest file",
                mirror.entry.display_id
            ),
        });
    }
    transaction
        .execute(
            "UPDATE entries SET source_path = ?1, content_hash = ?2 WHERE id = ?3",
            params![
                mirror.source_path,
                database_record.hash,
                database_record.internal_id
            ],
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    store::upsert_sync_state(
        &transaction,
        &database_path,
        database_record.internal_id,
        &mirror.source_path,
        &database_record.hash,
        &now(),
    )?;
    transaction.commit()
}

fn ensure_database_snapshot(
    connection: &Connection,
    database_path: &Path,
    expected: &DatabaseRecord,
    current: &Entry,
) -> Result<(), BelayError> {
    let current_hash = markdown::content_hash(current)?;
    let current_source_path = connection
        .query_row(
            "SELECT source_path FROM entries WHERE id = ?1",
            [expected.internal_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    if current_hash != expected.hash || current_source_path != expected.source_path {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {} SQLite state changed during sync; retry after reviewing the latest state",
                current.display_id
            ),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildOutcome {
    pub entries: usize,
    pub evidence: usize,
}

pub fn rebuild(repository: &Repository) -> Result<RebuildOutcome, BelayError> {
    let inventory = discover_mirrors(repository)?;
    validate_link_targets(&inventory)?;
    let database_path = repository.database_path();
    database_path
        .parent()
        .ok_or_else(|| BelayError::Validation {
            message: format!("database path {} has no parent", database_path.display()),
        })?;
    let temporary = unique_sibling(&database_path, "rebuild");
    let backup = unique_sibling(&database_path, "backup");
    let route_receipts = load_route_receipts_for_rebuild(&database_path)?;

    let build_result = (|| {
        database::initialize(&temporary)?;
        let mut connection = database::open(&temporary)?;
        let transaction = begin_immediate(&mut connection, &temporary)?;
        let mut internal_ids = BTreeMap::new();
        for (display_id, mirror) in &inventory.entries {
            let internal_id =
                store::insert_entry(&transaction, &temporary, &mirror.entry, &mirror.source_path)?;
            internal_ids.insert(display_id.clone(), internal_id);
        }
        for (display_id, mirror) in &inventory.entries {
            let internal_id = internal_ids[display_id];
            store::replace_links(&transaction, &temporary, internal_id, &mirror.entry.links)?;
            store::upsert_sync_state(
                &transaction,
                &temporary,
                internal_id,
                &mirror.source_path,
                &mirror.hash,
                &now(),
            )?;
        }
        let evidence = crate::evidence::rebuild_into(repository, &transaction, &temporary)?;
        for receipt in &route_receipts {
            transaction
                .execute(
                    "
                    INSERT INTO route_operation_receipts(
                        run_id, operation_id, preview_hash, outcome, target,
                        post_revision, recorded_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ",
                    params![
                        receipt.0, receipt.1, receipt.2, receipt.3, receipt.4, receipt.5, receipt.6
                    ],
                )
                .map_err(|source| BelayError::sqlite(&temporary, source))?;
        }
        transaction.commit()?;
        drop(connection);
        Ok(evidence)
    })();
    let evidence = match build_result {
        Ok(evidence) => evidence,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = replace_database(&database_path, &temporary, &backup) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(RebuildOutcome {
        entries: inventory.entries.len(),
        evidence,
    })
}

type RouteReceiptRecord = (String, String, String, String, String, u32, String);

fn load_route_receipts_for_rebuild(
    database_path: &Path,
) -> Result<Vec<RouteReceiptRecord>, BelayError> {
    if !database_path.exists() {
        return Ok(Vec::new());
    }
    // Rebuild must accept an older on-disk schema so it can replace it with the
    // latest schema. `database::open_read_only` deliberately rejects those
    // databases, so inspect the optional receipt table with a raw read-only
    // connection instead, while retaining the standard SQLite configuration.
    let connection = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    database::configure(&connection, database_path)?;
    let has_receipts: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'route_operation_receipts')",
            [],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    if !has_receipts {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "
            SELECT run_id, operation_id, preview_hash, outcome, target,
                   post_revision, recorded_at
            FROM route_operation_receipts
            ORDER BY run_id, operation_id
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn replace_database(active: &Path, temporary: &Path, backup: &Path) -> Result<(), BelayError> {
    #[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
    {
        let _ = backup;
        replace_database_exchange(active, temporary)
    }

    #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
    {
        replace_database_portable(active, temporary, backup)
    }
}

#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
fn replace_database_exchange(active: &Path, temporary: &Path) -> Result<(), BelayError> {
    let parent = active.parent().ok_or_else(|| BelayError::Validation {
        message: format!("database path {} has no parent", active.display()),
    })?;
    let active_name = active.file_name().ok_or_else(|| BelayError::Validation {
        message: format!("database path {} has no filename", active.display()),
    })?;
    let temporary_name = temporary
        .file_name()
        .ok_or_else(|| BelayError::Validation {
            message: format!("temporary database {} has no filename", temporary.display()),
        })?;
    let directory = rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| unix_io("open database directory", parent, source))?;
    let active_metadata = match fs::symlink_metadata(active) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return validation(format!(
                "database path {} must be a regular file",
                active.display()
            ));
        }
        Ok(metadata) => Some(metadata),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => return Err(BelayError::io("inspect database", active, source)),
    };
    if let Some(metadata) = active_metadata {
        fs::set_permissions(temporary, metadata.permissions())
            .map_err(|source| BelayError::io("preserve database permissions", temporary, source))?;
        rustix::fs::renameat_with(
            &directory,
            temporary_name,
            &directory,
            active_name,
            RenameFlags::EXCHANGE,
        )
        .map_err(|source| unix_io("exchange rebuilt database", active, source))?;
        if let Err(source) = rustix::fs::fsync(&directory) {
            if rustix::fs::renameat_with(
                &directory,
                temporary_name,
                &directory,
                active_name,
                RenameFlags::EXCHANGE,
            )
            .is_ok()
            {
                let _ = rustix::fs::fsync(&directory);
                return Err(unix_io("flush database directory", parent, source));
            }
            return Ok(());
        }
        let _ = rustix::fs::unlinkat(&directory, temporary_name, AtFlags::empty());
    } else {
        rustix::fs::renameat(&directory, temporary_name, &directory, active_name)
            .map_err(|source| unix_io("activate rebuilt database", active, source))?;
        if let Err(source) = rustix::fs::fsync(&directory) {
            if rustix::fs::renameat(&directory, active_name, &directory, temporary_name).is_ok() {
                let _ = rustix::fs::fsync(&directory);
                return Err(unix_io("flush database directory", parent, source));
            }
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
fn replace_database_portable(
    active: &Path,
    temporary: &Path,
    backup: &Path,
) -> Result<(), BelayError> {
    let active_exists = match fs::symlink_metadata(active) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return validation(format!(
                "database path {} must be a regular file",
                active.display()
            ));
        }
        Ok(_) => true,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => return Err(BelayError::io("inspect database", active, source)),
    };
    if active_exists {
        fs::rename(active, backup)
            .map_err(|source| BelayError::io("stage existing database", active, source))?;
    }
    if let Err(source) = fs::rename(temporary, active) {
        if active_exists {
            let _ = fs::rename(backup, active);
        }
        return Err(BelayError::io("activate rebuilt database", active, source));
    }
    if active_exists {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
fn unix_io(action: &'static str, path: &Path, source: rustix::io::Errno) -> BelayError {
    BelayError::io(
        action,
        path,
        std::io::Error::from_raw_os_error(source.raw_os_error()),
    )
}

pub fn doctor(repository: &Repository) -> DoctorReport {
    let mut checks = Vec::new();
    let mut has_drift = false;
    let mut has_invalid = false;

    checks.push(DoctorCheck {
        name: "configuration".to_owned(),
        status: "ok",
        detail: repository
            .belay_dir
            .join("config.toml")
            .display()
            .to_string(),
    });

    let inventory = match discover_mirrors(repository) {
        Ok(inventory) => {
            checks.push(DoctorCheck {
                name: "managed Markdown".to_owned(),
                status: "ok",
                detail: format!("{} valid entries", inventory.entries.len()),
            });
            if inventory.temporary_files.is_empty() {
                checks.push(DoctorCheck {
                    name: "orphaned temporary files".to_owned(),
                    status: "ok",
                    detail: "none".to_owned(),
                });
            } else {
                has_drift = true;
                checks.push(DoctorCheck {
                    name: "orphaned temporary files".to_owned(),
                    status: "drift",
                    detail: inventory
                        .temporary_files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            Some(inventory)
        }
        Err(error) => {
            has_invalid = true;
            checks.push(DoctorCheck {
                name: "managed Markdown".to_owned(),
                status: "invalid",
                detail: error.to_string(),
            });
            None
        }
    };

    let database_path = repository.database_path();
    match database::open_read_only(&database_path) {
        Ok(connection) => {
            match database::verify_schema_health(&connection, &database_path) {
                Ok(()) => checks.push(DoctorCheck {
                    name: "SQLite schema".to_owned(),
                    status: "ok",
                    detail: format!(
                        "version {}; required tables, indexes, FTS5, and foreign keys are valid",
                        database::LATEST_SCHEMA_VERSION
                    ),
                }),
                Err(error) => {
                    has_invalid = true;
                    checks.push(DoctorCheck {
                        name: "SQLite schema".to_owned(),
                        status: "invalid",
                        detail: error.to_string(),
                    });
                }
            }
            match database::verify_fts5(&connection) {
                Ok(()) => checks.push(DoctorCheck {
                    name: "FTS5 and bm25 runtime".to_owned(),
                    status: "ok",
                    detail: "available".to_owned(),
                }),
                Err(error) => {
                    has_invalid = true;
                    checks.push(DoctorCheck {
                        name: "FTS5 and bm25 runtime".to_owned(),
                        status: "invalid",
                        detail: error.to_string(),
                    });
                }
            }
            match invalid_fragment_details(&connection, &database_path) {
                Ok(details) if details.is_empty() => checks.push(DoctorCheck {
                    name: "SC/Task references".to_owned(),
                    status: "ok",
                    detail: "all structured fragments resolve uniquely".to_owned(),
                }),
                Ok(details) => {
                    has_drift = true;
                    checks.push(DoctorCheck {
                        name: "SC/Task references".to_owned(),
                        status: "drift",
                        detail: details.join("; "),
                    });
                }
                Err(error) => {
                    has_invalid = true;
                    checks.push(DoctorCheck {
                        name: "SC/Task references".to_owned(),
                        status: "invalid",
                        detail: error.to_string(),
                    });
                }
            }
            match crate::evidence::stale_doctor_details(repository, &connection, &database_path) {
                Ok(details) if details.is_empty() => checks.push(DoctorCheck {
                    name: "Evidence freshness".to_owned(),
                    status: "ok",
                    detail: "fresh or not required".to_owned(),
                }),
                Ok(details) => {
                    checks.push(DoctorCheck {
                        name: "Evidence freshness".to_owned(),
                        status: "drift",
                        detail: details.join("; "),
                    });
                }
                Err(error) => {
                    has_invalid = true;
                    checks.push(DoctorCheck {
                        name: "Evidence freshness".to_owned(),
                        status: "invalid",
                        detail: error.to_string(),
                    });
                }
            }
            if let Some(inventory) = &inventory {
                let missing_sections = inventory
                    .entries
                    .values()
                    .flat_map(|mirror| {
                        crate::goal::missing_required_sections(&mirror.entry)
                            .into_iter()
                            .map(|section| format!("{} missing {section}", mirror.entry.display_id))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                if missing_sections.is_empty() {
                    checks.push(DoctorCheck {
                        name: "Goal sections".to_owned(),
                        status: "ok",
                        detail: "all goals have required sections".to_owned(),
                    });
                } else {
                    checks.push(DoctorCheck {
                        name: "Goal sections".to_owned(),
                        status: "drift",
                        detail: missing_sections.join("; "),
                    });
                }
                let local_id_issues = inventory
                    .entries
                    .values()
                    .flat_map(|mirror| {
                        let findings = match mirror.entry.entry_type {
                            crate::entry::EntryType::Goal => {
                                crate::trace_ids::goal_id_findings(&mirror.entry.body)
                            }
                            crate::entry::EntryType::Plan => {
                                crate::trace_ids::plan_id_findings(&mirror.entry.body)
                            }
                            _ => Vec::new(),
                        };
                        findings
                            .into_iter()
                            .map(|finding| {
                                format!(
                                    "{} line {}: {}",
                                    mirror.entry.display_id, finding.line, finding.message
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                if local_id_issues.is_empty() {
                    checks.push(DoctorCheck {
                        name: "SC/Task ID format".to_owned(),
                        status: "ok",
                        detail: "all defined IDs are canonical and unique".to_owned(),
                    });
                } else {
                    has_drift = true;
                    checks.push(DoctorCheck {
                        name: "SC/Task ID format".to_owned(),
                        status: "drift",
                        detail: local_id_issues.join("; "),
                    });
                }
                match inspect_drift(&connection, &database_path, inventory) {
                    Ok(details) if details.is_empty() => checks.push(DoctorCheck {
                        name: "SQLite/Markdown drift".to_owned(),
                        status: "ok",
                        detail: "none".to_owned(),
                    }),
                    Ok(details) => {
                        has_drift = true;
                        checks.push(DoctorCheck {
                            name: "SQLite/Markdown drift".to_owned(),
                            status: "drift",
                            detail: details.join("; "),
                        });
                    }
                    Err(error) => {
                        has_invalid = true;
                        checks.push(DoctorCheck {
                            name: "SQLite/Markdown drift".to_owned(),
                            status: "invalid",
                            detail: error.to_string(),
                        });
                    }
                }
            }
        }
        Err(error) => {
            let missing = !database_path.exists();
            if missing {
                has_drift = true;
            } else {
                has_invalid = true;
            }
            checks.push(DoctorCheck {
                name: "SQLite schema".to_owned(),
                status: if missing { "missing" } else { "invalid" },
                detail: if missing {
                    "run `belay rebuild` to restore SQLite from managed Markdown".to_owned()
                } else {
                    error.to_string()
                },
            });
        }
    }

    match agent::doctor(repository) {
        Ok(report) => {
            has_drift |= report.has_drift;
            has_invalid |= report.has_malformed;
            for check in report.checks {
                checks.push(DoctorCheck {
                    name: check.name.to_owned(),
                    status: check.status.as_str(),
                    detail: check.detail(),
                });
            }
        }
        Err(error) => {
            has_invalid = true;
            checks.push(DoctorCheck {
                name: "agent integration".to_owned(),
                status: "invalid",
                detail: error.to_string(),
            });
        }
    }

    DoctorReport {
        checks,
        has_drift,
        has_invalid,
    }
}

fn invalid_fragment_details(
    connection: &Connection,
    database_path: &Path,
) -> Result<Vec<String>, BelayError> {
    let mut references = Vec::new();
    let mut statement = connection
        .prepare(
            "
            SELECT target.display_id, links.to_fragment
            FROM entry_links links
            JOIN entries target ON target.id = links.to_entry_id
            WHERE links.to_fragment != ''
            UNION
            SELECT substr(target, 1, instr(target, '#') - 1),
                   substr(target, instr(target, '#') + 1)
            FROM evidence_links
            WHERE instr(target, '#') > 0
            ORDER BY 1, 2
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    for (display_id, fragment) in rows {
        let value = format!("{display_id}#{fragment}");
        let reference = parse_entry_reference_id(&value)?;
        if let Err(error) =
            store::validate_reference_fragment(connection, database_path, &reference)
        {
            references.push(format!("{value}: {error}"));
        }
    }
    Ok(references)
}

fn inspect_drift(
    connection: &Connection,
    database_path: &Path,
    inventory: &MirrorInventory,
) -> Result<Vec<String>, BelayError> {
    let records = load_database_records(connection, database_path)?;
    let stale = load_stale_baselines(connection, database_path)?;
    let mut details = Vec::new();
    let mut ids = BTreeSet::new();
    ids.extend(records.keys().cloned());
    ids.extend(inventory.entries.keys().cloned());
    for id in ids {
        match (records.get(&id), inventory.entries.get(&id)) {
            (Some(database), Some(mirror)) => match &database.baseline {
                Some(baseline)
                    if database.hash == baseline.sqlite_hash
                        && mirror.hash == baseline.mirror_hash
                        && database.source_path.as_deref() == Some(&mirror.source_path) => {}
                Some(_) => details.push(format!("{id} has unsynchronized changes")),
                None if database.hash == mirror.hash => {
                    details.push(format!("{id} has no sync baseline"))
                }
                None => details.push(format!("{id} has ambiguous unbaselined content")),
            },
            (Some(_), None) => details.push(format!("{id} is missing Markdown")),
            (None, Some(_)) => details.push(format!("{id} is missing SQLite")),
            (None, None) => {}
        }
    }
    for baseline in stale {
        details.push(format!(
            "stale sync baseline {} has no SQLite row",
            baseline.source_path
        ));
    }
    Ok(details)
}

fn discover_mirrors(repository: &Repository) -> Result<MirrorInventory, BelayError> {
    let mut inventory = MirrorInventory::default();
    for entry_type in EntryType::ALL {
        let directory = repository.entries_path().join(entry_type.directory());
        discover_directory(repository, entry_type, &directory, &mut inventory)?;
    }
    Ok(inventory)
}

fn discover_directory(
    repository: &Repository,
    expected_type: EntryType,
    directory: &Path,
    inventory: &mut MirrorInventory,
) -> Result<(), BelayError> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|source| BelayError::io("inspect managed entry directory", directory, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return validation(format!(
            "managed entry directory {} must be a real directory",
            directory.display()
        ));
    }
    for item in fs::read_dir(directory)
        .map_err(|source| BelayError::io("read managed entry directory", directory, source))?
    {
        let item = item
            .map_err(|source| BelayError::io("read managed entry directory", directory, source))?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| BelayError::io("inspect managed entry path", &path, source))?;
        if metadata.file_type().is_symlink() {
            return validation(format!(
                "managed entry path {} must not be a symbolic link",
                path.display()
            ));
        }
        if metadata.is_dir() {
            discover_directory(repository, expected_type, &path, inventory)?;
            continue;
        }
        if !metadata.is_file() {
            return validation(format!(
                "managed entry path {} must be a regular file",
                path.display()
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| BelayError::Validation {
                message: format!("managed entry path {} is not UTF-8", path.display()),
            })?;
        if file_name.starts_with('.') && file_name.contains("tmp-") {
            inventory.temporary_files.push(path);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let contents = store::read_managed_file(repository, &path)?;
        let entry = markdown::parse(&contents)?;
        if entry.entry_type != expected_type {
            return validation(format!(
                "entry {} is stored under {}, but its type is {}",
                entry.display_id,
                expected_type.directory(),
                entry.entry_type
            ));
        }
        let expected_name = format!("{}.md", entry.display_id);
        if file_name != expected_name {
            return validation(format!(
                "managed Markdown filename {file_name:?} does not match frontmatter display ID {}; expected {expected_name:?}",
                entry.display_id
            ));
        }
        let relative =
            path.strip_prefix(&repository.belay_dir)
                .map_err(|_| BelayError::Validation {
                    message: format!(
                        "managed Markdown {} is outside {}",
                        path.display(),
                        repository.belay_dir.display()
                    ),
                })?;
        let source_path = store::path_to_storage_string(relative)?;
        let hash = markdown::content_hash(&entry)?;
        let display_id = entry.display_id.clone();
        let record = MirrorRecord {
            path,
            source_path,
            entry,
            hash,
        };
        if let Some(previous) = inventory.entries.insert(display_id.clone(), record) {
            return validation(format!(
                "duplicate display ID {display_id} in {} and {}",
                previous.path.display(),
                inventory.entries[&display_id].path.display()
            ));
        }
    }
    Ok(())
}

fn load_database_records(
    connection: &Connection,
    database_path: &Path,
) -> Result<BTreeMap<String, DatabaseRecord>, BelayError> {
    let mut statement = connection
        .prepare(
            "
            SELECT entry.id, entry.display_id, entry.source_path,
                   state.source_path,
                   state.sqlite_content_hash_at_last_sync,
                   state.mirror_content_hash_at_last_sync
            FROM entries entry
            LEFT JOIN sync_state state ON state.entry_id = entry.id
            ORDER BY entry.display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let mut records = BTreeMap::new();
    for (internal_id, display_id, source_path, baseline_path, sqlite_hash, mirror_hash) in rows {
        let entry = store::load_entry(connection, database_path, internal_id)?;
        let hash = markdown::content_hash(&entry)?;
        let baseline = match (baseline_path, sqlite_hash, mirror_hash) {
            (Some(source_path), Some(sqlite_hash), Some(mirror_hash)) => Some(Baseline {
                entry_id: internal_id,
                source_path,
                sqlite_hash,
                mirror_hash,
            }),
            (None, None, None) => None,
            _ => {
                return validation(format!(
                    "entry {display_id} has incomplete sync baseline metadata"
                ));
            }
        };
        records.insert(
            display_id,
            DatabaseRecord {
                internal_id,
                source_path,
                hash,
                baseline,
            },
        );
    }
    Ok(records)
}

fn load_stale_baselines(
    connection: &Connection,
    database_path: &Path,
) -> Result<Vec<Baseline>, BelayError> {
    let mut statement = connection
        .prepare(
            "
            SELECT state.entry_id, state.source_path, state.sqlite_content_hash_at_last_sync,
                   state.mirror_content_hash_at_last_sync
            FROM sync_state state
            LEFT JOIN entries entry ON entry.id = state.entry_id
            WHERE entry.id IS NULL
            ORDER BY state.source_path
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map([], |row| {
            Ok(Baseline {
                entry_id: row.get(0)?,
                source_path: row.get(1)?,
                sqlite_hash: row.get(2)?,
                mirror_hash: row.get(3)?,
            })
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn cleanup_orphaned_entry_rows(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
) -> Result<(), BelayError> {
    for statement in [
        "DELETE FROM entry_links WHERE from_entry_id = ?1 OR to_entry_id = ?1",
        "DELETE FROM entry_tags WHERE entry_id = ?1",
        "DELETE FROM entry_chunks WHERE entry_id = ?1",
        "DELETE FROM entry_fts WHERE entry_id = ?1",
        "DELETE FROM sync_state WHERE entry_id = ?1",
    ] {
        connection
            .execute(statement, [internal_id])
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    Ok(())
}

fn validate_link_targets(inventory: &MirrorInventory) -> Result<(), BelayError> {
    for mirror in inventory.entries.values() {
        for link in &mirror.entry.links {
            if !inventory.entries.contains_key(link_target_entry(&link.id)) {
                return validation(format!(
                    "entry {} links to missing entry {}",
                    mirror.entry.display_id, link.id
                ));
            }
        }
    }
    Ok(())
}

/// The entry part of a link target. Entry inventories are keyed by bare display
/// ID, but `docs/id-reference-standard.md` has links carry a `#sc-nnn` or
/// `#t-nnn` fragment, so the fragment must come off before the lookup. Whether
/// the fragment itself resolves is validated when the link is stored.
pub(crate) fn link_target_entry(id: &str) -> &str {
    id.split_once('#').map_or(id, |(entry, _)| entry)
}

fn unique_sibling(path: &Path, label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("belay.sqlite");
    path.with_file_name(format!(".{name}.{label}-{}-{nonce}", std::process::id()))
}

fn now() -> String {
    Local::now()
        .fixed_offset()
        .to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn import_timestamp(created_at: &str) -> Result<String, BelayError> {
    let created =
        DateTime::parse_from_rfc3339(created_at).map_err(|source| BelayError::Validation {
            message: format!("created_at must be an RFC 3339 timestamp: {source}"),
        })?;
    let current = Local::now().fixed_offset();
    Ok(if current < created { created } else { current }
        .to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}
