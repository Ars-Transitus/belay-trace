use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Local, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::entry::parse_entry_reference_id;
use crate::error::BelayError;
use crate::repository::Repository;

pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub schema_version: u32,
    pub display_id: String,
    pub kind: String,
    pub verdict: String,
    pub commit_sha: String,
    pub captured_at: String,
    pub source: String,
    pub issuer: String,
    pub summary: String,
    #[serde(default)]
    pub detail: Value,
    #[serde(default)]
    pub links: Vec<EvidenceLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceLink {
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Clone)]
pub struct ShownEvidence {
    pub record: EvidenceRecord,
    pub source_path: String,
    pub source_line: usize,
}

#[derive(Debug, Clone)]
pub struct RecordInput {
    pub kind: String,
    pub verdict: String,
    pub commit_sha: Option<String>,
    pub captured_at: Option<String>,
    pub source: String,
    pub issuer: String,
    pub summary: String,
    pub detail: Value,
    pub verifies: Vec<String>,
}

/// Raw original is durable even when its derived index could not be updated.
#[derive(Debug, Clone)]
pub struct RecordedEvidence {
    pub record: EvidenceRecord,
    pub index_error: Option<String>,
}
impl std::ops::Deref for RecordedEvidence {
    type Target = EvidenceRecord;
    fn deref(&self) -> &EvidenceRecord {
        &self.record
    }
}
impl RecordedEvidence {
    pub fn indexed(&self) -> bool {
        self.index_error.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct EvidenceStatus {
    pub target: String,
    pub records: Vec<EvidenceStatusRecord>,
}

#[derive(Debug, Clone)]
pub struct EvidenceStatusRecord {
    pub display_id: String,
    pub kind: String,
    pub verdict: String,
    pub source: String,
    pub captured_at: String,
    pub commit_sha: String,
    pub freshness: Freshness,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale(String),
    Unknown(String),
}

impl Freshness {
    pub fn label(&self) -> String {
        match self {
            Self::Fresh => "fresh".to_owned(),
            Self::Stale(reason) => format!("stale ({reason})"),
            Self::Unknown(reason) => format!("unknown ({reason})"),
        }
    }

    pub fn is_fresh(&self) -> bool {
        matches!(self, Self::Fresh)
    }
}

pub fn record(repository: &Repository, input: RecordInput) -> Result<RecordedEvidence, BelayError> {
    let _guard = crate::lifecycle::writer_lock(repository)?;
    validate_kind(&input.kind)?;
    validate_verdict(&input.verdict)?;
    if input.verifies.is_empty() {
        return validation("verify record requires at least one --verifies target");
    }
    let verifies = input
        .verifies
        .iter()
        .map(|target| parse_entry_reference_id(target).map(|reference| reference.canonical_id()))
        .collect::<Result<Vec<_>, _>>()?;
    for target in &verifies {
        validate_target(repository, target)?;
    }

    let captured_at = input
        .captured_at
        .map(|value| normalize_timestamp(&value))
        .transpose()?
        .unwrap_or_else(now);
    let commit_sha = input
        .commit_sha
        .unwrap_or_else(|| current_head(repository).unwrap_or_else(|_| "unknown".to_owned()));
    let display_id = allocate_display_id(repository, &captured_at)?;
    let record = EvidenceRecord {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        display_id,
        kind: input.kind,
        verdict: input.verdict,
        commit_sha,
        captured_at,
        source: input.source,
        issuer: input.issuer,
        summary: input.summary,
        detail: input.detail,
        links: verifies
            .into_iter()
            .map(|target| EvidenceLink {
                target,
                relation: "verifies".to_owned(),
            })
            .collect(),
    };
    validate_record(&record)?;
    crate::lifecycle::ensure_v2(repository)?;
    if let Err(error) = append_mirror(repository, &record) {
        let path = repository
            .evidence_path()
            .join("records")
            .join(format!("{}.json", record.display_id));
        if path.try_exists().unwrap_or(false) {
            return Err(BelayError::StorageSummary {
                message: format!(
                    "Evidence {} publication exists at {}, but durability is unconfirmed: {error}. Inspect this ID and run sync; do not blindly resend.",
                    record.display_id,
                    path.display()
                ),
            });
        }
        return Err(error);
    }
    let database_path = repository.database_path();
    let indexed = (|| {
        if !database_path
            .try_exists()
            .map_err(|e| BelayError::io("inspect Evidence index", &database_path, e))?
        {
            return validation("Evidence index is unavailable; rebuild from originals");
        }
        let mut connection = crate::database::open(&database_path)?;
        let transaction = crate::entry::begin_immediate(&mut connection, &database_path)?;
        insert_record(&transaction, &database_path, &record)?;
        transaction.commit()?;
        Ok::<_, BelayError>(())
    })();
    Ok(RecordedEvidence {
        record,
        index_error: indexed.err().map(|e| e.to_string()),
    })
}

pub fn import_junit(
    repository: &Repository,
    path: &Path,
    verifies: Vec<String>,
) -> Result<RecordedEvidence, BelayError> {
    let contents = fs::read_to_string(path)
        .map_err(|source| BelayError::io("read JUnit XML", path, source))?;
    let failures = count_attr(&contents, "failures") + count_attr(&contents, "errors");
    let tests = count_attr(&contents, "tests");
    let skipped = count_attr(&contents, "skipped");
    let failed_names = failed_test_names(&contents);
    let verdict = if failures > 0 { "fail" } else { "pass" };
    record(
        repository,
        RecordInput {
            kind: "test".to_owned(),
            verdict: verdict.to_owned(),
            commit_sha: None,
            captured_at: None,
            source: path.display().to_string(),
            issuer: "ci:junit".to_owned(),
            summary: format!("{tests} tests, {failures} failed, {skipped} skipped"),
            detail: json!({ "failed_tests": failed_names }),
            verifies,
        },
    )
}

pub fn status(repository: &Repository, target: &str) -> Result<EvidenceStatus, BelayError> {
    let target = crate::store::resolve_reference(repository, target)?.canonical_id();
    validate_target(repository, &target)?;
    let head = current_head(repository).ok();
    let mut originals = read_located_mirrors(repository)?;
    originals.retain(|shown| shown.record.links.iter().any(|l| l.target == target));
    originals.sort_by(|a, b| {
        let left = DateTime::parse_from_rfc3339(&a.record.captured_at).ok();
        let right = DateTime::parse_from_rfc3339(&b.record.captured_at).ok();
        right
            .cmp(&left)
            .then_with(|| b.record.display_id.cmp(&a.record.display_id))
    });
    let records = originals
        .into_iter()
        .map(|shown| {
            let r = shown.record;
            let freshness = freshness(repository, head.as_deref(), &r.commit_sha, &r.captured_at);
            EvidenceStatusRecord {
                display_id: r.display_id,
                kind: r.kind,
                verdict: r.verdict,
                source: r.source,
                captured_at: r.captured_at,
                commit_sha: r.commit_sha,
                summary: r.summary,
                freshness,
            }
        })
        .collect();
    Ok(EvidenceStatus { target, records })
}

pub fn render_status(status: &EvidenceStatus) -> String {
    let mut output = format!("{}\n\n", status.target);
    if status.records.is_empty() {
        output.push_str("No evidence recorded.\n");
        return output;
    }
    let fresh = status
        .records
        .iter()
        .filter(|record| record.freshness.is_fresh())
        .count();
    for record in &status.records {
        let short = short_commit(&record.commit_sha);
        output.push_str(&format!(
            "  {:<5} {:<14} {:<18} {:<20} {:<10} {}\n",
            record.verdict,
            record.kind,
            record.source,
            record.captured_at,
            short,
            record.freshness.label()
        ));
        output.push_str(&format!("    {}\n", record.summary));
    }
    output.push_str(&format!("\nfresh: {fresh} / {}\n", status.records.len()));
    output
}

pub fn looks_like_evidence_query(query: &str) -> bool {
    let query = query.trim();
    let id = query.split_once('#').map(|(left, _)| left).unwrap_or(query);
    id == "EVD" || id.starts_with("EVD-")
}

pub fn show(repository: &Repository, query: &str) -> Result<ShownEvidence, BelayError> {
    let query = query.trim();
    if query.is_empty() {
        return validation("evidence reference must not be empty");
    }
    if query.contains('#') {
        return validation(format!(
            "evidence reference {query:?} must not include a fragment"
        ));
    }
    let records = read_located_mirrors(repository)?;
    resolve_shown(&records, query)
}

pub fn render_shown(shown: &ShownEvidence) -> String {
    let record = &shown.record;
    let detail = serde_json::to_string(&record.detail).unwrap_or_else(|_| "{}".to_owned());
    let mut output = format!(
        "ID: {}\nType: evidence\nSchema: {}\nKind: {}\nVerdict: {}\nCommit: {}\nCaptured: {}\nIssuer: {}\nSource: {}\nSummary: {}\nDetail: {}\nLinks:\n",
        record.display_id,
        record.schema_version,
        record.kind,
        record.verdict,
        record.commit_sha,
        record.captured_at,
        record.issuer,
        record.source,
        record.summary,
        detail,
    );
    if record.links.is_empty() {
        output.push_str("  none\n");
    } else {
        for link in &record.links {
            output.push_str(&format!("  - {} {}\n", link.relation, link.target));
        }
    }
    output.push_str(&format!(
        "Location: {}:{}\n",
        shown.source_path, shown.source_line
    ));
    output
}

fn resolve_shown(records: &[ShownEvidence], query: &str) -> Result<ShownEvidence, BelayError> {
    if let Some(exact) = records.iter().find(|item| item.record.display_id == query) {
        return Ok(exact.clone());
    }
    let matches: Vec<&ShownEvidence> = records
        .iter()
        .filter(|item| item.record.display_id.starts_with(query))
        .collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => validation(format!("evidence {query} was not found")),
        many => {
            let ids = many
                .iter()
                .map(|item| item.record.display_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            validation(format!(
                "evidence reference {query:?} is ambiguous; matches: {ids}"
            ))
        }
    }
}

pub fn rebuild_into(
    repository: &Repository,
    connection: &Connection,
    database_path: &Path,
) -> Result<usize, BelayError> {
    let records = read_located_mirrors(repository)?;
    replace_index(connection, database_path, &records)?;
    Ok(records.len())
}

pub fn reindex(repository: &Repository) -> Result<usize, BelayError> {
    let _guard = crate::lifecycle::writer_lock(repository)?;
    let records = read_located_mirrors(repository)?;
    let database_path = repository.database_path();
    let mut connection = crate::database::open(&database_path)?;
    let transaction = crate::entry::begin_immediate(&mut connection, &database_path)?;
    replace_index(&transaction, &database_path, &records)?;
    transaction.commit()?;
    Ok(records.len())
}

/// Verification consumers must not rely on a stale or incomplete derived index.
pub fn validate_index_sources(repository: &Repository) -> Result<(), BelayError> {
    let originals = read_located_mirrors(repository)?;
    let path = repository.database_path();
    let connection = crate::database::open_read_only(&path)?;
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM evidence", [], |r| r.get(0))
        .map_err(|e| BelayError::sqlite(&path, e))?;
    if count != originals.len() as i64 {
        return Err(BelayError::Conflict {
            message: format!(
                "Evidence originals/index drift: {} originals, {count} indexed; run belay sync",
                originals.len()
            ),
        });
    }
    for shown in originals {
        let r = shown.record;
        type IndexedFields = (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        );
        let indexed:Option<IndexedFields> = connection.query_row(
            "SELECT kind,verdict,commit_sha,captured_at,source,issuer,summary,detail_json FROM evidence WHERE display_id=?1",
            [&r.display_id], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?))
        ).optional().map_err(|e|BelayError::sqlite(&path,e))?;
        let Some((kind, verdict, commit, captured, source, issuer, summary, detail)) = indexed
        else {
            return Err(BelayError::Conflict {
                message: format!(
                    "saved Evidence {} is unindexed; run belay sync",
                    r.display_id
                ),
            });
        };
        let detail: Value = serde_json::from_str(&detail).map_err(|e| BelayError::Validation {
            message: e.to_string(),
        })?;
        let mut statement=connection.prepare("SELECT target,relation FROM evidence_links JOIN evidence ON evidence.id=evidence_links.evidence_id WHERE evidence.display_id=?1 ORDER BY target,relation").map_err(|e|BelayError::sqlite(&path,e))?;
        let links = statement
            .query_map([&r.display_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| BelayError::sqlite(&path, e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| BelayError::sqlite(&path, e))?;
        let mut expected_links = r
            .links
            .iter()
            .map(|l| (l.target.clone(), l.relation.clone()))
            .collect::<Vec<_>>();
        expected_links.sort();
        if kind != r.kind
            || verdict != r.verdict
            || commit != r.commit_sha
            || captured != r.captured_at
            || source != r.source
            || issuer != r.issuer
            || summary != r.summary
            || detail != r.detail
            || links != expected_links
        {
            return Err(BelayError::Conflict {
                message: format!(
                    "Evidence {} originals/index content drift; run belay sync",
                    r.display_id
                ),
            });
        }
    }
    Ok(())
}

fn replace_index(
    connection: &Connection,
    database_path: &Path,
    records: &[ShownEvidence],
) -> Result<(), BelayError> {
    connection
        .execute("DELETE FROM evidence_links", [])
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    connection
        .execute("DELETE FROM evidence", [])
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    for item in records {
        insert_record(connection, database_path, &item.record)?;
    }
    Ok(())
}

pub fn stale_doctor_details(
    repository: &Repository,
    connection: &Connection,
    database_path: &Path,
) -> Result<Vec<String>, BelayError> {
    let head = current_head(repository).ok();
    let mut statement = connection
        .prepare(
            "
            SELECT display_id, type, status
            FROM entries
            WHERE (type = 'goal' AND status = 'active')
               OR (type = 'decision' AND status = 'accepted')
            ORDER BY display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let entries = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let mut details = Vec::new();
    for (display_id, entry_type, status) in entries {
        let passing_evidence: Option<(String, String)> = connection
            .query_row(
                "
                SELECT evidence.commit_sha, evidence.captured_at
                FROM evidence_links links
                JOIN evidence ON evidence.id = links.evidence_id
                WHERE links.target = ?1 AND links.relation = 'verifies'
                  AND evidence.verdict = 'pass'
                ORDER BY julianday(evidence.captured_at) DESC, evidence.display_id DESC
                LIMIT 1
                ",
                [display_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|source| BelayError::sqlite(database_path, source))?;
        let Some((commit, captured_at)) = passing_evidence else {
            details.push(format!(
                "{display_id} ({entry_type}/{status}) has no passing evidence"
            ));
            continue;
        };
        match freshness(repository, head.as_deref(), &commit, &captured_at) {
            Freshness::Fresh => {}
            Freshness::Stale(reason) => details.push(format!(
                "{display_id} ({entry_type}/{status}) depends on stale evidence ({reason})"
            )),
            Freshness::Unknown(reason) => details.push(format!(
                "{display_id} ({entry_type}/{status}) has passing evidence with unknown freshness ({reason})"
            )),
        }
    }
    Ok(details)
}

pub fn latest_for_target(
    repository: &Repository,
    target: &str,
) -> Result<Vec<EvidenceStatusRecord>, BelayError> {
    Ok(status(repository, target)?
        .records
        .into_iter()
        .take(5)
        .collect())
}

pub fn insert_record(
    connection: &Connection,
    database_path: &Path,
    record: &EvidenceRecord,
) -> Result<(), BelayError> {
    validate_record(record)?;
    connection
        .execute(
            "
            INSERT INTO evidence(
                display_id, kind, verdict, commit_sha, captured_at, source,
                issuer, summary, detail_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                record.display_id,
                record.kind,
                record.verdict,
                record.commit_sha,
                record.captured_at,
                record.source,
                record.issuer,
                record.summary,
                serde_json::to_string(&record.detail).map_err(|source| BelayError::Validation {
                    message: format!("could not serialize evidence detail: {source}"),
                })?
            ],
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let internal_id: i64 = connection
        .query_row(
            "SELECT id FROM evidence WHERE display_id = ?1",
            [record.display_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    for link in &record.links {
        let target = parse_entry_reference_id(&link.target)?.canonical_id();
        connection
            .execute(
                "
                INSERT INTO evidence_links(evidence_id, target, relation)
                VALUES (?1, ?2, ?3)
                ",
                params![internal_id, target, link.relation],
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
    }
    Ok(())
}

fn append_mirror(repository: &Repository, record: &EvidenceRecord) -> Result<(), BelayError> {
    crate::lifecycle::ensure_directory(repository, Path::new("evidence/records"))?;
    let path = repository
        .evidence_path()
        .join("records")
        .join(format!("{}.json", record.display_id));
    let bytes = serde_json::to_vec(record).map_err(|e| BelayError::Validation {
        message: e.to_string(),
    })?;
    crate::store::write_new_file(repository, &path, &bytes)
}

pub fn read_located_mirrors(repository: &Repository) -> Result<Vec<ShownEvidence>, BelayError> {
    let mut by_id = BTreeMap::<String, (String, ShownEvidence)>::new();
    let loose = crate::pack::loose_evidence(repository)?;
    for (raw, _) in loose {
        let record: EvidenceRecord =
            serde_json::from_str(&raw.raw_payload).map_err(|e| BelayError::Validation {
                message: e.to_string(),
            })?;
        let shown = ShownEvidence {
            record,
            source_path: format!(".belay/{}", raw.original_path),
            source_line: raw.source_line,
        };
        merge_original(&mut by_id, raw.raw_hash, shown)?;
    }
    for packed in crate::pack::read_evidence(repository)? {
        let shown = ShownEvidence {
            record: packed.record,
            source_path: format!(
                ".belay/packs/{}.json:{}",
                packed.pack_hash, packed.original_path
            ),
            source_line: packed.source_line,
        };
        merge_original(&mut by_id, packed.raw_hash, shown)?;
    }
    Ok(by_id.into_values().map(|(_, shown)| shown).collect())
}
fn merge_original(
    records: &mut BTreeMap<String, (String, ShownEvidence)>,
    raw_hash: String,
    shown: ShownEvidence,
) -> Result<(), BelayError> {
    if let Some((old_hash, old)) = records.get(&shown.record.display_id) {
        if old_hash != &raw_hash {
            return validation(format!(
                "conflicting duplicate evidence ID {} in {}:{} and {}:{}",
                shown.record.display_id,
                old.source_path,
                old.source_line,
                shown.source_path,
                shown.source_line
            ));
        }
    } else {
        records.insert(shown.record.display_id.clone(), (raw_hash, shown));
    }
    Ok(())
}

pub(crate) fn validate_record(record: &EvidenceRecord) -> Result<(), BelayError> {
    if record.schema_version != EVIDENCE_SCHEMA_VERSION {
        return validation(format!(
            "unsupported evidence schema {}; expected {}",
            record.schema_version, EVIDENCE_SCHEMA_VERSION
        ));
    }
    validate_evidence_id(&record.display_id)?;
    validate_kind(&record.kind)?;
    validate_verdict(&record.verdict)?;
    normalize_timestamp(&record.captured_at)?;
    if record.commit_sha.trim().is_empty()
        || record.source.trim().is_empty()
        || record.issuer.trim().is_empty()
        || record.summary.trim().is_empty()
    {
        return validation("evidence commit, source, issuer, and summary must not be empty");
    }
    for link in &record.links {
        parse_entry_reference_id(&link.target)?;
        if link.relation != "verifies" && link.relation != "refutes" {
            return validation(format!("unsupported evidence relation {:?}", link.relation));
        }
    }
    Ok(())
}

fn validate_target(repository: &Repository, target: &str) -> Result<(), BelayError> {
    let reference = parse_entry_reference_id(target)?;
    let database_path = repository.database_path();
    let connection = match crate::database::open_read_only(&database_path) {
        Ok(connection) => connection,
        Err(_) => return validate_original_target(repository, &reference),
    };
    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM entries WHERE display_id = ?1",
            [&reference.display_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    if exists.is_none() {
        return validation(format!("evidence target {target} was not found"));
    }
    crate::store::validate_reference_fragment(&connection, &database_path, &reference)?;
    Ok(())
}

fn validate_original_target(
    repository: &Repository,
    reference: &crate::entry::EntryReferenceParts,
) -> Result<(), BelayError> {
    let mut entries = crate::pack::loose_entries(repository)?;
    entries.extend(
        crate::pack::read_entries(repository)?
            .into_iter()
            .map(|p| p.entry),
    );
    let selected = entries
        .into_iter()
        .filter(|e| e.display_id == reference.display_id)
        .max_by_key(|e| e.revision)
        .ok_or_else(|| BelayError::Validation {
            message: format!("Evidence target {} has no original", reference.display_id),
        })?;
    if let Some(fragment) = &reference.fragment {
        if crate::trace_ids::fragment_definition(selected.entry_type, &selected.body, fragment)
            .is_none()
        {
            return validation(format!(
                "Evidence target {} fragment #{fragment} missing or ambiguous in original",
                selected.display_id
            ));
        }
    }
    Ok(())
}

fn validate_kind(kind: &str) -> Result<(), BelayError> {
    if matches!(
        kind,
        "test"
            | "ci-run"
            | "lint"
            | "type-check"
            | "bench"
            | "security-scan"
            | "human-approval"
            | "llm-eval"
            | "metric"
    ) {
        Ok(())
    } else {
        validation(format!("unsupported evidence kind {kind:?}"))
    }
}

fn validate_verdict(verdict: &str) -> Result<(), BelayError> {
    if matches!(verdict, "pass" | "fail" | "warn" | "info") {
        Ok(())
    } else {
        validation(format!("unsupported evidence verdict {verdict:?}"))
    }
}

fn allocate_display_id(_repository: &Repository, _captured_at: &str) -> Result<String, BelayError> {
    Ok(format!("EVD-{}", crate::lifecycle::random_id()?))
}

pub(crate) fn validate_evidence_id(value: &str) -> Result<(), BelayError> {
    if let Some(hex) = value.strip_prefix("EVD-") {
        if hex.len() == 32
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Ok(());
        }
    }
    let mut parts = value.split('-');
    if parts.next() != Some("EVD") {
        return validation(format!("invalid evidence ID {value:?}"));
    }
    let timestamp = parts.next().unwrap_or_default();
    let sequence = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%S").is_err()
        || sequence.len() != 3
        || sequence
            .parse::<u16>()
            .ok()
            .is_none_or(|number| number == 0)
    {
        return validation(format!("invalid evidence ID {value:?}"));
    }
    Ok(())
}

fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn normalize_timestamp(value: &str) -> Result<String, BelayError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, false))
        .map_err(|source| BelayError::Validation {
            message: format!("timestamp must be RFC3339: {source}"),
        })
}

pub fn current_head(repository: &Repository) -> Result<String, BelayError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository.root)
        .output()
        .map_err(|source| BelayError::io("run git rev-parse", &repository.root, source))?;
    if !output.status.success() {
        return validation("git rev-parse HEAD failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn freshness(
    repository: &Repository,
    head: Option<&str>,
    commit_sha: &str,
    captured_at: &str,
) -> Freshness {
    freshness_at(repository, head, commit_sha, captured_at, Utc::now())
}

pub(crate) fn freshness_at(
    repository: &Repository,
    head: Option<&str>,
    commit_sha: &str,
    captured_at: &str,
    now: DateTime<Utc>,
) -> Freshness {
    let Ok(captured_at) = DateTime::parse_from_rfc3339(captured_at) else {
        return Freshness::Unknown("captured-at invalid".to_owned());
    };
    let captured_at = captured_at.with_timezone(&Utc);
    if captured_at > now {
        return Freshness::Unknown("captured-at is in the future".to_owned());
    }
    if commit_sha == "unknown" {
        return time_freshness(repository, captured_at, now, "commit unknown");
    }
    let Some(head) = head else {
        return time_freshness(repository, captured_at, now, "git unavailable");
    };
    if commit_sha == head {
        return Freshness::Fresh;
    }
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{commit_sha}..{head}")])
        .current_dir(&repository.root)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let behind = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
                .unwrap_or(u32::MAX);
            if behind <= repository.config.verify.stale_after_commits {
                Freshness::Fresh
            } else {
                Freshness::Stale(format!("{behind} commits behind"))
            }
        }
        Ok(_) => Freshness::Stale("not HEAD".to_owned()),
        Err(_) => time_freshness(repository, captured_at, now, "git unavailable"),
    }
}

pub(crate) fn record_freshness_at(
    repository: &Repository,
    record: &EvidenceRecord,
    as_of: DateTime<Utc>,
) -> Freshness {
    let head = current_head(repository).ok();
    freshness_at(
        repository,
        head.as_deref(),
        &record.commit_sha,
        &record.captured_at,
        as_of,
    )
}

fn time_freshness(
    repository: &Repository,
    captured_at: DateTime<Utc>,
    now: DateTime<Utc>,
    fallback_reason: &str,
) -> Freshness {
    let Some(threshold) =
        chrono::Duration::try_days(i64::from(repository.config.verify.stale_after_days))
    else {
        return Freshness::Fresh;
    };
    if now.signed_duration_since(captured_at) <= threshold {
        Freshness::Fresh
    } else {
        Freshness::Stale(format!(
            "older than {} days ({fallback_reason})",
            repository.config.verify.stale_after_days
        ))
    }
}

fn count_attr(contents: &str, attr: &str) -> u64 {
    let pattern = format!("{attr}=\"");
    contents
        .split(&pattern)
        .skip(1)
        .filter_map(|tail| tail.split('"').next()?.parse::<u64>().ok())
        .sum()
}

fn failed_test_names(contents: &str) -> Vec<String> {
    contents
        .split("<testcase")
        .skip(1)
        .filter(|case| case.contains("<failure") || case.contains("<error"))
        .filter_map(|case| {
            let name = case.split("name=\"").nth(1)?.split('"').next()?;
            Some(name.to_owned())
        })
        .collect()
}

fn short_commit(commit: &str) -> String {
    if commit.len() > 12 {
        commit[..12].to_owned()
    } else {
        commit.to_owned()
    }
}

fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn repository(stale_after_days: u32) -> Repository {
        let mut config = Config::default();
        config.verify.stale_after_days = stale_after_days;
        Repository {
            root: std::env::temp_dir(),
            belay_dir: std::env::temp_dir().join(".belay"),
            config,
        }
    }

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn time_fallback_is_inclusive_and_applies_when_git_or_commit_is_unavailable() {
        let repository = repository(14);
        let now = utc("2026-07-22T00:00:00Z");

        assert_eq!(
            freshness_at(&repository, None, "abc123", "2026-07-08T00:00:00Z", now),
            Freshness::Fresh
        );
        assert!(matches!(
            freshness_at(
                &repository,
                Some("head"),
                "unknown",
                "2026-07-07T23:59:59Z",
                now
            ),
            Freshness::Stale(reason) if reason == "older than 14 days (commit unknown)"
        ));
    }

    #[test]
    fn time_fallback_preserves_subsecond_precision_at_the_boundary() {
        let repository = repository(14);
        let now = utc("2026-07-22T00:00:00.500000000Z");

        assert_eq!(
            freshness_at(
                &repository,
                None,
                "abc123",
                "2026-07-08T00:00:00.500000000Z",
                now
            ),
            Freshness::Fresh
        );
        assert!(matches!(
            freshness_at(
                &repository,
                None,
                "abc123",
                "2026-07-08T00:00:00.499999999Z",
                now
            ),
            Freshness::Stale(_)
        ));
    }

    #[test]
    fn time_fallback_does_not_claim_invalid_or_future_evidence_is_fresh() {
        let repository = repository(14);
        let now = utc("2026-07-22T00:00:00Z");

        assert_eq!(
            freshness_at(&repository, Some("head"), "head", "invalid", now),
            Freshness::Unknown("captured-at invalid".to_owned())
        );
        assert_eq!(
            freshness_at(
                &repository,
                Some("head"),
                "head",
                "2026-07-22T00:00:01Z",
                now
            ),
            Freshness::Unknown("captured-at is in the future".to_owned())
        );
    }

    #[test]
    fn matching_head_takes_precedence_over_evidence_age() {
        let repository = repository(14);
        assert_eq!(
            freshness_at(
                &repository,
                Some("head"),
                "head",
                "2020-01-01T00:00:00Z",
                utc("2026-07-22T00:00:00Z")
            ),
            Freshness::Fresh
        );
    }

    fn sample_json(id: &str) -> String {
        format!(
            r#"{{"schema_version":1,"display_id":"{id}","kind":"test","verdict":"pass","commit_sha":"abc123def456","captured_at":"2026-09-01T12:00:00+09:00","source":"cargo test","issuer":"cli-test","summary":"mirror-only record","detail":{{"fixture":true}},"links":[{{"target":"GOAL-20260901T120000-001-example","relation":"verifies"}}]}}"#
        )
    }

    fn mirror_repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".belay/evidence")).expect("evidence dir");
        let repository = Repository {
            root: dir.path().to_path_buf(),
            belay_dir: dir.path().join(".belay"),
            config: Config::default(),
        };
        (dir, repository)
    }

    #[test]
    fn evidence_query_shape_is_prefix_based_and_ignores_entry_ids() {
        assert!(looks_like_evidence_query("EVD"));
        assert!(looks_like_evidence_query("EVD-20260901T120000-001"));
        assert!(looks_like_evidence_query("EVD-20260901T120000-001#sc-001"));
        assert!(!looks_like_evidence_query(
            "DEC-20260901T120000-001-use-sqlite"
        ));
        assert!(!looks_like_evidence_query("retrieval-hygiene-archive"));
        assert!(!looks_like_evidence_query("evd-20260901T120000-001"));
    }

    #[test]
    fn show_resolves_exact_id_from_ndjson_without_sqlite() {
        let (_dir, repository) = mirror_repo();
        let id = "EVD-20260901T120000-001";
        std::fs::write(
            repository.evidence_path().join("2026-09.ndjson"),
            format!("{}\n", sample_json(id)),
        )
        .expect("write mirror");

        let shown = show(&repository, id).expect("show exact evidence");
        assert_eq!(shown.record.display_id, id);
        assert_eq!(shown.record.schema_version, 1);
        assert_eq!(shown.record.kind, "test");
        assert_eq!(shown.record.verdict, "pass");
        assert_eq!(shown.record.commit_sha, "abc123def456");
        assert_eq!(shown.record.captured_at, "2026-09-01T12:00:00+09:00");
        assert_eq!(shown.record.issuer, "cli-test");
        assert_eq!(shown.record.source, "cargo test");
        assert_eq!(shown.record.summary, "mirror-only record");
        assert_eq!(shown.source_line, 1);
        assert!(
            shown.source_path.ends_with("2026-09.ndjson"),
            "{}",
            shown.source_path
        );

        let rendered = render_shown(&shown);
        for expected in [
            "ID: EVD-20260901T120000-001",
            "Type: evidence",
            "Schema: 1",
            "Kind: test",
            "Verdict: pass",
            "Commit: abc123def456",
            "Captured: 2026-09-01T12:00:00+09:00",
            "Issuer: cli-test",
            "Source: cargo test",
            "Summary: mirror-only record",
            r#"Detail: {"fixture":true}"#,
            "verifies GOAL-20260901T120000-001-example",
            "Location:",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected} in {rendered}"
            );
        }
    }

    fn write_ndjson(repository: &Repository, name: &str, lines: &[&str]) {
        std::fs::write(
            repository.evidence_path().join(name),
            format!("{}\n", lines.join("\n")),
        )
        .expect("write ndjson");
    }

    fn show_err(repository: &Repository, query: &str) -> String {
        show(repository, query)
            .expect_err("show should fail")
            .to_string()
    }

    #[test]
    fn unique_prefix_resolves_one_evidence_record() {
        let (_dir, repository) = mirror_repo();
        write_ndjson(
            &repository,
            "2026-09.ndjson",
            &[
                &sample_json("EVD-20260901T120000-001"),
                &sample_json("EVD-20260901T120100-001"),
            ],
        );
        let shown = show(&repository, "EVD-20260901T120000").expect("unique prefix");
        assert_eq!(shown.record.display_id, "EVD-20260901T120000-001");
    }

    #[test]
    fn show_fails_closed_for_fragment_zero_ambiguous_duplicate_and_malformed() {
        let (_dir, repository) = mirror_repo();
        write_ndjson(
            &repository,
            "2026-09.ndjson",
            &[
                &sample_json("EVD-20260901T120000-001"),
                &sample_json("EVD-20260901T120100-001"),
            ],
        );

        let fragment = show_err(&repository, "EVD-20260901T120000-001#sc-001");
        assert!(
            fragment.contains("must not include a fragment"),
            "{fragment}"
        );

        let missing = show_err(&repository, "EVD-20260801T000000-001");
        assert!(missing.contains("was not found"), "{missing}");

        let ambiguous = show_err(&repository, "EVD-20260901");
        assert!(ambiguous.contains("ambiguous"), "{ambiguous}");
        assert!(ambiguous.contains("EVD-20260901T120000-001"), "{ambiguous}");
        assert!(ambiguous.contains("EVD-20260901T120100-001"), "{ambiguous}");

        let (_dup_dir, duplicate_repo) = mirror_repo();
        write_ndjson(
            &duplicate_repo,
            "2026-08.ndjson",
            &[&sample_json("EVD-20260901T120000-001")],
        );
        write_ndjson(
            &duplicate_repo,
            "2026-09.ndjson",
            &[&sample_json("EVD-20260901T120000-001")
                .replace("mirror-only record", "conflicting record")],
        );
        let duplicate = show_err(&duplicate_repo, "EVD-20260901T120000-001");
        assert!(duplicate.contains("duplicate evidence ID"), "{duplicate}");
        assert!(duplicate.contains("2026-08.ndjson:1"), "{duplicate}");
        assert!(duplicate.contains("2026-09.ndjson:1"), "{duplicate}");

        let cases = [
            (r#"{"not":"json""#.to_owned(), "invalid evidence JSON"),
            (
                sample_json("EVD-20260901T120000-001")
                    .replace(r#""schema_version":1"#, r#""schema_version":2"#),
                "unsupported evidence schema 2",
            ),
            (
                sample_json("EVD-20260901T120000-001")
                    .replace(r#""kind":"test""#, r#""kind":"joke""#),
                "unsupported evidence kind",
            ),
            (
                sample_json("EVD-20260901T120000-001")
                    .replace(r#""verdict":"pass""#, r#""verdict":"maybe""#),
                "unsupported evidence verdict",
            ),
            (
                sample_json("EVD-20260901T120000-001")
                    .replace(r#""relation":"verifies""#, r#""relation":"implements""#),
                "unsupported evidence relation",
            ),
            (
                sample_json("EVD-20260901T120000-001").replace(
                    r#""target":"GOAL-20260901T120000-001-example""#,
                    r#""target":"not-an-id""#,
                ),
                "invalid display ID",
            ),
        ];
        for (line, expected) in cases {
            let (_case_dir, case_repo) = mirror_repo();
            write_ndjson(&case_repo, "2026-09.ndjson", &[line.as_str()]);
            let error = show_err(&case_repo, "EVD-20260901T120000-001");
            assert!(
                error.contains(expected),
                "expected {expected:?} in {error} for {line}"
            );
        }
    }
}
