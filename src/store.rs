#[cfg(test)]
use std::cell::Cell;
use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use rusqlite::{Connection, OptionalExtension, Row, params};
#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
use rustix::fs::RenameFlags;
#[cfg(unix)]
use rustix::fs::{AtFlags, Mode, OFlags};
#[cfg(unix)]
use std::os::fd::OwnedFd;

use crate::database;
use crate::entry::{
    Entry, EntryLink, EntryReferenceParts, EntryStatus, EntryType, LinkRelation, MetadataValue,
    allocate_display_id, begin_immediate, normalize_id_query, parse_display_id,
    parse_entry_reference_id, split_reference_query,
};
use crate::error::BelayError;
use crate::markdown::{self, EntryChunk};
use crate::repository::Repository;

#[derive(Debug)]
pub struct ShownEntry {
    pub entry: Entry,
    pub source_path: String,
    pub inbound_links: Vec<EntryLink>,
    pub fragment: Option<ShownFragment>,
}

/// One item of an entry, addressed by a canonical `#sc-nnn` or `#t-nnn`
/// fragment. `definition` is the line that defines the fragment; `section` is
/// the body section that carries its detail, which a well-formed entry has and
/// a bare one legitimately does not.
#[derive(Debug, Clone)]
pub struct ShownFragment {
    pub fragment: String,
    pub definition: String,
    pub section: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOutcome {
    Changed,
    Unchanged,
}

#[derive(Debug, Clone)]
pub(crate) struct RouteMutationOutcome<T> {
    pub value: T,
    pub outcome: MutationOutcome,
    pub replayed: bool,
    pub post_revision: u32,
}

#[derive(Debug, Clone, Copy)]
struct RouteReceipt<'a> {
    run_id: &'a str,
    operation_id: &'a str,
    preview_hash: &'a str,
}

#[cfg(test)]
thread_local! {
    static FAIL_DIRECTORY_SYNC: Cell<bool> = const { Cell::new(false) };
}

pub fn create(
    repository: &Repository,
    entry_type: EntryType,
    title: String,
    body: String,
) -> Result<Entry, BelayError> {
    create_with_metadata(repository, entry_type, title, body, BTreeMap::new())
}

pub(crate) fn create_with_metadata(
    repository: &Repository,
    entry_type: EntryType,
    title: String,
    body: String,
    metadata: BTreeMap<String, MetadataValue>,
) -> Result<Entry, BelayError> {
    Ok(create_with_metadata_internal(repository, entry_type, title, body, metadata, None)?.value)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_create_with_metadata(
    repository: &Repository,
    run_id: &str,
    operation_id: &str,
    preview_hash: &str,
    entry_type: EntryType,
    title: String,
    body: String,
    metadata: BTreeMap<String, MetadataValue>,
) -> Result<RouteMutationOutcome<Entry>, BelayError> {
    create_with_metadata_internal(
        repository,
        entry_type,
        title,
        body,
        metadata,
        Some(RouteReceipt {
            run_id,
            operation_id,
            preview_hash,
        }),
    )
}

fn create_with_metadata_internal(
    repository: &Repository,
    entry_type: EntryType,
    title: String,
    body: String,
    metadata: BTreeMap<String, MetadataValue>,
    receipt: Option<RouteReceipt<'_>>,
) -> Result<RouteMutationOutcome<Entry>, BelayError> {
    if title.trim().is_empty() {
        return validation("entry title must not be empty");
    }
    if title.contains('\0') || body.contains('\0') {
        return validation("entry title and body must not contain NUL characters");
    }

    let now = now();
    let timestamp = format_timestamp(&now);
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    if let Some(receipt) = receipt {
        if let Some((outcome, target, post_revision)) =
            load_route_receipt(&transaction, &database_path, receipt)?
        {
            let internal_id = resolve_internal_id(&transaction, &database_path, &target)?;
            let entry = load_entry(&transaction, &database_path, internal_id)?;
            transaction.commit()?;
            return Ok(RouteMutationOutcome {
                value: entry,
                outcome,
                replayed: true,
                post_revision,
            });
        }
        let matches = find_entries_with_metadata(&transaction, &database_path, &metadata)?;
        if matches.len() > 1 {
            return Err(BelayError::Conflict {
                message: format!(
                    "multiple entries claim Route operation {} for run {}",
                    receipt.operation_id, receipt.run_id
                ),
            });
        }
        if let Some(entry) = matches.into_iter().next() {
            if entry.entry_type != entry_type
                || entry.title != title
                || entry.body != body
                || entry.status != entry_type.default_status()
                || entry.metadata != metadata
            {
                return Err(BelayError::Conflict {
                    message: format!(
                        "entry {} claims Route operation {} but does not match the approved create operation",
                        entry.display_id, receipt.operation_id
                    ),
                });
            }
            insert_route_receipt(
                &transaction,
                &database_path,
                receipt,
                MutationOutcome::Changed,
                &entry.display_id,
                entry.revision,
                &timestamp,
            )?;
            transaction.commit()?;
            return Ok(RouteMutationOutcome {
                post_revision: entry.revision,
                value: entry,
                outcome: MutationOutcome::Changed,
                replayed: true,
            });
        }
    }
    let display_id = allocate_display_id(
        &transaction,
        &repository.entries_path(),
        entry_type,
        &now,
        &title,
    )?;
    let entry = Entry {
        display_id,
        entry_type,
        title,
        status: entry_type.default_status(),
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        revision: 1,
        tags: Vec::new(),
        links: Vec::new(),
        metadata,
        body,
    }
    .normalized()?;
    let relative_path = mirror_relative_path(repository, &entry);
    let source_path = path_to_storage_string(&relative_path)?;
    let destination = repository.belay_dir.join(&relative_path);
    let rendered = markdown::render(&entry)?;
    let hash = markdown::content_hash(&entry)?;
    let chunks = markdown::generate_chunks(&entry.body);

    transaction
        .execute(
            "
            INSERT INTO entries(
                display_id, type, title, status, created_at, updated_at, revision,
                body, metadata_json, source_path, content_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                entry.display_id,
                entry.entry_type.to_string(),
                entry.title,
                entry.status.to_string(),
                entry.created_at,
                entry.updated_at,
                entry.revision,
                entry.body,
                serialize_metadata(&entry.metadata)?,
                source_path,
                hash,
            ],
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let internal_id = transaction.last_insert_rowid();
    replace_tags(&transaction, &database_path, internal_id, &entry.tags)?;
    replace_chunks_and_fts(&transaction, &database_path, internal_id, &entry, &chunks)?;
    upsert_sync_state(
        &transaction,
        &database_path,
        internal_id,
        &source_path,
        &hash,
        &timestamp,
    )?;
    write_new_file(repository, &destination, rendered.as_bytes())?;
    if let Some(receipt) = receipt {
        insert_route_receipt(
            &transaction,
            &database_path,
            receipt,
            MutationOutcome::Changed,
            &entry.display_id,
            entry.revision,
            &timestamp,
        )?;
    }
    let post_revision = entry.revision;
    transaction.commit()?;
    Ok(RouteMutationOutcome {
        value: entry,
        outcome: MutationOutcome::Changed,
        replayed: false,
        post_revision,
    })
}

pub fn show(repository: &Repository, reference: &str) -> Result<ShownEntry, BelayError> {
    let database_path = repository.database_path();
    let connection = database::open(&database_path)?;
    let reference = resolve_reference_on(&connection, &database_path, reference)?;
    let mut shown = load_shown_entry(&connection, &database_path, &reference.display_id)?;
    if let Some(fragment) = reference.fragment.as_deref() {
        // Reuse link validation so a fragment means the same thing whether it
        // is being stored or read: canonical, present, and unambiguous. An
        // invalid fragment fails here rather than degrading to whole-entry
        // output, which would silently return far more than was asked for.
        validate_reference_fragment(&connection, &database_path, &reference)?;
        let internal_id = resolve_internal_id(&connection, &database_path, &reference.display_id)?;
        let definition = crate::trace_ids::fragment_definition(
            shown.entry.entry_type,
            &shown.entry.body,
            fragment,
        )
        .ok_or_else(|| BelayError::Validation {
            message: format!(
                "fragment #{fragment} did not resolve to a definition in {}",
                reference.display_id
            ),
        })?;
        let section = load_fragment_section(&connection, &database_path, internal_id, fragment)?;
        shown.fragment = Some(ShownFragment {
            fragment: fragment.to_owned(),
            definition,
            section,
        });
    }
    Ok(shown)
}

/// The body section whose heading matches the fragment ID, read from the chunks
/// belay already generates, so sectioning here agrees with search and context
/// rather than reimplementing Markdown parsing.
fn load_fragment_section(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    fragment: &str,
) -> Result<Option<String>, BelayError> {
    let mut statement = connection
        .prepare(
            "SELECT text FROM entry_chunks
             WHERE entry_id = ?1 AND lower(section) = ?2
             ORDER BY ordinal",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let sections = statement
        .query_map(params![internal_id, fragment], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    Ok((!sections.is_empty()).then(|| sections.join("\n\n")))
}

pub fn link(
    repository: &Repository,
    from: &str,
    to: &str,
    relation: LinkRelation,
) -> Result<MutationOutcome, BelayError> {
    link_with_expected_revision(repository, from, to, relation, None)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_link_if_revision(
    repository: &Repository,
    run_id: &str,
    operation_id: &str,
    preview_hash: &str,
    from: &str,
    to: &str,
    relation: LinkRelation,
    expected_revision: u32,
) -> Result<RouteMutationOutcome<String>, BelayError> {
    link_with_expected_revision_and_receipt(
        repository,
        from,
        to,
        relation,
        Some(expected_revision),
        Some(RouteReceipt {
            run_id,
            operation_id,
            preview_hash,
        }),
    )
}

fn link_with_expected_revision(
    repository: &Repository,
    from: &str,
    to: &str,
    relation: LinkRelation,
    expected_revision: Option<u32>,
) -> Result<MutationOutcome, BelayError> {
    Ok(link_with_expected_revision_and_receipt(
        repository,
        from,
        to,
        relation,
        expected_revision,
        None,
    )?
    .outcome)
}

fn link_with_expected_revision_and_receipt(
    repository: &Repository,
    from: &str,
    to: &str,
    relation: LinkRelation,
    expected_revision: Option<u32>,
    receipt: Option<RouteReceipt<'_>>,
) -> Result<RouteMutationOutcome<String>, BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let from_display_id = resolve_display_id_on(&connection, &database_path, from)?;
    let to_reference = resolve_reference_on(&connection, &database_path, to)?;
    if from_display_id == to_reference.display_id {
        return validation("entry links must not target the same display ID");
    }

    let transaction = begin_immediate(&mut connection, &database_path)?;
    if let Some(receipt) = receipt {
        if let Some((outcome, target, post_revision)) =
            load_route_receipt(&transaction, &database_path, receipt)?
        {
            transaction.commit()?;
            return Ok(RouteMutationOutcome {
                value: target,
                outcome,
                replayed: true,
                post_revision,
            });
        }
    }
    let from_id = resolve_internal_id(&transaction, &database_path, &from_display_id)?;
    let to_id = resolve_internal_id(&transaction, &database_path, &to_reference.display_id)?;
    ensure_expected_revision(
        &transaction,
        &database_path,
        from_id,
        &from_display_id,
        expected_revision,
    )?;
    validate_reference_fragment(&transaction, &database_path, &to_reference)?;
    let expected_mirror_hash =
        ensure_no_mirror_drift(repository, &transaction, &database_path, from_id)?;
    let inserted = transaction
        .execute(
            "
            INSERT OR IGNORE INTO entry_links(
                from_entry_id, to_entry_id, to_fragment, relation, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, '{}')
            ",
            params![
                from_id,
                to_id,
                to_reference.fragment.as_deref().unwrap_or(""),
                relation.to_string()
            ],
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    if inserted == 0 {
        let target = format!(
            "{from_display_id} {relation} {}",
            to_reference.canonical_id()
        );
        let post_revision = entry_revision(&transaction, &database_path, from_id)?;
        if let Some(receipt) = receipt {
            insert_route_receipt(
                &transaction,
                &database_path,
                receipt,
                MutationOutcome::Unchanged,
                &target,
                post_revision,
                &now_string(),
            )?;
        }
        transaction.commit()?;
        return Ok(RouteMutationOutcome {
            value: target,
            outcome: MutationOutcome::Unchanged,
            replayed: false,
            post_revision,
        });
    }

    update_mutated_entry(
        repository,
        &transaction,
        &database_path,
        from_id,
        None,
        &expected_mirror_hash,
    )?;
    let post_revision = entry_revision(&transaction, &database_path, from_id)?;
    let target = format!(
        "{from_display_id} {relation} {}",
        to_reference.canonical_id()
    );
    if let Some(receipt) = receipt {
        insert_route_receipt(
            &transaction,
            &database_path,
            receipt,
            MutationOutcome::Changed,
            &target,
            post_revision,
            &now_string(),
        )?;
    }
    transaction.commit()?;
    Ok(RouteMutationOutcome {
        value: target,
        outcome: MutationOutcome::Changed,
        replayed: false,
        post_revision,
    })
}

pub fn set_status(
    repository: &Repository,
    display_id: &str,
    status: EntryStatus,
) -> Result<MutationOutcome, BelayError> {
    set_status_with_expected_revision(repository, display_id, status, None)
}

#[cfg(test)]
pub(crate) fn set_status_if_revision(
    repository: &Repository,
    display_id: &str,
    status: EntryStatus,
    expected_revision: u32,
) -> Result<MutationOutcome, BelayError> {
    set_status_with_expected_revision(repository, display_id, status, Some(expected_revision))
}

pub(crate) fn route_set_status_if_revision(
    repository: &Repository,
    run_id: &str,
    operation_id: &str,
    preview_hash: &str,
    display_id: &str,
    status: EntryStatus,
    expected_revision: u32,
) -> Result<RouteMutationOutcome<String>, BelayError> {
    set_status_with_expected_revision_and_receipt(
        repository,
        display_id,
        status,
        Some(expected_revision),
        Some(RouteReceipt {
            run_id,
            operation_id,
            preview_hash,
        }),
    )
}

fn set_status_with_expected_revision(
    repository: &Repository,
    display_id: &str,
    status: EntryStatus,
    expected_revision: Option<u32>,
) -> Result<MutationOutcome, BelayError> {
    Ok(set_status_with_expected_revision_and_receipt(
        repository,
        display_id,
        status,
        expected_revision,
        None,
    )?
    .outcome)
}

fn set_status_with_expected_revision_and_receipt(
    repository: &Repository,
    display_id: &str,
    status: EntryStatus,
    expected_revision: Option<u32>,
    receipt: Option<RouteReceipt<'_>>,
) -> Result<RouteMutationOutcome<String>, BelayError> {
    let database_path = repository.database_path();
    let mut connection = database::open(&database_path)?;
    let display_id = resolve_display_id_on(&connection, &database_path, display_id)?;
    let transaction = begin_immediate(&mut connection, &database_path)?;
    if let Some(receipt) = receipt {
        if let Some((outcome, target, post_revision)) =
            load_route_receipt(&transaction, &database_path, receipt)?
        {
            transaction.commit()?;
            return Ok(RouteMutationOutcome {
                value: target,
                outcome,
                replayed: true,
                post_revision,
            });
        }
    }
    let internal_id = resolve_internal_id(&transaction, &database_path, &display_id)?;
    ensure_expected_revision(
        &transaction,
        &database_path,
        internal_id,
        &display_id,
        expected_revision,
    )?;
    let entry_type = transaction
        .query_row(
            "SELECT type FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .parse::<EntryType>()?;
    if !entry_type.allows_status(status) {
        return validation(format!(
            "status {status} is not valid for entry type {entry_type}"
        ));
    }
    let current = transaction
        .query_row(
            "SELECT status FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .parse::<EntryStatus>()?;
    let expected_mirror_hash =
        ensure_no_mirror_drift(repository, &transaction, &database_path, internal_id)?;
    if current == status {
        let post_revision = entry_revision(&transaction, &database_path, internal_id)?;
        if let Some(receipt) = receipt {
            insert_route_receipt(
                &transaction,
                &database_path,
                receipt,
                MutationOutcome::Unchanged,
                &display_id,
                post_revision,
                &now_string(),
            )?;
        }
        transaction.commit()?;
        return Ok(RouteMutationOutcome {
            value: display_id.to_owned(),
            outcome: MutationOutcome::Unchanged,
            replayed: false,
            post_revision,
        });
    }

    transaction
        .execute(
            "UPDATE entries SET status = ?1 WHERE id = ?2",
            params![status.to_string(), internal_id],
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    update_mutated_entry(
        repository,
        &transaction,
        &database_path,
        internal_id,
        Some(status),
        &expected_mirror_hash,
    )?;
    let post_revision = entry_revision(&transaction, &database_path, internal_id)?;
    if let Some(receipt) = receipt {
        insert_route_receipt(
            &transaction,
            &database_path,
            receipt,
            MutationOutcome::Changed,
            &display_id,
            post_revision,
            &now_string(),
        )?;
    }
    transaction.commit()?;
    Ok(RouteMutationOutcome {
        value: display_id.to_owned(),
        outcome: MutationOutcome::Changed,
        replayed: false,
        post_revision,
    })
}

fn ensure_expected_revision(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    display_id: &str,
    expected_revision: Option<u32>,
) -> Result<(), BelayError> {
    let Some(expected_revision) = expected_revision else {
        return Ok(());
    };
    let actual_revision = connection
        .query_row(
            "SELECT revision FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get::<_, u32>(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    if actual_revision != expected_revision {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {display_id} revision changed: expected {expected_revision}, actual {actual_revision}"
            ),
        });
    }
    Ok(())
}

fn load_route_receipt(
    connection: &Connection,
    database_path: &Path,
    receipt: RouteReceipt<'_>,
) -> Result<Option<(MutationOutcome, String, u32)>, BelayError> {
    let row = connection
        .query_row(
            "
            SELECT preview_hash, outcome, target, post_revision
            FROM route_operation_receipts
            WHERE run_id = ?1 AND operation_id = ?2
            ",
            params![receipt.run_id, receipt.operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let Some((recorded_hash, outcome, target, post_revision)) = row else {
        return Ok(None);
    };
    if recorded_hash != receipt.preview_hash {
        return Err(BelayError::Conflict {
            message: format!(
                "Route operation {} for run {} is already bound to preview {}; requested {}",
                receipt.operation_id, receipt.run_id, recorded_hash, receipt.preview_hash
            ),
        });
    }
    let outcome = match outcome.as_str() {
        "changed" => MutationOutcome::Changed,
        "unchanged" => MutationOutcome::Unchanged,
        other => {
            return validation(format!(
                "Route operation receipt has unsupported outcome {other}"
            ));
        }
    };
    Ok(Some((outcome, target, post_revision)))
}

fn find_entries_with_metadata(
    connection: &Connection,
    database_path: &Path,
    required: &BTreeMap<String, MetadataValue>,
) -> Result<Vec<Entry>, BelayError> {
    let mut statement = connection
        .prepare("SELECT id, metadata_json FROM entries ORDER BY display_id")
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    drop(statement);
    let mut matches = Vec::new();
    for (internal_id, metadata_json) in rows {
        let metadata: BTreeMap<String, MetadataValue> = serde_json::from_str(&metadata_json)
            .map_err(|source| BelayError::Validation {
                message: format!("could not deserialize entry metadata: {source}"),
            })?;
        if required
            .iter()
            .all(|(key, value)| metadata.get(key) == Some(value))
        {
            matches.push(load_entry(connection, database_path, internal_id)?);
        }
    }
    Ok(matches)
}

fn insert_route_receipt(
    connection: &Connection,
    database_path: &Path,
    receipt: RouteReceipt<'_>,
    outcome: MutationOutcome,
    target: &str,
    post_revision: u32,
    recorded_at: &str,
) -> Result<(), BelayError> {
    let outcome = match outcome {
        MutationOutcome::Changed => "changed",
        MutationOutcome::Unchanged => "unchanged",
    };
    connection
        .execute(
            "
            INSERT INTO route_operation_receipts(
                run_id, operation_id, preview_hash, outcome, target, post_revision, recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                receipt.run_id,
                receipt.operation_id,
                receipt.preview_hash,
                outcome,
                target,
                post_revision,
                recorded_at
            ],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    Ok(())
}

fn entry_revision(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
) -> Result<u32, BelayError> {
    connection
        .query_row(
            "SELECT revision FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn now_string() -> String {
    format_timestamp(&now())
}

fn update_mutated_entry(
    repository: &Repository,
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    status: Option<EntryStatus>,
    expected_mirror_hash: &str,
) -> Result<(), BelayError> {
    let mut entry = load_entry(connection, database_path, internal_id)?;
    if let Some(status) = status {
        entry.status = status;
    }
    entry.revision = entry
        .revision
        .checked_add(1)
        .ok_or_else(|| BelayError::Validation {
            message: format!("entry {} revision overflowed", entry.display_id),
        })?;
    entry.updated_at = mutation_timestamp(&entry.created_at)?;
    entry = entry.normalized()?;
    let hash = markdown::content_hash(&entry)?;
    let rendered = markdown::render(&entry)?;
    let source_path: String = connection
        .query_row(
            "SELECT source_path FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let destination = managed_source_path(repository, &source_path)?;

    connection
        .execute(
            "
            UPDATE entries
            SET status = ?1, updated_at = ?2, revision = ?3, content_hash = ?4
            WHERE id = ?5
            ",
            params![
                entry.status.to_string(),
                entry.updated_at,
                entry.revision,
                hash,
                internal_id
            ],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    upsert_sync_state(
        connection,
        database_path,
        internal_id,
        &source_path,
        &hash,
        &entry.updated_at,
    )?;
    replace_file(
        repository,
        &destination,
        rendered.as_bytes(),
        expected_mirror_hash,
    )?;
    Ok(())
}

fn ensure_no_mirror_drift(
    repository: &Repository,
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
) -> Result<String, BelayError> {
    let (display_id, source_path, sqlite_hash, baseline_sqlite_hash, baseline_mirror_hash) =
        connection
            .query_row(
                "
                SELECT entry.display_id, entry.source_path, entry.content_hash,
                       state.sqlite_content_hash_at_last_sync,
                       state.mirror_content_hash_at_last_sync
                FROM entries entry
                JOIN sync_state state ON state.entry_id = entry.id
                WHERE entry.id = ?1
                ",
                [internal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    if sqlite_hash != baseline_sqlite_hash {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {display_id} has unsynchronized SQLite changes; run `belay sync` before mutating it"
            ),
        });
    }

    let mirror_path = managed_source_path(repository, &source_path)?;
    let contents = read_managed_file(repository, &mirror_path)?;
    let mirror = markdown::parse(&contents).map_err(|error| BelayError::Conflict {
        message: format!(
            "entry {display_id} has invalid or unsynchronized Markdown ({error}); run `belay sync` before mutating it"
        ),
    })?;
    if mirror.display_id != display_id {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {display_id} mirror contains display ID {}; run `belay sync` before mutating it",
                mirror.display_id
            ),
        });
    }
    let mirror_hash = markdown::content_hash(&mirror)?;
    if mirror_hash != baseline_mirror_hash {
        return Err(BelayError::Conflict {
            message: format!(
                "entry {display_id} has unsynchronized Markdown changes; run `belay sync` before mutating it"
            ),
        });
    }
    Ok(baseline_mirror_hash)
}

fn load_shown_entry(
    connection: &Connection,
    database_path: &Path,
    display_id: &str,
) -> Result<ShownEntry, BelayError> {
    let internal_id = resolve_internal_id(connection, database_path, display_id)?;
    let entry = load_entry(connection, database_path, internal_id)?;
    let source_path = connection
        .query_row(
            "SELECT source_path FROM entries WHERE id = ?1",
            [internal_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let inbound_links = load_links(connection, database_path, internal_id, false)?;
    Ok(ShownEntry {
        entry,
        source_path,
        inbound_links,
        fragment: None,
    })
}

pub(crate) fn load_entry(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
) -> Result<Entry, BelayError> {
    let mut entry = connection
        .query_row(
            "
            SELECT display_id, type, title, status, created_at, updated_at,
                   revision, body, metadata_json
            FROM entries
            WHERE id = ?1
            ",
            [internal_id],
            entry_from_row,
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    entry.tags = load_tags(connection, database_path, internal_id)?;
    entry.links = load_links(connection, database_path, internal_id, true)?;
    entry.normalized()
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let entry_type = row.get::<_, String>(1)?;
    let status = row.get::<_, String>(3)?;
    let metadata = row.get::<_, String>(8)?;
    Ok(Entry {
        display_id: row.get(0)?,
        entry_type: entry_type.parse().map_err(conversion_error)?,
        title: row.get(2)?,
        status: status.parse().map_err(conversion_error)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        revision: row.get(6)?,
        tags: Vec::new(),
        links: Vec::new(),
        metadata: serde_json::from_str(&metadata).map_err(conversion_error)?,
        body: row.get(7)?,
    })
}

fn load_tags(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
) -> Result<Vec<String>, BelayError> {
    let mut statement = connection
        .prepare("SELECT tag FROM entry_tags WHERE entry_id = ?1 ORDER BY tag")
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map([internal_id], |row| row.get(0))
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn load_links(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    outbound: bool,
) -> Result<Vec<EntryLink>, BelayError> {
    let sql = if outbound {
        "
        SELECT links.relation, target.display_id, links.to_fragment, links.metadata_json
        FROM entry_links links
        JOIN entries target ON target.id = links.to_entry_id
        WHERE links.from_entry_id = ?1
        ORDER BY links.relation, target.display_id, links.to_fragment
        "
    } else {
        "
        SELECT links.relation, source.display_id, '', links.metadata_json
        FROM entry_links links
        JOIN entries source ON source.id = links.from_entry_id
        WHERE links.to_entry_id = ?1
        ORDER BY links.relation, source.display_id
        "
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map([internal_id], |row| {
            let relation = row.get::<_, String>(0)?;
            let display_id = row.get::<_, String>(1)?;
            let fragment = row.get::<_, String>(2)?;
            let metadata = row.get::<_, String>(3)?;
            let id = if fragment.is_empty() {
                display_id
            } else {
                format!("{display_id}#{fragment}")
            };
            Ok(EntryLink {
                relation: relation.parse().map_err(conversion_error)?,
                id,
                metadata: serde_json::from_str(&metadata).map_err(conversion_error)?,
            })
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

pub(crate) fn resolve_internal_id(
    connection: &Connection,
    database_path: &Path,
    display_id: &str,
) -> Result<i64, BelayError> {
    connection
        .query_row(
            "SELECT id FROM entries WHERE display_id = ?1",
            [display_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .ok_or_else(|| BelayError::Validation {
            message: format!("entry {display_id} was not found"),
        })
}

pub fn resolve_reference(
    repository: &Repository,
    value: &str,
) -> Result<EntryReferenceParts, BelayError> {
    let database_path = repository.database_path();
    let connection = database::open_read_only(&database_path)?;
    resolve_reference_on(&connection, &database_path, value)
}

pub(crate) fn resolve_reference_on(
    connection: &Connection,
    database_path: &Path,
    value: &str,
) -> Result<EntryReferenceParts, BelayError> {
    let (query, fragment) = split_reference_query(value)?;
    let display_id = resolve_display_id_on(connection, database_path, &query)?;
    Ok(EntryReferenceParts {
        display_id,
        fragment,
    })
}

fn resolve_display_id_on(
    connection: &Connection,
    database_path: &Path,
    query: &str,
) -> Result<String, BelayError> {
    let query = normalize_id_query(query.trim());
    if query.is_empty() {
        return validation("entry reference must not be empty");
    }

    if let Some(display_id) = connection
        .query_row(
            "SELECT display_id FROM entries WHERE display_id = ?1",
            [&query],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| BelayError::sqlite(database_path, source))?
    {
        return Ok(display_id);
    }

    if parse_display_id(&query).is_ok() {
        return validation(format!("entry {query} was not found"));
    }

    let escaped = like_escape(&query);
    if has_entry_type_prefix(&query) {
        let pattern = format!("{escaped}%");
        let matches = prefixed_display_ids(connection, database_path, &pattern)?;
        match matches.as_slice() {
            [only] => return Ok(only.clone()),
            [] => {}
            many => return ambiguous_reference(&query, many),
        }
    }

    let slug_matches = slug_display_ids(connection, database_path, &query)?;
    match slug_matches.as_slice() {
        [only] => Ok(only.clone()),
        [] => validation(format!("entry {query} was not found")),
        many => ambiguous_reference(&query, many),
    }
}

fn has_entry_type_prefix(query: &str) -> bool {
    query.starts_with("GOAL-")
        || query.starts_with("PLN-")
        || query.starts_with("DEC-")
        || query.starts_with("WRK-")
        || query.starts_with("REV-")
        || query.starts_with("NOTE-")
}

fn like_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn prefixed_display_ids(
    connection: &Connection,
    database_path: &Path,
    pattern: &str,
) -> Result<Vec<String>, BelayError> {
    let mut statement = connection
        .prepare(
            "SELECT display_id FROM entries WHERE display_id LIKE ?1 ESCAPE '\\' ORDER BY display_id",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([pattern], |row| row.get::<_, String>(0))
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn slug_display_ids(
    connection: &Connection,
    database_path: &Path,
    query: &str,
) -> Result<Vec<String>, BelayError> {
    let mut statement = connection
        .prepare("SELECT display_id FROM entries ORDER BY display_id")
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let mut matches = Vec::new();
    for display_id in rows {
        let display_id = display_id.map_err(|source| BelayError::sqlite(database_path, source))?;
        let Ok(parts) = parse_display_id(&display_id) else {
            continue;
        };
        if parts.slug == query || parts.slug.starts_with(query) {
            matches.push(display_id);
        }
    }
    Ok(matches)
}

fn ambiguous_reference(query: &str, matches: &[String]) -> Result<String, BelayError> {
    validation(format!(
        "entry reference {query:?} is ambiguous; matches: {}",
        matches.join(", ")
    ))
}

pub(crate) fn replace_tags(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    tags: &[String],
) -> Result<(), BelayError> {
    for tag in tags {
        connection
            .execute(
                "INSERT INTO entry_tags(entry_id, tag) VALUES (?1, ?2)",
                params![internal_id, tag],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    Ok(())
}

pub(crate) fn replace_chunks_and_fts(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    entry: &Entry,
    chunks: &[EntryChunk],
) -> Result<(), BelayError> {
    connection
        .execute(
            "DELETE FROM entry_chunks WHERE entry_id = ?1",
            [internal_id],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    connection
        .execute("DELETE FROM entry_fts WHERE entry_id = ?1", [internal_id])
        .map_err(|source| BelayError::sqlite(database_path, source))?;

    for chunk in chunks {
        let token_estimate =
            i64::try_from(chunk.token_estimate).map_err(|_| BelayError::Validation {
                message: format!(
                    "chunk {} token estimate exceeds SQLite integer range",
                    chunk.ordinal
                ),
            })?;
        connection
            .execute(
                "
                INSERT INTO entry_chunks(
                    entry_id, section, ordinal, text, token_estimate, content_hash
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    internal_id,
                    chunk.section,
                    chunk.ordinal,
                    chunk.text,
                    token_estimate,
                    chunk.content_hash,
                ],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
        connection
            .execute(
                "
                INSERT INTO entry_fts(
                    entry_id, chunk_ordinal, title, body, section, chunk_text
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    internal_id,
                    chunk.ordinal,
                    entry.title,
                    entry.body,
                    chunk.section,
                    chunk.text
                ],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    if chunks.is_empty() {
        connection
            .execute(
                "
                INSERT INTO entry_fts(
                    entry_id, chunk_ordinal, title, body, section, chunk_text
                )
                VALUES (?1, 0, ?2, ?3, 'Body', '')
                ",
                params![internal_id, entry.title, entry.body],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    Ok(())
}

pub(crate) fn upsert_sync_state(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    source_path: &str,
    hash: &str,
    synced_at: &str,
) -> Result<(), BelayError> {
    connection
        .execute(
            "
            INSERT INTO sync_state(
                entry_id, source_path, sqlite_content_hash_at_last_sync,
                mirror_content_hash_at_last_sync, synced_at
            ) VALUES (?1, ?2, ?3, ?3, ?4)
            ON CONFLICT(entry_id) DO UPDATE SET
                source_path = excluded.source_path,
                sqlite_content_hash_at_last_sync =
                    excluded.sqlite_content_hash_at_last_sync,
                mirror_content_hash_at_last_sync =
                    excluded.mirror_content_hash_at_last_sync,
                synced_at = excluded.synced_at
            ",
            params![internal_id, source_path, hash, synced_at],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    Ok(())
}

pub(crate) fn insert_entry(
    connection: &Connection,
    database_path: &Path,
    entry: &Entry,
    source_path: &str,
) -> Result<i64, BelayError> {
    let hash = markdown::content_hash(entry)?;
    connection
        .execute(
            "
            INSERT INTO entries(
                display_id, type, title, status, created_at, updated_at, revision,
                body, metadata_json, source_path, content_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                entry.display_id,
                entry.entry_type.to_string(),
                entry.title,
                entry.status.to_string(),
                entry.created_at,
                entry.updated_at,
                entry.revision,
                entry.body,
                serialize_metadata(&entry.metadata)?,
                source_path,
                hash,
            ],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let internal_id = connection.last_insert_rowid();
    replace_tags(connection, database_path, internal_id, &entry.tags)?;
    let chunks = markdown::generate_chunks(&entry.body);
    replace_chunks_and_fts(connection, database_path, internal_id, entry, &chunks)?;
    Ok(internal_id)
}

pub(crate) fn replace_entry(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    entry: &Entry,
    source_path: &str,
) -> Result<(), BelayError> {
    let hash = markdown::content_hash(entry)?;
    connection
        .execute(
            "
            UPDATE entries
            SET type = ?1, title = ?2, status = ?3, created_at = ?4,
                updated_at = ?5, revision = ?6, body = ?7, metadata_json = ?8,
                source_path = ?9, content_hash = ?10
            WHERE id = ?11
            ",
            params![
                entry.entry_type.to_string(),
                entry.title,
                entry.status.to_string(),
                entry.created_at,
                entry.updated_at,
                entry.revision,
                entry.body,
                serialize_metadata(&entry.metadata)?,
                source_path,
                hash,
                internal_id,
            ],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    connection
        .execute("DELETE FROM entry_tags WHERE entry_id = ?1", [internal_id])
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    replace_tags(connection, database_path, internal_id, &entry.tags)?;
    let chunks = markdown::generate_chunks(&entry.body);
    replace_chunks_and_fts(connection, database_path, internal_id, entry, &chunks)
}

pub(crate) fn replace_links(
    connection: &Connection,
    database_path: &Path,
    internal_id: i64,
    links: &[EntryLink],
) -> Result<(), BelayError> {
    connection
        .execute(
            "DELETE FROM entry_links WHERE from_entry_id = ?1",
            [internal_id],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    for link in links {
        let target = parse_entry_reference_id(&link.id)?;
        let target_id = resolve_internal_id(connection, database_path, &target.display_id)?;
        validate_reference_fragment(connection, database_path, &target)?;
        let metadata =
            serde_json::to_string(&link.metadata).map_err(|source| BelayError::Validation {
                message: format!("could not serialize link metadata: {source}"),
            })?;
        connection
            .execute(
                "
                INSERT INTO entry_links(
                    from_entry_id, to_entry_id, to_fragment, relation, metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    internal_id,
                    target_id,
                    target.fragment.as_deref().unwrap_or(""),
                    link.relation.to_string(),
                    metadata
                ],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    Ok(())
}

pub(crate) fn validate_reference_fragment(
    connection: &Connection,
    database_path: &Path,
    reference: &crate::entry::EntryReferenceParts,
) -> Result<(), BelayError> {
    let Some(fragment) = reference.fragment.as_deref() else {
        return Ok(());
    };
    let (entry_type, body) = connection
        .query_row(
            "SELECT type, body FROM entries WHERE display_id = ?1",
            [&reference.display_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let entry_type = entry_type.parse()?;
    if !crate::trace_ids::valid_reference_fragment(entry_type, fragment) {
        return validation(format!(
            "fragment #{fragment} is not canonical for {}; expected #sc-NNN for a Goal or #t-NNN for a Plan",
            reference.display_id
        ));
    }
    match crate::trace_ids::fragment_match_count(entry_type, &body, fragment) {
        1 => Ok(()),
        0 => validation(format!(
            "fragment #{fragment} was not found in {}",
            reference.display_id
        )),
        count => validation(format!(
            "fragment #{fragment} is ambiguous in {} ({count} definitions)",
            reference.display_id
        )),
    }
}

pub(crate) fn validate_entry_reference(
    repository: &Repository,
    reference: &str,
) -> Result<(), BelayError> {
    let reference = parse_entry_reference_id(reference)?;
    let database_path = repository.database_path();
    let connection = database::open_read_only(&database_path)?;
    resolve_internal_id(&connection, &database_path, &reference.display_id)?;
    validate_reference_fragment(&connection, &database_path, &reference)
}

pub(crate) fn route_receipt_targets(
    repository: &Repository,
    run_id: &str,
    preview_hash: &str,
) -> Result<BTreeMap<String, (String, MutationOutcome)>, BelayError> {
    let database_path = repository.database_path();
    let connection = database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT operation_id, target, outcome
            FROM route_operation_receipts
            WHERE run_id = ?1 AND preview_hash = ?2
            ORDER BY operation_id
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    statement
        .query_map(params![run_id, preview_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .into_iter()
        .map(|(operation_id, target, outcome)| {
            let outcome = match outcome.as_str() {
                "changed" => Ok(MutationOutcome::Changed),
                "unchanged" => Ok(MutationOutcome::Unchanged),
                other => validation(format!(
                    "Route operation receipt has unsupported outcome {other}"
                )),
            }?;
            Ok((operation_id, (target, outcome)))
        })
        .collect()
}

pub(crate) fn serialize_metadata(
    metadata: &BTreeMap<String, MetadataValue>,
) -> Result<String, BelayError> {
    serde_json::to_string(metadata).map_err(|source| BelayError::Validation {
        message: format!("could not serialize entry metadata: {source}"),
    })
}

pub(crate) fn mirror_relative_path(repository: &Repository, entry: &Entry) -> PathBuf {
    repository
        .config
        .storage
        .entries
        .join(entry.entry_type.directory())
        .join(format!("{}.md", entry.display_id))
}

pub(crate) fn managed_source_path(
    repository: &Repository,
    source_path: &str,
) -> Result<PathBuf, BelayError> {
    let relative = Path::new(source_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || !relative.starts_with(&repository.config.storage.entries)
    {
        return validation(format!(
            "entry source path {source_path:?} is outside the managed mirror"
        ));
    }
    Ok(repository.belay_dir.join(relative))
}

pub(crate) fn path_to_storage_string(path: &Path) -> Result<String, BelayError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BelayError::Validation {
            message: format!("managed source path {} is not valid UTF-8", path.display()),
        })
}

pub(crate) fn write_new_file(
    repository: &Repository,
    path: &Path,
    contents: &[u8],
) -> Result<(), BelayError> {
    #[cfg(unix)]
    {
        let parent = open_managed_parent(repository, path)?;
        let temporary = write_temporary_at(&parent, path, contents)?;
        match rustix::fs::linkat(
            &parent.fd,
            &temporary,
            &parent.fd,
            &parent.file_name,
            AtFlags::empty(),
        ) {
            Ok(()) => {
                rustix::fs::unlinkat(&parent.fd, &temporary, AtFlags::empty())
                    .map_err(|source| unix_io("remove temporary file", path, source))?;
                sync_managed_directory(&parent)?;
                Ok(())
            }
            Err(source) => {
                let _ = rustix::fs::unlinkat(&parent.fd, &temporary, AtFlags::empty());
                Err(unix_io("create managed Markdown", path, source))
            }
        }
    }

    #[cfg(not(unix))]
    {
        write_new_file_portable(repository, path, contents)
    }
}

#[cfg(not(unix))]
fn write_new_file_portable(
    repository: &Repository,
    path: &Path,
    contents: &[u8],
) -> Result<(), BelayError> {
    validate_managed_parent(repository, path)?;
    if fs::symlink_metadata(path).is_ok() {
        return validation(format!(
            "managed Markdown path {} already exists; no file was overwritten",
            path.display()
        ));
    }
    let temporary = write_temporary(repository, path, contents)?;
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)
                .map_err(|source| BelayError::io("remove temporary file", &temporary, source))?;
            sync_parent_directory(repository, path)?;
            Ok(())
        }
        Err(source) => {
            let _ = fs::remove_file(&temporary);
            Err(BelayError::io("create managed Markdown", path, source))
        }
    }
}

pub(crate) fn replace_file(
    repository: &Repository,
    path: &Path,
    contents: &[u8],
    expected_hash: &str,
) -> Result<(), BelayError> {
    #[cfg(unix)]
    {
        let parent = open_managed_parent(repository, path)?;
        validate_regular_file_at(&parent, path)?;
        let temporary = write_temporary_at(&parent, path, contents)?;
        publish_replacement(&parent, path, &temporary, expected_hash)
    }

    #[cfg(not(unix))]
    {
        replace_file_portable(repository, path, contents, expected_hash)
    }
}

#[cfg(not(unix))]
fn replace_file_portable(
    repository: &Repository,
    path: &Path,
    contents: &[u8],
    expected_hash: &str,
) -> Result<(), BelayError> {
    validate_managed_file(repository, path)?;
    let current = fs::read_to_string(path)
        .map_err(|source| BelayError::io("read managed Markdown", path, source))?;
    ensure_expected_mirror(path, &current, expected_hash)?;
    let temporary = write_temporary(repository, path, contents)?;
    if let Err(source) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(BelayError::io("replace managed Markdown", path, source));
    }
    sync_parent_directory(repository, path)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_temporary(
    repository: &Repository,
    destination: &Path,
    contents: &[u8],
) -> Result<PathBuf, BelayError> {
    validate_managed_parent(repository, destination)?;
    let parent = destination.parent().ok_or_else(|| BelayError::Validation {
        message: format!(
            "managed Markdown path {} has no parent",
            destination.display()
        ),
    })?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| BelayError::Validation {
            message: format!(
                "managed Markdown path {} has an invalid filename",
                destination.display()
            ),
        })?;
    for attempt in 0..100_u32 {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = parent.join(format!(
            ".{file_name}.tmp-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                file.write_all(contents)
                    .map_err(|source| BelayError::io("write temporary file", &temporary, source))?;
                file.sync_all()
                    .map_err(|source| BelayError::io("flush temporary file", &temporary, source))?;
                return Ok(temporary);
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(BelayError::io("create temporary file", &temporary, source));
            }
        }
    }
    validation(format!(
        "could not allocate a temporary file beside {}",
        destination.display()
    ))
}

#[cfg(not(unix))]
fn validate_managed_file(repository: &Repository, path: &Path) -> Result<(), BelayError> {
    validate_managed_parent(repository, path)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|source| BelayError::io("inspect", path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return validation(format!(
            "managed Markdown path {} must be a regular file",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_managed_parent(repository: &Repository, path: &Path) -> Result<(), BelayError> {
    let relative =
        path.strip_prefix(&repository.belay_dir)
            .map_err(|_| BelayError::Validation {
                message: format!(
                    "managed path {} must stay within {}",
                    path.display(),
                    repository.belay_dir.display()
                ),
            })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return validation(format!(
            "managed path {} must use normal relative components",
            path.display()
        ));
    }

    require_real_directory(&repository.belay_dir)?;
    let mut current = repository.belay_dir.clone();
    let parent = relative.parent().ok_or_else(|| BelayError::Validation {
        message: format!("managed path {} has no parent", path.display()),
    })?;
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return validation(format!(
                "managed path {} must use normal relative components",
                path.display()
            ));
        };
        current.push(component);
        require_real_directory(&current)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_real_directory(path: &Path) -> Result<(), BelayError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| BelayError::io("inspect", path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return validation(format!(
            "managed path {} must be a real directory, not a symbolic link",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(repository: &Repository, path: &Path) -> Result<(), BelayError> {
    validate_managed_parent(repository, path)?;
    Ok(())
}

#[cfg(unix)]
struct ManagedParent {
    fd: OwnedFd,
    file_name: OsString,
    path: PathBuf,
}

#[cfg(unix)]
fn open_managed_parent(repository: &Repository, path: &Path) -> Result<ManagedParent, BelayError> {
    let relative =
        path.strip_prefix(&repository.belay_dir)
            .map_err(|_| BelayError::Validation {
                message: format!(
                    "managed path {} must stay within {}",
                    path.display(),
                    repository.belay_dir.display()
                ),
            })?;
    let file_name = relative
        .file_name()
        .ok_or_else(|| BelayError::Validation {
            message: format!("managed path {} has no filename", path.display()),
        })?
        .to_os_string();
    let parent = relative.parent().ok_or_else(|| BelayError::Validation {
        message: format!("managed path {} has no parent", path.display()),
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return validation(format!(
            "managed path {} must use normal relative components",
            path.display()
        ));
    }

    let directory_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = rustix::fs::open(&repository.belay_dir, directory_flags, Mode::empty())
        .map_err(|source| managed_directory_error(&repository.belay_dir, source))?;
    let mut parent_path = repository.belay_dir.clone();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return validation(format!(
                "managed path {} must use normal relative components",
                path.display()
            ));
        };
        parent_path.push(component);
        fd = rustix::fs::openat(&fd, component, directory_flags, Mode::empty())
            .map_err(|source| managed_directory_error(&parent_path, source))?;
    }
    Ok(ManagedParent {
        fd,
        file_name,
        path: parent_path,
    })
}

#[cfg(unix)]
fn validate_regular_file_at(parent: &ManagedParent, path: &Path) -> Result<(), BelayError> {
    let fd = rustix::fs::openat(
        &parent.fd,
        &parent.file_name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| unix_io("open managed Markdown", path, source))?;
    let file = fs::File::from(fd);
    let metadata = file
        .metadata()
        .map_err(|source| BelayError::io("inspect", path, source))?;
    if !metadata.is_file() {
        return validation(format!(
            "managed Markdown path {} must be a regular file",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn read_managed_file(
    repository: &Repository,
    path: &Path,
) -> Result<String, BelayError> {
    let parent = open_managed_parent(repository, path)?;
    let fd = rustix::fs::openat(
        &parent.fd,
        &parent.file_name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| unix_io("open managed Markdown", path, source))?;
    let mut file = fs::File::from(fd);
    let metadata = file
        .metadata()
        .map_err(|source| BelayError::io("inspect", path, source))?;
    if !metadata.is_file() {
        return validation(format!(
            "managed Markdown path {} must be a regular file",
            path.display()
        ));
    }
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|source| BelayError::io("read managed Markdown", path, source))?;
    Ok(contents)
}

#[cfg(not(unix))]
pub(crate) fn read_managed_file(
    repository: &Repository,
    path: &Path,
) -> Result<String, BelayError> {
    validate_managed_file(repository, path)?;
    fs::read_to_string(path).map_err(|source| BelayError::io("read managed Markdown", path, source))
}

#[cfg(unix)]
fn write_temporary_at(
    parent: &ManagedParent,
    destination: &Path,
    contents: &[u8],
) -> Result<OsString, BelayError> {
    let file_name = parent
        .file_name
        .to_str()
        .ok_or_else(|| BelayError::Validation {
            message: format!(
                "managed Markdown path {} has an invalid filename",
                destination.display()
            ),
        })?;
    for attempt in 0..100_u32 {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = OsString::from(format!(
            ".{file_name}.tmp-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match rustix::fs::openat(
            &parent.fd,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
        ) {
            Ok(fd) => {
                let mut file = fs::File::from(fd);
                file.write_all(contents).map_err(|source| {
                    BelayError::io("write temporary file", destination, source)
                })?;
                file.sync_all().map_err(|source| {
                    BelayError::io("flush temporary file", destination, source)
                })?;
                return Ok(temporary);
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => {
                return Err(unix_io("create temporary file", destination, source));
            }
        }
    }
    validation(format!(
        "could not allocate a temporary file beside {}",
        destination.display()
    ))
}

#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
fn publish_replacement(
    parent: &ManagedParent,
    path: &Path,
    temporary: &OsString,
    expected_hash: &str,
) -> Result<(), BelayError> {
    if let Err(source) = rustix::fs::renameat_with(
        &parent.fd,
        temporary,
        &parent.fd,
        &parent.file_name,
        RenameFlags::EXCHANGE,
    ) {
        let _ = rustix::fs::unlinkat(&parent.fd, temporary, AtFlags::empty());
        return Err(unix_io("exchange managed Markdown", path, source));
    }

    let previous = read_file_at(parent, temporary, path);
    let expected = match previous {
        Ok(contents) => ensure_expected_mirror(path, &contents, expected_hash),
        Err(error) => Err(error),
    };
    if let Err(error) = expected {
        let rollback = rustix::fs::renameat_with(
            &parent.fd,
            temporary,
            &parent.fd,
            &parent.file_name,
            RenameFlags::EXCHANGE,
        );
        if rollback.is_ok() {
            let _ = rustix::fs::unlinkat(&parent.fd, temporary, AtFlags::empty());
            let _ = sync_managed_directory(parent);
            return Err(error);
        }
        return Err(BelayError::Conflict {
            message: format!(
                "managed Markdown {} changed concurrently and rollback failed; preserve both files and run `belay sync`",
                path.display()
            ),
        });
    }

    rustix::fs::unlinkat(&parent.fd, temporary, AtFlags::empty())
        .map_err(|source| unix_io("remove previous managed Markdown", path, source))?;
    sync_managed_directory(parent)
}

#[cfg(all(
    unix,
    not(any(target_vendor = "apple", target_os = "linux", target_os = "android"))
))]
fn publish_replacement(
    parent: &ManagedParent,
    path: &Path,
    temporary: &OsString,
    expected_hash: &str,
) -> Result<(), BelayError> {
    let current = read_file_at(parent, &parent.file_name, path)?;
    ensure_expected_mirror(path, &current, expected_hash)?;
    if let Err(source) = rustix::fs::renameat(&parent.fd, temporary, &parent.fd, &parent.file_name)
    {
        let _ = rustix::fs::unlinkat(&parent.fd, temporary, AtFlags::empty());
        return Err(unix_io("replace managed Markdown", path, source));
    }
    sync_managed_directory(parent)
}

#[cfg(unix)]
fn read_file_at(
    parent: &ManagedParent,
    file_name: &std::ffi::OsStr,
    path: &Path,
) -> Result<String, BelayError> {
    let fd = rustix::fs::openat(
        &parent.fd,
        file_name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| unix_io("open managed Markdown", path, source))?;
    let mut file = fs::File::from(fd);
    let metadata = file
        .metadata()
        .map_err(|source| BelayError::io("inspect", path, source))?;
    if !metadata.is_file() {
        return validation(format!(
            "managed Markdown path {} must be a regular file",
            path.display()
        ));
    }
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|source| BelayError::io("read managed Markdown", path, source))?;
    Ok(contents)
}

fn ensure_expected_mirror(
    path: &Path,
    contents: &str,
    expected_hash: &str,
) -> Result<(), BelayError> {
    let entry = markdown::parse(contents).map_err(|error| BelayError::Conflict {
        message: format!(
            "managed Markdown {} changed concurrently ({error}); run `belay sync`",
            path.display()
        ),
    })?;
    let actual_hash = markdown::content_hash(&entry)?;
    if actual_hash != expected_hash {
        return Err(BelayError::Conflict {
            message: format!(
                "managed Markdown {} changed concurrently; run `belay sync`",
                path.display()
            ),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn unix_io(action: &'static str, path: &Path, source: rustix::io::Errno) -> BelayError {
    BelayError::io(
        action,
        path,
        std::io::Error::from_raw_os_error(source.raw_os_error()),
    )
}

#[cfg(unix)]
fn sync_managed_directory(parent: &ManagedParent) -> Result<(), BelayError> {
    #[cfg(test)]
    if FAIL_DIRECTORY_SYNC.replace(false) {
        return Err(BelayError::io(
            "flush directory",
            &parent.path,
            std::io::Error::other("injected directory sync failure"),
        ));
    }
    rustix::fs::fsync(&parent.fd).map_err(|source| unix_io("flush directory", &parent.path, source))
}

#[cfg(unix)]
fn managed_directory_error(path: &Path, source: rustix::io::Errno) -> BelayError {
    if matches!(source, rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) {
        BelayError::Validation {
            message: format!(
                "managed path {} must be a real directory, not a symbolic link",
                path.display()
            ),
        }
    } else {
        unix_io("open managed directory", path, source)
    }
}

fn now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

fn mutation_timestamp(created_at: &str) -> Result<String, BelayError> {
    let created =
        DateTime::parse_from_rfc3339(created_at).map_err(|source| BelayError::Validation {
            message: format!("created_at must be an RFC 3339 timestamp: {source}"),
        })?;
    let current = now();
    Ok(format_timestamp(if current < created {
        &created
    } else {
        &current
    }))
}

fn format_timestamp(timestamp: &DateTime<FixedOffset>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn conversion_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::*;
    use crate::repository;

    #[test]
    fn directory_sync_failure_leaves_markdown_for_recovery_and_rolls_back_sqlite() {
        let temporary = tempdir().expect("create temp directory");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = repository::initialize(temporary.path())
            .expect("initialize repository")
            .repository;

        FAIL_DIRECTORY_SYNC.set(true);
        let error = create(
            &repository,
            EntryType::Decision,
            "Recoverable decision".to_owned(),
            "Body".to_owned(),
        )
        .expect_err("injected directory sync must fail");
        assert_eq!(error.exit_code(), 6);

        let connection = Connection::open(repository.database_path()).expect("open database");
        let entry_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .expect("count entries");
        assert_eq!(entry_count, 0);

        let mirrors = fs::read_dir(repository.entries_path().join("decisions"))
            .expect("read mirrors")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect mirrors");
        assert_eq!(mirrors.len(), 1);
        let rendered = fs::read_to_string(mirrors[0].path()).expect("read recoverable mirror");
        let entry = markdown::parse(&rendered).expect("parse recoverable mirror");
        assert_eq!(entry.title, "Recoverable decision");

        let replacement_temporary = tempdir().expect("create replacement temp directory");
        fs::create_dir(replacement_temporary.path().join(".git"))
            .expect("create replacement repository marker");
        let replacement_repository = repository::initialize(replacement_temporary.path())
            .expect("initialize replacement repository")
            .repository;
        let created = create(
            &replacement_repository,
            EntryType::Decision,
            "Replacement decision".to_owned(),
            "Body".to_owned(),
        )
        .expect("create replacement entry");

        FAIL_DIRECTORY_SYNC.set(true);
        let error = set_status(
            &replacement_repository,
            &created.display_id,
            EntryStatus::Accepted,
        )
        .expect_err("injected replacement directory sync must fail");
        assert_eq!(error.exit_code(), 6);

        let connection =
            Connection::open(replacement_repository.database_path()).expect("open replacement DB");
        let database_state: (String, i64) = connection
            .query_row(
                "SELECT status, revision FROM entries WHERE display_id = ?1",
                [&created.display_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read rolled-back database state");
        assert_eq!(database_state, ("proposed".to_owned(), 1));

        let mirror_path = replacement_repository
            .entries_path()
            .join("decisions")
            .join(format!("{}.md", created.display_id));
        let mirror = markdown::parse(
            &fs::read_to_string(mirror_path).expect("read replacement recovery mirror"),
        )
        .expect("parse replacement recovery mirror");
        assert_eq!(mirror.status, EntryStatus::Accepted);
        assert_eq!(mirror.revision, 2);
    }

    #[test]
    fn replacement_restores_the_original_when_expected_mirror_hash_does_not_match() {
        let temporary = tempdir().expect("create temp directory");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = repository::initialize(temporary.path())
            .expect("initialize repository")
            .repository;
        let created = create(
            &repository,
            EntryType::Decision,
            "Concurrent decision".to_owned(),
            "Original body".to_owned(),
        )
        .expect("create entry");
        let mirror_path = repository
            .entries_path()
            .join("decisions")
            .join(format!("{}.md", created.display_id));
        let original = fs::read_to_string(&mirror_path).expect("read original mirror");
        let mut replacement = created;
        replacement.status = EntryStatus::Accepted;
        replacement.revision = 2;
        let rendered = markdown::render(&replacement).expect("render replacement");

        let error = replace_file(
            &repository,
            &mirror_path,
            rendered.as_bytes(),
            "unexpected-hash",
        )
        .expect_err("mismatched expected hash must fail");
        assert_eq!(error.exit_code(), 5);
        assert_eq!(
            fs::read_to_string(&mirror_path).expect("read restored mirror"),
            original
        );
    }

    #[test]
    fn expected_revision_is_checked_inside_status_mutation() {
        let temporary = tempdir().expect("create temp directory");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = repository::initialize(temporary.path())
            .expect("initialize repository")
            .repository;
        let created = create(
            &repository,
            EntryType::Goal,
            "Revision guarded goal".to_owned(),
            "## Summary\n\nGuard the mutation.".to_owned(),
        )
        .expect("create goal");
        set_status(&repository, &created.display_id, EntryStatus::Active)
            .expect("advance revision");

        let error = set_status_if_revision(
            &repository,
            &created.display_id,
            EntryStatus::Completed,
            created.revision,
        )
        .expect_err("stale expected revision must fail");
        assert_eq!(error.exit_code(), 5);
        let shown = show(&repository, &created.display_id).expect("show guarded goal");
        assert_eq!(shown.entry.status, EntryStatus::Active);
        assert_eq!(shown.entry.revision, 2);
    }

    #[test]
    fn route_create_receipt_replays_the_same_entry() {
        let temporary = tempdir().expect("create temp directory");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = repository::initialize(temporary.path())
            .expect("initialize repository")
            .repository;
        let first = route_create_with_metadata(
            &repository,
            "route-001",
            "op-001",
            "preview-hash",
            EntryType::Decision,
            "Route decision".to_owned(),
            "Durably journal this operation.".to_owned(),
            BTreeMap::new(),
        )
        .expect("apply first Route create");
        let replay = route_create_with_metadata(
            &repository,
            "route-001",
            "op-001",
            "preview-hash",
            EntryType::Decision,
            "Route decision".to_owned(),
            "Durably journal this operation.".to_owned(),
            BTreeMap::new(),
        )
        .expect("replay Route create");

        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.value.display_id, replay.value.display_id);
        let connection =
            Connection::open(repository.database_path()).expect("open Route receipt database");
        let entry_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .expect("count entries");
        let receipt_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM route_operation_receipts", [], |row| {
                row.get(0)
            })
            .expect("count receipts");
        assert_eq!(entry_count, 1);
        assert_eq!(receipt_count, 1);
    }
}
