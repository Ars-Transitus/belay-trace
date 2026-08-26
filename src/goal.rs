use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::context::{self, ContextFormat};
use crate::entry::{Entry, EntryStatus, EntryType, LinkRelation};
use crate::error::BelayError;
use crate::repository::Repository;
use crate::store;

pub const REQUIRED_SECTIONS: [&str; 6] = [
    "Summary",
    "Success Criteria",
    "Constraints",
    "Non-goals",
    "Verification",
    "Risks",
];

const BUILTIN_AMBIGUOUS_TERMS: &[&str] = &[
    "適切に",
    "いい感じ",
    "なるべく",
    "高速に",
    "必要に応じて",
    "など",
    "appropriate",
    "fast",
    "simple",
    "etc",
    "as needed",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalLintFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalLintReport {
    pub goal_id: String,
    pub checklist_passed: usize,
    pub checklist_total: usize,
    pub findings: Vec<GoalLintFinding>,
}

impl GoalLintReport {
    pub fn has_strict_findings(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.layer != "lexicon")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalLintFinding {
    pub layer: &'static str,
    pub field: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

pub fn template() -> String {
    REQUIRED_SECTIONS
        .iter()
        .map(|section| {
            if *section == "Success Criteria" {
                format!("## {section}\n\n- [SC-001] TODO\n")
            } else {
                format!("## {section}\n\n- TODO\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

pub fn lint(
    repository: &Repository,
    id: Option<&str>,
    all: bool,
) -> Result<Vec<GoalLintReport>, BelayError> {
    let goals = if all {
        load_goals(repository)?
    } else {
        let id = id.ok_or_else(|| BelayError::Validation {
            message: "`belay goal lint` requires a goal ID or --all".to_owned(),
        })?;
        vec![load_goal(repository, id)?]
    };
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    goals
        .iter()
        .map(|goal| lint_entry(repository, &connection, &database_path, goal))
        .collect()
}

pub fn render_lint(
    reports: &[GoalLintReport],
    format: GoalLintFormat,
    score: bool,
) -> Result<String, BelayError> {
    match format {
        GoalLintFormat::Json => serde_json::to_string_pretty(reports)
            .map(|json| format!("{json}\n"))
            .map_err(|source| BelayError::Validation {
                message: format!("could not serialize goal lint report: {source}"),
            }),
        GoalLintFormat::Human => {
            let mut output = String::new();
            for report in reports {
                output.push_str(&format!("{}\n\n", report.goal_id));
                output.push_str(&format!(
                    "Checklist: {}/{} passed\n",
                    report.checklist_passed, report.checklist_total
                ));
                if score {
                    let value = report
                        .checklist_passed
                        .checked_mul(100)
                        .and_then(|value| value.checked_div(report.checklist_total))
                        .unwrap_or(100);
                    output.push_str(&format!("Goal Score: {value}/100\n"));
                }
                if report.findings.is_empty() {
                    output.push_str("\nNo deterministic findings.\n");
                } else {
                    output.push_str("\nMissing:\n");
                    for finding in &report.findings {
                        let line = finding
                            .line
                            .map(|line| format!(" (line {line})"))
                            .unwrap_or_default();
                        output.push_str(&format!(
                            "- [{}] {}: {}{}\n",
                            finding.layer, finding.field, finding.message, line
                        ));
                    }
                }
                output.push_str(
                    "\nRubric for semantic review: run `belay goal improve` and pass the output to your agent.\n\n",
                );
            }
            Ok(output)
        }
    }
}

pub fn improve(repository: &Repository, id: &str, budget: usize) -> Result<String, BelayError> {
    let goal = load_goal(repository, id)?;
    let reports = lint(repository, Some(id), false)?;
    let lint_text = render_lint(&reports, GoalLintFormat::Human, false)?;
    let context = context::generate(repository, &goal.title, ContextFormat::Agent, budget, false)?;
    Ok(format!(
        "# Goal Improvement Bundle\n\n\
         Target: {}\n\
         Title: {}\n\
         Status: {}\n\n\
         ## Current Goal\n\n{}\n\n\
         ## Deterministic Lint\n\n{}\n\
         ## Semantic Review Rubric\n\n\
         Evaluate Completeness, Consistency, Testability, Observability, Ambiguity, Business Alignment, Risk Awareness, and Context Fit.\n\n\
         Return an improved Goal draft, the reason for each material change, newly added considerations, and unresolved questions for the human owner.\n\n\
         ## Related Context\n\n{}",
        goal.display_id, goal.title, goal.status, goal.body, lint_text, context.text
    ))
}

pub fn lint_entry(
    repository: &Repository,
    connection: &Connection,
    database_path: &std::path::Path,
    goal: &Entry,
) -> Result<GoalLintReport, BelayError> {
    if goal.entry_type != EntryType::Goal {
        return Err(BelayError::Validation {
            message: format!("entry {} is {}, not goal", goal.display_id, goal.entry_type),
        });
    }
    let sections = section_map(&goal.body);
    let mut findings = Vec::new();
    let mut total = REQUIRED_SECTIONS.len() + 2;

    for section in REQUIRED_SECTIONS {
        match sections.get(&normalize_section(section)) {
            None => findings.push(GoalLintFinding {
                layer: "structure",
                field: section.to_owned(),
                message: "section is missing".to_owned(),
                line: None,
            }),
            Some((line, text)) if empty_section(text) => findings.push(GoalLintFinding {
                layer: "structure",
                field: section.to_owned(),
                message: "section is empty".to_owned(),
                line: Some(*line),
            }),
            Some(_) => {}
        }
    }

    total += 1;
    findings.extend(
        crate::trace_ids::goal_id_findings(&goal.body)
            .into_iter()
            .map(|finding| GoalLintFinding {
                layer: "structure",
                field: finding.field.to_owned(),
                message: finding.message,
                line: Some(finding.line),
            }),
    );

    let allowed = repository
        .config
        .lint
        .allowed_terms
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<BTreeSet<_>>();
    let mut terms = BUILTIN_AMBIGUOUS_TERMS
        .iter()
        .map(|term| (*term).to_owned())
        .chain(repository.config.lint.ambiguous_terms.iter().cloned())
        .filter(|term| !allowed.contains(&term.to_lowercase()))
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    for (line, text) in non_code_lines(&goal.body) {
        let lower = text.to_lowercase();
        for term in &terms {
            if lower.contains(&term.to_lowercase()) {
                findings.push(GoalLintFinding {
                    layer: "lexicon",
                    field: "body".to_owned(),
                    message: format!("ambiguous term {term:?}"),
                    line: Some(line),
                });
            }
        }
    }

    if goal.status == EntryStatus::Active {
        let incoming_fulfills: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM entry_links links
                JOIN entries target ON target.id = links.to_entry_id
                WHERE target.display_id = ?1 AND links.relation = ?2
                ",
                params![goal.display_id, LinkRelation::Fulfills.to_string()],
                |row| row.get(0),
            )
            .map_err(|source| BelayError::sqlite(database_path, source))?;
        if incoming_fulfills == 0 {
            findings.push(GoalLintFinding {
                layer: "graph",
                field: "fulfills".to_owned(),
                message: "no incoming fulfills link while status is active".to_owned(),
                line: None,
            });
        }
    }

    let broken_links = goal
        .links
        .iter()
        .filter_map(|link| {
            let target = crate::entry::parse_entry_reference_id(&link.id).ok()?;
            let exists: Option<i64> = connection
                .query_row(
                    "SELECT id FROM entries WHERE display_id = ?1",
                    [target.display_id],
                    |row| row.get(0),
                )
                .optional()
                .ok()?;
            exists.is_none().then_some(link.id.clone())
        })
        .collect::<Vec<_>>();
    for target in broken_links {
        findings.push(GoalLintFinding {
            layer: "graph",
            field: "links".to_owned(),
            message: format!("link target {target} does not exist"),
            line: None,
        });
    }

    total += terms.len().min(1);
    let failed = findings
        .iter()
        .filter(|finding| finding.layer != "lexicon")
        .count()
        + usize::from(findings.iter().any(|finding| finding.layer == "lexicon"));
    let passed = total.saturating_sub(failed);
    Ok(GoalLintReport {
        goal_id: goal.display_id.clone(),
        checklist_passed: passed,
        checklist_total: total,
        findings,
    })
}

fn load_goal(repository: &Repository, id: &str) -> Result<Entry, BelayError> {
    let shown = store::show(repository, id)?;
    if shown.entry.entry_type != EntryType::Goal {
        return Err(BelayError::Validation {
            message: format!("entry {id} is {}, not goal", shown.entry.entry_type),
        });
    }
    Ok(shown.entry)
}

fn load_goals(repository: &Repository) -> Result<Vec<Entry>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare("SELECT id FROM entries WHERE type = 'goal' ORDER BY display_id")
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    ids.into_iter()
        .map(|id| store::load_entry(&connection, &database_path, id))
        .collect()
}

pub fn missing_required_sections(entry: &Entry) -> Vec<&'static str> {
    if entry.entry_type != EntryType::Goal {
        return Vec::new();
    }
    let sections = section_map(&entry.body);
    REQUIRED_SECTIONS
        .iter()
        .copied()
        .filter(|section| {
            sections
                .get(&normalize_section(section))
                .is_none_or(|(_, text)| empty_section(text))
        })
        .collect()
}

fn section_map(body: &str) -> std::collections::BTreeMap<String, (usize, String)> {
    let mut result = std::collections::BTreeMap::<String, (usize, String)>::new();
    let mut current = None::<(String, usize, String)>;
    for (index, line) in body.lines().enumerate() {
        if let Some(title) = line.strip_prefix("## ") {
            if let Some((title, line, text)) = current.take() {
                result.insert(normalize_section(&title), (line, text));
            }
            current = Some((title.trim().to_owned(), index + 1, String::new()));
        } else if let Some((_, _, text)) = &mut current {
            text.push_str(line);
            text.push('\n');
        }
    }
    if let Some((title, line, text)) = current {
        result.insert(normalize_section(&title), (line, text));
    }
    result
}

fn normalize_section(section: &str) -> String {
    section.trim().to_ascii_lowercase()
}

fn empty_section(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.is_empty() || trimmed == "- TODO" || trimmed == "TODO"
}

fn non_code_lines(body: &str) -> Vec<(usize, &str)> {
    let mut in_code = false;
    let mut lines = Vec::new();
    for (index, line) in body.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            lines.push((index + 1, line));
        }
    }
    lines
}
