use rusqlite::params;

use crate::entry::{EntryStatus, EntryType};
use crate::error::BelayError;
use crate::repository::Repository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCandidate {
    pub display_id: String,
    pub entry_type: EntryType,
    pub status: EntryStatus,
    pub title: String,
    pub reasons: Vec<&'static str>,
}

pub fn candidates(repository: &Repository) -> Result<Vec<ArchiveCandidate>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, display_id, type, status, title
            FROM entries
            WHERE status != 'archived'
            ORDER BY updated_at DESC, display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(&database_path, source))?;

    let mut results = Vec::new();
    for row in rows {
        let (internal_id, display_id, entry_type, status, title) =
            row.map_err(|source| BelayError::sqlite(&database_path, source))?;
        let entry_type = entry_type.parse::<EntryType>()?;
        let status = status.parse::<EntryStatus>()?;
        let mut reasons = Vec::new();
        if matches!(
            status,
            EntryStatus::Completed
                | EntryStatus::Abandoned
                | EntryStatus::Superseded
                | EntryStatus::Rejected
        ) {
            reasons.push("terminal-status");
        }
        if !has_live_inbound(&connection, &database_path, internal_id)? {
            reasons.push("no-live-inbound");
        }
        if is_superseded_target(&connection, &database_path, internal_id)? {
            reasons.push("superseded-old-side");
        }
        // `no-live-inbound` alone would list every live-unlinked draft; require
        // a terminal or supersedes reason before offering the entry.
        let offered = reasons
            .iter()
            .any(|reason| matches!(*reason, "terminal-status" | "superseded-old-side"));
        if !offered {
            continue;
        }
        results.push(ArchiveCandidate {
            display_id,
            entry_type,
            status,
            title,
            reasons,
        });
    }
    Ok(results)
}

pub fn render_candidates(candidates: &[ArchiveCandidate]) -> String {
    if candidates.is_empty() {
        return "No archive candidates.\n".to_owned();
    }
    let mut output = String::from("Archive candidates\n");
    for candidate in candidates {
        output.push_str(&format!(
            "- {} [{}] {}: {}\n",
            candidate.display_id,
            candidate.status,
            candidate.title,
            candidate.reasons.join(", ")
        ));
    }
    output.push_str(
        "Review these entries, then `belay status <id> archived` to hide them from default retrieval.\n",
    );
    output
}

fn has_live_inbound(
    connection: &rusqlite::Connection,
    database_path: &std::path::Path,
    target_id: i64,
) -> Result<bool, BelayError> {
    let mut statement = connection
        .prepare(
            "
            SELECT source.type, source.status
            FROM entry_links links
            JOIN entries source ON source.id = links.from_entry_id
            WHERE links.to_entry_id = ?1
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let rows = statement
        .query_map([target_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    for row in rows {
        let (entry_type, status) =
            row.map_err(|source| BelayError::sqlite(database_path, source))?;
        let entry_type = entry_type.parse::<EntryType>()?;
        let status = status.parse::<EntryStatus>()?;
        if entry_type.is_live_status(status) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_superseded_target(
    connection: &rusqlite::Connection,
    database_path: &std::path::Path,
    target_id: i64,
) -> Result<bool, BelayError> {
    let count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM entry_links
            WHERE to_entry_id = ?1 AND relation = 'supersedes'
            ",
            params![target_id],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    Ok(count > 0)
}
