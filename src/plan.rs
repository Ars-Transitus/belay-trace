use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::entry::{Entry, EntryType};
use crate::error::BelayError;
use crate::repository::Repository;
use crate::store;
use crate::trace_ids;

/// The baseline a task section must carry for a reader with no prior context to
/// act on it. It is deliberately generic: a consumer that needs more — a
/// difficulty, an owner, a budget — adds fields, and unknown fields are not
/// findings. Encoding one consumer's workflow here would make belay's linter
/// something every other user has to fight.
pub const REQUIRED_TASK_FIELDS: [&str; 5] =
    ["Objective", "Scope", "Steps", "Acceptance", "Verification"];

pub const TASK_STATES: [&str; 6] = [
    "not-started",
    "in-progress",
    "blocked",
    "implemented",
    "verified",
    "dropped",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanLintFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanLintReport {
    pub plan_id: String,
    /// False when the Plan has no Delivery Map. Structural checks are then
    /// skipped rather than failed, so an entry written before the Delivery Map
    /// convention does not have to be rewritten to stay lintable.
    pub delivery_map: bool,
    pub checklist_passed: usize,
    pub checklist_total: usize,
    pub findings: Vec<PlanLintFinding>,
}

impl PlanLintReport {
    pub fn has_strict_findings(&self) -> bool {
        !self.findings.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanLintFinding {
    pub layer: &'static str,
    pub field: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

pub fn lint(
    repository: &Repository,
    id: Option<&str>,
    all: bool,
) -> Result<Vec<PlanLintReport>, BelayError> {
    let plans = if all {
        load_plans(repository)?
    } else {
        let id = id.ok_or_else(|| BelayError::Validation {
            message: "`belay plan lint` requires a plan ID or --all".to_owned(),
        })?;
        vec![load_plan(repository, id)?]
    };
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    plans
        .iter()
        .map(|plan| lint_entry(&connection, plan))
        .collect()
}

pub fn lint_entry(
    connection: &rusqlite::Connection,
    plan: &Entry,
) -> Result<PlanLintReport, BelayError> {
    if plan.entry_type != EntryType::Plan {
        return Err(BelayError::Validation {
            message: format!("entry {} is {}, not plan", plan.display_id, plan.entry_type),
        });
    }

    if !trace_ids::has_delivery_map(&plan.body) {
        return Ok(PlanLintReport {
            plan_id: plan.display_id.clone(),
            delivery_map: false,
            checklist_passed: 0,
            checklist_total: 0,
            findings: Vec::new(),
        });
    }

    let mut findings = Vec::new();
    let rows = trace_ids::delivery_map_rows(&plan.body);

    // 1. The map defines tasks at all.
    let mut total: usize = 1;
    if rows.is_empty() {
        findings.push(PlanLintFinding {
            layer: "structure",
            field: "Delivery Map".to_owned(),
            message: "Delivery Map has no task rows".to_owned(),
            line: None,
        });
    }

    // 2. The Goal item column exists, so a task can be traced to intent.
    total += 1;
    if !rows.is_empty() && rows.iter().all(|row| row.cell("goal item").is_none()) {
        findings.push(PlanLintFinding {
            layer: "structure",
            field: "Delivery Map".to_owned(),
            message: "Delivery Map has no `Goal item` column".to_owned(),
            line: rows.first().map(|row| row.line),
        });
    }

    // 3. Task IDs are canonical and unique. Reuse the same check the fragment
    //    standard uses, so an ID that lints here also resolves as `#t-nnn`.
    total += 1;
    findings.extend(
        trace_ids::plan_id_findings(&plan.body)
            .into_iter()
            .map(|finding| PlanLintFinding {
                layer: "structure",
                field: finding.field.to_owned(),
                message: finding.message,
                line: Some(finding.line),
            }),
    );

    // 4. States come from the fixed set.
    total += 1;
    for row in &rows {
        let Some(state) = row.cell("state") else {
            continue;
        };
        if state.is_empty() || TASK_STATES.contains(&state.to_ascii_lowercase().as_str()) {
            continue;
        }
        findings.push(PlanLintFinding {
            layer: "structure",
            field: format!("Delivery Map {}", row.id),
            message: format!("state {state:?} is not one of {}", TASK_STATES.join(", ")),
            line: Some(row.line),
        });
    }

    // 5. Every task has a body section, and 6. that section carries the
    //    baseline fields. A row naming a task no part of the document explains
    //    is the gap this linter exists to surface.
    total += 2;
    for row in &rows {
        if !trace_ids::valid_task_id(&row.id) {
            continue;
        }
        let Some(section) = task_section(&plan.body, &row.id) else {
            findings.push(PlanLintFinding {
                layer: "structure",
                field: format!("Delivery Map {}", row.id),
                message: format!("no `## {}` section explains this task", row.id),
                line: Some(row.line),
            });
            continue;
        };
        for field in REQUIRED_TASK_FIELDS {
            if !section_has_field(&section.1, field) {
                findings.push(PlanLintFinding {
                    layer: "structure",
                    field: format!("{} {field}", row.id),
                    message: "task section is missing this field".to_owned(),
                    line: Some(section.0),
                });
            }
        }
    }

    // 7. Links point at entries that exist.
    total += 1;
    for link in &plan.links {
        let Ok(target) = crate::entry::parse_entry_reference_id(&link.id) else {
            continue;
        };
        let exists: Option<i64> = connection
            .query_row(
                "SELECT id FROM entries WHERE display_id = ?1",
                [target.display_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);
        if exists.is_none() {
            findings.push(PlanLintFinding {
                layer: "graph",
                field: "links".to_owned(),
                message: format!("link target {} does not exist", link.id),
                line: None,
            });
        }
    }

    let failed = distinct_failed_checks(&findings);
    Ok(PlanLintReport {
        plan_id: plan.display_id.clone(),
        delivery_map: true,
        checklist_passed: total.saturating_sub(failed),
        checklist_total: total,
        findings,
    })
}

pub fn render_lint(
    reports: &[PlanLintReport],
    format: PlanLintFormat,
) -> Result<String, BelayError> {
    match format {
        PlanLintFormat::Json => serde_json::to_string_pretty(reports)
            .map(|json| format!("{json}\n"))
            .map_err(|source| BelayError::Validation {
                message: format!("could not serialize plan lint report: {source}"),
            }),
        PlanLintFormat::Human => {
            let mut output = String::new();
            for report in reports {
                output.push_str(&format!("{}\n\n", report.plan_id));
                if !report.delivery_map {
                    output.push_str("Delivery Map: none; structural checks skipped\n\n");
                    continue;
                }
                output.push_str(&format!(
                    "Checklist: {}/{} passed\n",
                    report.checklist_passed, report.checklist_total
                ));
                if report.findings.is_empty() {
                    output.push_str("\nNo deterministic findings.\n\n");
                } else {
                    output.push_str("\nFindings:\n");
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
                    output.push('\n');
                }
            }
            Ok(output)
        }
    }
}

/// One check may produce many findings — six tasks can each miss a section —
/// so the checklist counts failed checks, not failed items.
fn distinct_failed_checks(findings: &[PlanLintFinding]) -> usize {
    let mut kinds = std::collections::BTreeSet::new();
    for finding in findings {
        let kind = if finding.layer == "graph" {
            "links"
        } else if finding.message.contains("no `## ") {
            "sections"
        } else if finding.message.contains("missing this field") {
            "fields"
        } else if finding.message.contains("is not one of") {
            "states"
        } else if finding.message.contains("Goal item") {
            "goal-item"
        } else if finding.message.contains("no task rows") {
            "rows"
        } else {
            "ids"
        };
        kinds.insert(kind);
    }
    kinds.len()
}

/// The `## T-NNN` section body and its heading line, matched the way a fragment
/// is matched so `plan lint` and `belay show <id>#t-nnn` agree on what a task
/// section is.
fn task_section(body: &str, id: &str) -> Option<(usize, String)> {
    let mut current = None::<(usize, String)>;
    for (index, line) in body.lines().enumerate() {
        if let Some(heading) = line.strip_prefix("## ") {
            if current.is_some() {
                break;
            }
            if heading.trim().eq_ignore_ascii_case(id) {
                current = Some((index + 1, String::new()));
            }
            continue;
        }
        if let Some((_, text)) = &mut current {
            text.push_str(line);
            text.push('\n');
        }
    }
    current
}

/// A field is present when the section has a `**Field**` marker or a
/// `Field:` label, so the check does not dictate one Markdown style.
fn section_has_field(section: &str, field: &str) -> bool {
    let lower = section.to_ascii_lowercase();
    let field = field.to_ascii_lowercase();
    lower.contains(&format!("**{field}**")) || lower.contains(&format!("{field}:"))
}

fn load_plan(repository: &Repository, id: &str) -> Result<Entry, BelayError> {
    let shown = store::show(repository, id)?;
    if shown.entry.entry_type != EntryType::Plan {
        return Err(BelayError::Validation {
            message: format!("entry {id} is {}, not plan", shown.entry.entry_type),
        });
    }
    Ok(shown.entry)
}

fn load_plans(repository: &Repository) -> Result<Vec<Entry>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare("SELECT id FROM entries WHERE type = 'plan' ORDER BY display_id")
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
