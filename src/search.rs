use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params_from_iter};

use crate::entry::{EntryStatus, EntryType};
use crate::error::BelayError;
use crate::repository::Repository;

const MAX_RESULTS: usize = 100;
const EXCERPT_CHARS: usize = 320;

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub entry_type: Option<EntryType>,
    pub status_include: Vec<EntryStatus>,
    pub status_exclude: Vec<EntryStatus>,
    pub tag: Option<String>,
    pub display_id: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub internal_id: i64,
    pub display_id: String,
    pub entry_type: EntryType,
    pub title: String,
    pub status: EntryStatus,
    pub tags: Vec<String>,
    pub source_path: String,
    pub score: Option<f64>,
    pub reason: String,
    pub section: String,
    pub excerpt: String,
    pub match_count: usize,
    pub chunk_ordinal: u32,
}

#[derive(Debug)]
struct RawResult {
    internal_id: i64,
    display_id: String,
    entry_type: String,
    title: String,
    status: String,
    source_path: String,
    section: String,
    excerpt: String,
    body: String,
    chunk_ordinal: i64,
    score: Option<f64>,
}

pub fn search(
    repository: &Repository,
    request: &SearchRequest,
) -> Result<Vec<SearchResult>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open(&database_path)?;
    search_connection(&connection, &database_path, request)
}

/// Search a caller-owned SQLite generation. Browse uses this entry point so its
/// results have exactly the same FTS5/BM25 and filter semantics as the CLI.
pub fn search_connection(
    connection: &Connection,
    database_path: &std::path::Path,
    request: &SearchRequest,
) -> Result<Vec<SearchResult>, BelayError> {
    validate_request(request)?;
    let exact_id = match &request.display_id {
        Some(display_id) => Some(display_id.clone()),
        None if !request.query.trim().is_empty() => connection
            .query_row(
                "SELECT display_id FROM entries WHERE display_id = ?1",
                [request.query.trim()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| BelayError::sqlite(database_path, source))?,
        None => None,
    };

    if let Some(display_id) = exact_id {
        return exact_result(connection, database_path, request, &display_id)
            .map(|result| result.into_iter().collect());
    }
    if request.query.trim().is_empty() {
        return structured_results(connection, database_path, request);
    }
    keyword_results(connection, database_path, request)
}

pub fn linked_results(
    repository: &Repository,
    seed_results: &[SearchResult],
    limit: usize,
) -> Result<Vec<SearchResult>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open(&database_path)?;
    let seed_ids = seed_results
        .iter()
        .map(|result| result.internal_id)
        .collect::<HashSet<_>>();
    let mut results = Vec::<SearchResult>::new();
    let mut seen = HashSet::<i64>::new();

    for seed in seed_results {
        let mut statement = connection
            .prepare(
                "
                SELECT links.from_entry_id, source.display_id,
                       links.to_entry_id, target.display_id, links.to_fragment, links.relation
                FROM entry_links links
                JOIN entries source ON source.id = links.from_entry_id
                JOIN entries target ON target.id = links.to_entry_id
                WHERE links.from_entry_id = ?1 OR links.to_entry_id = ?1
                ORDER BY links.relation, source.display_id, target.display_id
                ",
            )
            .map_err(|source| BelayError::sqlite(&database_path, source))?;
        let links = statement
            .query_map([seed.internal_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|source| BelayError::sqlite(&database_path, source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| BelayError::sqlite(&database_path, source))?;

        for (from_id, from_display_id, to_id, to_display_id, to_fragment, relation) in links {
            let target_reference = if to_fragment.is_empty() {
                to_display_id.clone()
            } else {
                format!("{to_display_id}#{to_fragment}")
            };
            let (linked_id, linked_display_id, reason) = if from_id == seed.internal_id {
                (
                    to_id,
                    to_display_id,
                    format!(
                        "linked from {} via {} to {}",
                        seed.display_id, relation, target_reference
                    ),
                )
            } else {
                (
                    from_id,
                    from_display_id,
                    format!("links to {} via {}", seed.display_id, relation),
                )
            };
            if seed_ids.contains(&linked_id) || !seen.insert(linked_id) {
                continue;
            }
            if let Some(mut result) =
                load_entry_result(&connection, &database_path, &linked_display_id)?
            {
                result.reason = reason;
                results.push(result);
            }
        }
    }

    results.truncate(limit);
    Ok(results)
}

fn validate_request(request: &SearchRequest) -> Result<(), BelayError> {
    if request.limit == 0 || request.limit > MAX_RESULTS {
        return Err(BelayError::Validation {
            message: format!("search limit must be between 1 and {MAX_RESULTS}"),
        });
    }
    if request.query.trim().is_empty()
        && request.entry_type.is_none()
        && request.status_include.is_empty()
        && request.status_exclude.is_empty()
        && request.tag.is_none()
        && request.display_id.is_none()
    {
        return Err(BelayError::Validation {
            message: "search requires a query or at least one structured filter".to_owned(),
        });
    }
    let include = unique_statuses(&request.status_include);
    let exclude = unique_statuses(&request.status_exclude);
    if include.iter().any(|status| exclude.contains(status)) {
        return Err(BelayError::Validation {
            message: "status include and exclude must not overlap".to_owned(),
        });
    }
    if request
        .tag
        .as_ref()
        .is_some_and(|tag| tag.trim().is_empty())
    {
        return Err(BelayError::Validation {
            message: "search tag must not be empty".to_owned(),
        });
    }
    if let Some(display_id) = &request.display_id {
        crate::entry::parse_display_id(display_id)?;
    }
    Ok(())
}

/// Hide `archived` from default retrieval unless the caller opted in or
/// explicitly included that status.
pub fn apply_default_archived_exclude(
    include: &[EntryStatus],
    exclude: &mut Vec<EntryStatus>,
    include_archived: bool,
) {
    if include_archived || include.contains(&EntryStatus::Archived) {
        return;
    }
    if !exclude.contains(&EntryStatus::Archived) {
        exclude.push(EntryStatus::Archived);
    }
}

struct FilterBind {
    predicate: String,
    values: Vec<Value>,
    next_index: usize,
}

impl FilterBind {
    fn new(request: &SearchRequest, start_index: usize) -> Self {
        let mut index = start_index;
        let mut clauses = Vec::new();
        let mut values = Vec::new();

        let type_index = index;
        index += 1;
        clauses.push(format!(
            "(?{type_index} IS NULL OR entry.type = ?{type_index})"
        ));
        values.push(optional_text(
            request.entry_type.map(|value| value.to_string()),
        ));

        let include = unique_statuses(&request.status_include);
        if !include.is_empty() {
            let placeholders = numbered_placeholders(&mut index, include.len());
            clauses.push(format!("entry.status IN ({placeholders})"));
            values.extend(
                include
                    .into_iter()
                    .map(|status| Value::Text(status.to_string())),
            );
        }
        let exclude = unique_statuses(&request.status_exclude);
        if !exclude.is_empty() {
            let placeholders = numbered_placeholders(&mut index, exclude.len());
            clauses.push(format!("entry.status NOT IN ({placeholders})"));
            values.extend(
                exclude
                    .into_iter()
                    .map(|status| Value::Text(status.to_string())),
            );
        }

        let tag_index = index;
        index += 1;
        clauses.push(format!(
            "(?{tag_index} IS NULL OR EXISTS (
                  SELECT 1 FROM entry_tags tag
                  WHERE tag.entry_id = entry.id AND tag.tag = ?{tag_index}
              ))"
        ));
        values.push(optional_text(request.tag.clone()));

        Self {
            predicate: clauses.join(" AND "),
            values,
            next_index: index,
        }
    }
}

fn numbered_placeholders(index: &mut usize, count: usize) -> String {
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        parts.push(format!("?{index}"));
        *index += 1;
    }
    parts.join(", ")
}

fn unique_statuses(values: &[EntryStatus]) -> Vec<EntryStatus> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for status in values {
        if seen.insert(*status) {
            unique.push(*status);
        }
    }
    unique
}

fn optional_text(value: Option<String>) -> Value {
    match value {
        Some(text) => Value::Text(text),
        None => Value::Null,
    }
}

fn exact_result(
    connection: &Connection,
    database_path: &std::path::Path,
    request: &SearchRequest,
    display_id: &str,
) -> Result<Option<SearchResult>, BelayError> {
    let filters = FilterBind::new(request, 2);
    let sql = format!(
        "
            SELECT entry.id, entry.display_id, entry.type, entry.title, entry.status,
                   entry.source_path,
                   COALESCE((
                       SELECT chunk.section FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), 'Body'),
                   COALESCE((
                       SELECT chunk.text FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), entry.body),
                   entry.body,
                   COALESCE((
                       SELECT chunk.ordinal FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), 0)
            FROM entries entry
            WHERE entry.display_id = ?1
              AND {}
            ",
        filters.predicate
    );
    let mut values = vec![Value::Text(display_id.to_owned())];
    values.extend(filters.values);
    let raw = connection
        .query_row(&sql, params_from_iter(values), |row| {
            raw_from_row(row, None)
        })
        .optional()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    raw.map(|raw| finalize_result(connection, database_path, raw, "exact display-ID match"))
        .transpose()
}

fn load_entry_result(
    connection: &Connection,
    database_path: &std::path::Path,
    display_id: &str,
) -> Result<Option<SearchResult>, BelayError> {
    exact_result(
        connection,
        database_path,
        &SearchRequest {
            query: String::new(),
            entry_type: None,
            status_include: Vec::new(),
            status_exclude: Vec::new(),
            tag: None,
            display_id: Some(display_id.to_owned()),
            limit: 1,
        },
        display_id,
    )
}

fn structured_results(
    connection: &Connection,
    database_path: &std::path::Path,
    request: &SearchRequest,
) -> Result<Vec<SearchResult>, BelayError> {
    let filters = FilterBind::new(request, 1);
    let limit = i64::try_from(request.limit).expect("validated search limit fits i64");
    let limit_index = filters.next_index;
    let sql = format!(
        "
            SELECT entry.id, entry.display_id, entry.type, entry.title, entry.status,
                   entry.source_path,
                   COALESCE((
                       SELECT chunk.section FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), 'Body'),
                   COALESCE((
                       SELECT chunk.text FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), entry.body),
                   entry.body,
                   COALESCE((
                       SELECT chunk.ordinal FROM entry_chunks chunk
                       WHERE chunk.entry_id = entry.id ORDER BY chunk.ordinal LIMIT 1
                   ), 0)
            FROM entries entry
            WHERE {}
            ORDER BY entry.type, entry.display_id
            LIMIT ?{limit_index}
            ",
        filters.predicate
    );
    let mut values = filters.values;
    values.push(Value::Integer(limit));
    let mut statement = connection
        .prepare(&sql)
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let raw = statement
        .query_map(params_from_iter(values), |row| raw_from_row(row, None))
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    raw.into_iter()
        .map(|raw| finalize_result(connection, database_path, raw, "matched structured filters"))
        .collect()
}

fn keyword_results(
    connection: &Connection,
    database_path: &std::path::Path,
    request: &SearchRequest,
) -> Result<Vec<SearchResult>, BelayError> {
    let fts_query = plain_text_fts_query(&request.query)?;
    let filters = FilterBind::new(request, 2);
    let sql = format!(
        "
            SELECT entry.id, entry.display_id, entry.type, entry.title, entry.status,
                   entry.source_path, entry_fts.section, entry_fts.chunk_text,
                   entry.body, CAST(entry_fts.chunk_ordinal AS INTEGER),
                   bm25(entry_fts)
            FROM entry_fts
            JOIN entries entry ON entry.id = entry_fts.entry_id
            WHERE entry_fts MATCH ?1
              AND {}
            ORDER BY bm25(entry_fts), entry.status, entry.display_id,
                     CAST(entry_fts.chunk_ordinal AS INTEGER)
            ",
        filters.predicate
    );
    let mut values = vec![Value::Text(fts_query)];
    values.extend(filters.values);
    let mut statement = connection
        .prepare(&sql)
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let raw = statement
        .query_map(params_from_iter(values), |row| {
            let score = row.get::<_, f64>(10)?;
            raw_from_row(row, Some(score))
        })
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;

    let terms = query_terms(&request.query);
    let mut aggregated = HashMap::<i64, SearchResult>::new();
    for raw in raw {
        if let Some(existing) = aggregated.get_mut(&raw.internal_id) {
            existing.match_count += 1;
            continue;
        }
        let reason = keyword_reason(&raw, &terms);
        let result = finalize_result(connection, database_path, raw, &reason)?;
        aggregated.insert(result.internal_id, result);
    }
    let mut results = aggregated.into_values().collect::<Vec<_>>();
    results.sort_by(|left, right| {
        left.score
            .unwrap_or_default()
            .total_cmp(&right.score.unwrap_or_default())
            .then_with(|| left.status.to_string().cmp(&right.status.to_string()))
            .then_with(|| left.display_id.cmp(&right.display_id))
            .then_with(|| left.chunk_ordinal.cmp(&right.chunk_ordinal))
    });
    results.truncate(request.limit);
    Ok(results)
}

fn raw_from_row(row: &rusqlite::Row<'_>, score: Option<f64>) -> rusqlite::Result<RawResult> {
    Ok(RawResult {
        internal_id: row.get(0)?,
        display_id: row.get(1)?,
        entry_type: row.get(2)?,
        title: row.get(3)?,
        status: row.get(4)?,
        source_path: row.get(5)?,
        section: row.get(6)?,
        excerpt: row.get(7)?,
        body: row.get(8)?,
        chunk_ordinal: row.get(9)?,
        score,
    })
}

fn finalize_result(
    connection: &Connection,
    database_path: &std::path::Path,
    raw: RawResult,
    reason: &str,
) -> Result<SearchResult, BelayError> {
    let entry_type = raw.entry_type.parse::<EntryType>()?;
    let status = raw.status.parse::<EntryStatus>()?;
    let chunk_ordinal = u32::try_from(raw.chunk_ordinal).map_err(|_| BelayError::Validation {
        message: format!("entry {} has an invalid chunk ordinal", raw.display_id),
    })?;
    let mut statement = connection
        .prepare("SELECT tag FROM entry_tags WHERE entry_id = ?1 ORDER BY tag")
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let tags = statement
        .query_map([raw.internal_id], |row| row.get(0))
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    let excerpt = if raw.excerpt.trim().is_empty() {
        raw.body
    } else {
        raw.excerpt
    };
    Ok(SearchResult {
        internal_id: raw.internal_id,
        display_id: raw.display_id,
        entry_type,
        title: raw.title,
        status,
        tags,
        source_path: raw.source_path,
        score: raw.score,
        reason: reason.to_owned(),
        section: raw.section,
        excerpt: compact_excerpt(&excerpt),
        match_count: 1,
        chunk_ordinal,
    })
}

fn plain_text_fts_query(query: &str) -> Result<String, BelayError> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Err(BelayError::Validation {
            message: "search query must contain at least one word".to_owned(),
        });
    }
    Ok(terms
        .into_iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR "))
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|character: char| {
                !character.is_alphanumeric() && character != '_' && character != '-'
            })
        })
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn keyword_reason(raw: &RawResult, terms: &[String]) -> String {
    let title = raw.title.to_lowercase();
    let excerpt = raw.excerpt.to_lowercase();
    if terms.iter().any(|term| excerpt.contains(term)) {
        format!("keyword match in section {}", raw.section)
    } else if terms.iter().any(|term| title.contains(term)) {
        "keyword match in title".to_owned()
    } else {
        "keyword match in body".to_owned()
    }
}

fn compact_excerpt(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= EXCERPT_CHARS {
        return compact;
    }
    let mut excerpt = compact.chars().take(EXCERPT_CHARS - 3).collect::<String>();
    excerpt.push_str("...");
    excerpt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_queries_are_escaped_as_fts_terms() {
        assert_eq!(
            plain_text_fts_query("sqlite migration").expect("query"),
            "\"sqlite\" OR \"migration\""
        );
        assert_eq!(
            plain_text_fts_query("sync: direct-edit").expect("query"),
            "\"sync\" OR \"direct-edit\""
        );
    }

    #[test]
    fn excerpt_is_compact_and_bounded() {
        let excerpt = compact_excerpt(&"word ".repeat(100));
        assert!(excerpt.chars().count() <= EXCERPT_CHARS);
        assert!(!excerpt.contains('\n'));
    }

    #[test]
    fn overlapping_status_include_and_exclude_are_rejected() {
        let error = validate_request(&SearchRequest {
            query: "trace".to_owned(),
            entry_type: None,
            status_include: vec![EntryStatus::Completed],
            status_exclude: vec![EntryStatus::Completed],
            tag: None,
            display_id: None,
            limit: 20,
        })
        .expect_err("overlap");
        assert!(error.to_string().contains("must not overlap"));
    }

    #[test]
    fn status_include_and_exclude_filter_structured_results() {
        let temporary = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(temporary.path().join(".git")).expect("git marker");
        let repository = crate::repository::initialize(temporary.path())
            .expect("init")
            .repository;
        let proposed = crate::store::create(
            &repository,
            EntryType::Decision,
            "Keep proposed".to_owned(),
            "proposed decision body".to_owned(),
        )
        .expect("create proposed");
        let accepted = crate::store::create(
            &repository,
            EntryType::Decision,
            "Promote accepted".to_owned(),
            "accepted decision body".to_owned(),
        )
        .expect("create accepted");
        crate::store::set_status(&repository, &accepted.display_id, EntryStatus::Accepted)
            .expect("accept");

        let included = search(
            &repository,
            &SearchRequest {
                query: String::new(),
                entry_type: Some(EntryType::Decision),
                status_include: vec![EntryStatus::Accepted],
                status_exclude: Vec::new(),
                tag: None,
                display_id: None,
                limit: 20,
            },
        )
        .expect("include");
        assert_eq!(
            included
                .iter()
                .map(|item| item.display_id.as_str())
                .collect::<Vec<_>>(),
            vec![accepted.display_id.as_str()]
        );

        let excluded = search(
            &repository,
            &SearchRequest {
                query: String::new(),
                entry_type: Some(EntryType::Decision),
                status_include: Vec::new(),
                status_exclude: vec![EntryStatus::Proposed],
                tag: None,
                display_id: None,
                limit: 20,
            },
        )
        .expect("exclude");
        assert_eq!(
            excluded
                .iter()
                .map(|item| item.display_id.as_str())
                .collect::<Vec<_>>(),
            vec![accepted.display_id.as_str()]
        );
        assert!(
            !excluded
                .iter()
                .any(|item| item.display_id == proposed.display_id)
        );
    }
}
