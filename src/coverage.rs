use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::Digest;

use crate::entry::{EntryStatus, EntryType, LinkRelation};
use crate::error::BelayError;
use crate::evidence::{self, Freshness};
use crate::repository::Repository;
use crate::store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    Human,
    Json,
}

#[derive(Debug, Serialize)]
pub struct CoverageReport {
    pub active_goals: usize,
    pub dimensions: Vec<CoverageDimension>,
    pub uncovered: Vec<UncoveredItem>,
}

#[derive(Debug, Serialize)]
pub struct CoverageDimension {
    pub name: &'static str,
    pub traceability: CoverageRatio,
    pub verified: CoverageRatio,
}

#[derive(Debug, Serialize)]
pub struct CoverageRatio {
    pub covered: usize,
    pub total: usize,
    pub percent: usize,
}

#[derive(Debug, Serialize)]
pub struct UncoveredItem {
    pub goal_id: String,
    pub dimension: &'static str,
    pub reason: String,
    pub next_action: String,
}

#[derive(Debug, Clone)]
struct GoalCoverage {
    goal_id: String,
    decision_trace: bool,
    decision_verified: bool,
    implementation_trace: bool,
    implementation_verified: bool,
    test_trace: bool,
    test_verified: bool,
    monitoring_trace: bool,
    monitoring_verified: bool,
    notes: BTreeMap<&'static str, String>,
}

pub fn report(
    repository: &Repository,
    target: Option<&str>,
    include_completed: bool,
) -> Result<CoverageReport, BelayError> {
    let resolved_target = match target {
        Some(target) => {
            let resolved = crate::store::resolve_reference(repository, target)?;
            if resolved.fragment.is_some() {
                return Err(BelayError::Validation {
                    message: "coverage target must be a Goal entry, not a fragment".to_owned(),
                });
            }
            Some(resolved.display_id)
        }
        None => None,
    };
    let target = resolved_target.as_deref();
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let goals = load_goals(&connection, &database_path, target, include_completed)?;
    let head = evidence::current_head(repository).ok();
    let mut rows = Vec::new();
    for (internal_id, goal_id) in goals {
        rows.push(coverage_for_goal(
            repository,
            &connection,
            &database_path,
            head.as_deref(),
            internal_id,
            goal_id,
        )?);
    }
    Ok(render_report(rows))
}

pub fn render(report: &CoverageReport, format: CoverageFormat) -> Result<String, BelayError> {
    match format {
        CoverageFormat::Json => serde_json::to_string_pretty(report)
            .map(|json| format!("{json}\n"))
            .map_err(|source| BelayError::Validation {
                message: format!("could not serialize coverage report: {source}"),
            }),
        CoverageFormat::Human => {
            let mut output = format!("Active goals: {}\n\n", report.active_goals);
            output.push_str("               traceability   verified\n");
            for dimension in &report.dimensions {
                output.push_str(&format!(
                    "{:<18} {:>3}/{:<3} {:>7} {:>3}/{:<3} {:>7}\n",
                    dimension.name,
                    dimension.traceability.covered,
                    dimension.traceability.total,
                    format!("{}%", dimension.traceability.percent),
                    dimension.verified.covered,
                    dimension.verified.total,
                    format!("{}%", dimension.verified.percent)
                ));
            }
            output.push_str("\nUncovered (verified):\n");
            if report.uncovered.is_empty() {
                output.push_str("  none\n");
            } else {
                for item in &report.uncovered {
                    output.push_str(&format!(
                        "  {} {}: {}. Next: {}\n",
                        item.goal_id, item.dimension, item.reason, item.next_action
                    ));
                }
            }
            Ok(output)
        }
    }
}

pub fn fails_under(report: &CoverageReport, threshold: Option<(String, usize)>) -> bool {
    let Some((kind, minimum)) = threshold else {
        return false;
    };
    report.dimensions.iter().any(|dimension| {
        let percent = match kind.as_str() {
            "traceability" => dimension.traceability.percent,
            "verified" => dimension.verified.percent,
            _ => return false,
        };
        percent < minimum
    })
}

pub fn success_criteria_items(
    repository: &Repository,
    goal_id: &str,
) -> Result<Vec<String>, BelayError> {
    let shown = store::show(repository, goal_id)?;
    Ok(extract_success_criteria(&shown.entry.body)
        .into_iter()
        .map(|item| format!("{goal_id}#{}", item_id(&item)))
        .collect())
}

fn load_goals(
    connection: &Connection,
    database_path: &std::path::Path,
    target: Option<&str>,
    include_completed: bool,
) -> Result<Vec<(i64, String)>, BelayError> {
    let status = EntryStatus::Active.to_string();
    let completed = EntryStatus::Completed.to_string();
    let mut statement = connection
        .prepare(
            "
            SELECT id, display_id
            FROM entries
            WHERE type = 'goal'
              AND (?1 IS NULL OR display_id = ?1)
              AND (status = ?2 OR (?3 AND status = ?4))
            ORDER BY display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map(
            params![target, status, include_completed, completed],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn coverage_for_goal(
    repository: &Repository,
    connection: &Connection,
    database_path: &std::path::Path,
    head: Option<&str>,
    goal_internal_id: i64,
    goal_id: String,
) -> Result<GoalCoverage, BelayError> {
    let decision_sources = fulfilling_sources(
        connection,
        database_path,
        goal_internal_id,
        EntryType::Decision,
    )?;
    let work_sources =
        fulfilling_sources(connection, database_path, goal_internal_id, EntryType::Work)?;
    let decision_trace = !decision_sources.is_empty();
    let implementation_trace = !work_sources.is_empty();
    let decision_verified = decision_sources.iter().any(|display_id| {
        has_fresh_pass(
            repository,
            connection,
            database_path,
            head,
            display_id,
            None,
        )
    });
    let implementation_verified = work_sources.iter().any(|display_id| {
        has_fresh_pass(
            repository,
            connection,
            database_path,
            head,
            display_id,
            Some(&["test", "ci-run"]),
        )
    });
    let test_trace = has_evidence_kind(
        connection,
        database_path,
        &goal_id,
        Some(&["test", "ci-run"]),
    )?;
    let test_verified = has_fresh_pass(
        repository,
        connection,
        database_path,
        head,
        &goal_id,
        Some(&["test", "ci-run"]),
    );
    let monitoring_trace =
        has_evidence_kind(connection, database_path, &goal_id, Some(&["metric"]))?;
    let monitoring_verified = has_fresh_pass(
        repository,
        connection,
        database_path,
        head,
        &goal_id,
        Some(&["metric"]),
    );
    let mut notes = BTreeMap::new();
    if !decision_trace {
        notes.insert("decision", "no fulfills decision entry".to_owned());
    } else if !decision_verified {
        notes.insert(
            "decision",
            "decision evidence missing, stale, or not passing".to_owned(),
        );
    }
    if !implementation_trace {
        notes.insert("implementation", "no fulfills work entry".to_owned());
    } else if !implementation_verified {
        notes.insert(
            "implementation",
            "work evidence missing, stale, or not passing".to_owned(),
        );
    }
    if !test_trace {
        notes.insert("test", "no test or CI evidence".to_owned());
    } else if !test_verified {
        notes.insert("test", "test evidence stale or failing".to_owned());
    }
    if !monitoring_trace {
        notes.insert("monitoring", "no metric evidence".to_owned());
    } else if !monitoring_verified {
        notes.insert("monitoring", "metric evidence stale or failing".to_owned());
    }
    Ok(GoalCoverage {
        goal_id,
        decision_trace,
        decision_verified,
        implementation_trace,
        implementation_verified,
        test_trace,
        test_verified,
        monitoring_trace,
        monitoring_verified,
        notes,
    })
}

fn fulfilling_sources(
    connection: &Connection,
    database_path: &std::path::Path,
    goal_internal_id: i64,
    source_type: EntryType,
) -> Result<Vec<String>, BelayError> {
    let mut statement = connection
        .prepare(
            "
            SELECT source.display_id
            FROM entry_links links
            JOIN entries source ON source.id = links.from_entry_id
            WHERE links.to_entry_id = ?1
              AND links.relation = ?2
              AND source.type = ?3
            ORDER BY source.display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    statement
        .query_map(
            params![
                goal_internal_id,
                LinkRelation::Fulfills.to_string(),
                source_type.to_string()
            ],
            |row| row.get(0),
        )
        .map_err(|source| BelayError::sqlite(database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(database_path, source))
}

fn has_evidence_kind(
    connection: &Connection,
    database_path: &std::path::Path,
    target: &str,
    kinds: Option<&[&str]>,
) -> Result<bool, BelayError> {
    let found: Option<String> = connection
        .query_row(
            "
            SELECT evidence.kind
            FROM evidence_links links
            JOIN evidence ON evidence.id = links.evidence_id
            WHERE links.target = ?1 AND links.relation = 'verifies'
            LIMIT 1
            ",
            [target],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| BelayError::sqlite(database_path, source))?;
    Ok(found.is_some_and(|kind| kinds.is_none_or(|kinds| kinds.contains(&kind.as_str()))))
}

fn has_fresh_pass(
    repository: &Repository,
    connection: &Connection,
    _database_path: &std::path::Path,
    head: Option<&str>,
    target: &str,
    kinds: Option<&[&str]>,
) -> bool {
    let mut statement = match connection.prepare(
        "
        SELECT evidence.kind, evidence.verdict, evidence.commit_sha, evidence.captured_at
        FROM evidence_links links
        JOIN evidence ON evidence.id = links.evidence_id
        WHERE links.target = ?1 AND links.relation = 'verifies'
        ",
    ) {
        Ok(statement) => statement,
        Err(_) => return false,
    };
    let rows = match statement.query_map([target], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return false,
    };
    rows.filter_map(Result::ok)
        .any(|(kind, verdict, commit, captured_at)| {
            verdict == "pass"
                && kinds.is_none_or(|kinds| kinds.contains(&kind.as_str()))
                && matches!(
                    evidence::freshness(repository, head, &commit, &captured_at),
                    Freshness::Fresh
                )
        })
}

fn render_report(rows: Vec<GoalCoverage>) -> CoverageReport {
    let total = rows.len();
    let dimensions = vec![
        dimension(
            "decision",
            total,
            rows.iter().filter(|row| row.decision_trace).count(),
            rows.iter().filter(|row| row.decision_verified).count(),
        ),
        dimension(
            "implementation",
            total,
            rows.iter().filter(|row| row.implementation_trace).count(),
            rows.iter()
                .filter(|row| row.implementation_verified)
                .count(),
        ),
        dimension(
            "test",
            total,
            rows.iter().filter(|row| row.test_trace).count(),
            rows.iter().filter(|row| row.test_verified).count(),
        ),
        dimension(
            "monitoring",
            total,
            rows.iter().filter(|row| row.monitoring_trace).count(),
            rows.iter().filter(|row| row.monitoring_verified).count(),
        ),
    ];
    let mut uncovered = Vec::new();
    for row in rows {
        for dimension in ["decision", "implementation", "test", "monitoring"] {
            let verified = match dimension {
                "decision" => row.decision_verified,
                "implementation" => row.implementation_verified,
                "test" => row.test_verified,
                "monitoring" => row.monitoring_verified,
                _ => true,
            };
            if !verified {
                let reason = row
                    .notes
                    .get(dimension)
                    .cloned()
                    .unwrap_or_else(|| "not verified".to_owned());
                let next_action = match dimension {
                    "decision" => "link an accepted decision and record passing evidence",
                    "implementation" => "link work and record fresh test or CI evidence",
                    "test" => "record fresh passing test or CI evidence",
                    "monitoring" => "record fresh metric evidence",
                    _ => "add evidence",
                }
                .to_owned();
                uncovered.push(UncoveredItem {
                    goal_id: row.goal_id.clone(),
                    dimension,
                    reason,
                    next_action,
                });
            }
        }
    }
    CoverageReport {
        active_goals: total,
        dimensions,
        uncovered,
    }
}

fn dimension(name: &'static str, total: usize, trace: usize, verified: usize) -> CoverageDimension {
    CoverageDimension {
        name,
        traceability: ratio(trace, total),
        verified: ratio(verified, total),
    }
}

fn ratio(covered: usize, total: usize) -> CoverageRatio {
    CoverageRatio {
        covered,
        total,
        percent: covered
            .checked_mul(100)
            .and_then(|value| value.checked_div(total))
            .unwrap_or(100),
    }
}

fn extract_success_criteria(body: &str) -> Vec<String> {
    let mut in_section = false;
    let mut items = Vec::new();
    for line in body.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            in_section = title.trim().eq_ignore_ascii_case("Success Criteria");
            continue;
        }
        if in_section {
            if line.starts_with("## ") {
                break;
            }
            let trimmed = line.trim();
            if let Some(item) = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
                .or_else(|| trimmed.strip_prefix("+ "))
            {
                items.push(item.trim().to_owned());
            }
        }
    }
    items
}

fn item_id(text: &str) -> String {
    let digest = sha2::Sha256::digest(text.split_whitespace().collect::<Vec<_>>().join(" "));
    format!("sc-{:x}", digest)[..11].to_owned()
}
