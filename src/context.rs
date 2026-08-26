use std::collections::{BTreeMap, BTreeSet};

use rusqlite::params;

use crate::entry::EntryType;
use crate::error::BelayError;
use crate::markdown::estimate_tokens;
use crate::repository::Repository;
use crate::search::{self, SearchRequest, SearchResult};

const MIN_CONTEXT_BUDGET: usize = 64;
const MINIMUM_EVIDENCE_BUDGET: usize = 40;
const PRIMARY_RESULT_LIMIT: usize = 12;
const LINKED_RESULT_LIMIT: usize = 20;
const TASK_ECHO_BUDGET: usize = 64;
const TARGET_ENTRY_TOKENS: usize = 150;
const MIN_ADMITTED: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFormat {
    Human,
    Agent,
}

#[derive(Debug)]
pub struct ContextBundle {
    pub text: String,
    pub estimated_tokens: usize,
    pub included_entries: usize,
}

#[derive(Debug)]
struct Candidate {
    result: SearchResult,
    evidence: Vec<EvidenceUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceUnit {
    section: String,
    text: String,
}

struct SelectionContext<'a> {
    format: ContextFormat,
    header: &'a str,
    budget: usize,
    candidates: &'a [Candidate],
    terms: &'a [String],
}

pub fn generate(
    repository: &Repository,
    task: &str,
    format: ContextFormat,
    budget: usize,
    include_archived: bool,
) -> Result<ContextBundle, BelayError> {
    if task.trim().is_empty() {
        return Err(BelayError::Validation {
            message: "context task must not be empty".to_owned(),
        });
    }
    if budget < MIN_CONTEXT_BUDGET {
        return Err(BelayError::Validation {
            message: format!("context budget must be at least {MIN_CONTEXT_BUDGET} tokens"),
        });
    }

    let primary = search::search(
        repository,
        &SearchRequest {
            query: task.to_owned(),
            entry_type: None,
            status_include: Vec::new(),
            status_exclude: archived_exclude(include_archived),
            tag: None,
            display_id: None,
            limit: PRIMARY_RESULT_LIMIT,
        },
    )?;
    let linked = search::linked_results(repository, &primary, LINKED_RESULT_LIMIT)?;
    let linked = filter_archived(linked, include_archived);
    let terms = query_terms(task);
    let mut candidates = load_candidates(repository, &terms, primary, true)?;
    candidates.extend(load_candidates(repository, &terms, linked, false)?);

    let selection_budget = budget.saturating_mul(9) / 10;
    let task_budget = TASK_ECHO_BUDGET
        .min(selection_budget.saturating_div(3))
        .max(1);
    let task = truncate_at_boundary(task.trim(), task_budget);
    let header = render_header(format, &task, budget, selection_budget);
    if estimate_tokens(&header) > selection_budget {
        return Err(BelayError::Validation {
            message: "context budget is too small for required output metadata".to_owned(),
        });
    }

    let selection_context = SelectionContext {
        format,
        header: &header,
        budget: selection_budget,
        candidates: &candidates,
        terms: &terms,
    };
    let mut selected = Vec::<(usize, Vec<EvidenceUnit>)>::new();
    let available_for_entries = selection_budget.saturating_sub(estimate_tokens(&header));
    let admission_cap = if candidates.is_empty() {
        0
    } else {
        let minimum = MIN_ADMITTED.min(candidates.len());
        (available_for_entries / TARGET_ENTRY_TOKENS).clamp(minimum, candidates.len())
    };
    for (candidate_index, candidate) in candidates.iter().take(admission_cap).enumerate() {
        let Some(first) = candidate.evidence.first() else {
            continue;
        };
        if let Some(fitted) =
            fit_minimum_evidence(&selection_context, &selected, candidate_index, first)
        {
            selected.push((candidate_index, vec![fitted]));
        }
    }

    distribute_remaining_budget(
        format,
        &header,
        selection_budget,
        &candidates,
        &mut selected,
        &terms,
    );

    let mut output = render_selection(format, &header, &candidates, &selected, admission_cap);
    if selected.is_empty() {
        let no_results = match format {
            ContextFormat::Agent => {
                "\nNo relevant entries fit the selection budget.\nSuggested next read: belay search \"<keywords>\"\n"
            }
            ContextFormat::Human => {
                "\nNo relevant entries fit the selection budget.\nFollow-up: belay search \"<keywords>\"\n"
            }
        };
        if estimate_tokens(&(output.clone() + no_results)) <= selection_budget {
            output.push_str(no_results);
        }
    }

    if estimate_tokens(&output) > selection_budget {
        output = truncate_at_boundary(&output, selection_budget);
    }
    let estimated_tokens = estimate_tokens(&output);
    debug_assert!(estimated_tokens <= selection_budget);
    Ok(ContextBundle {
        text: output,
        estimated_tokens,
        included_entries: selected.len(),
    })
}

pub fn compile(
    repository: &Repository,
    task: &str,
    format: ContextFormat,
    budget: usize,
    seeds: &[String],
    include_archived: bool,
) -> Result<ContextBundle, BelayError> {
    if task.trim().is_empty() {
        return compile_working_set(repository, format, budget, include_archived);
    }
    let mut seeds = seeds.to_vec();
    if crate::entry::looks_like_entry_id_query(task) {
        if let Ok(resolved) = crate::store::resolve_reference(repository, task) {
            seeds.push(resolved.display_id.clone());
        }
    }
    let base_budget = budget.saturating_mul(6) / 10;
    let base = generate(
        repository,
        task,
        format,
        base_budget.max(MIN_CONTEXT_BUDGET),
        include_archived,
    )?;
    let goals = compile_goals(repository, task, &seeds, include_archived)?;
    let failures = compile_failures(repository)?;
    let mut output = match format {
        ContextFormat::Agent => format!(
            "# Context: {}\n(compiled by belay, budget={})\n\n",
            task.trim(),
            budget
        ),
        ContextFormat::Human => format!("# Context: {}\n\nBudget: {}\n\n", task.trim(), budget),
    };
    output.push_str("## Goals\n");
    if goals.is_empty() {
        output.push_str("No directly related goals found.\n\n");
    } else {
        for goal in &goals {
            output.push_str(&format!(
                "- {} [{}]: {}\n",
                goal.display_id, goal.status, goal.title
            ));
            for section in [
                "Success Criteria",
                "Constraints",
                "Non-goals",
                "Verification",
            ] {
                if let Some(text) = section_text(&goal.body, section) {
                    let text = truncate_at_boundary(&text, 180);
                    if !text.is_empty() {
                        output.push_str(&format!("  {section}: {}\n", text.replace('\n', " ")));
                    }
                }
            }
            if let Ok(records) = crate::evidence::latest_for_target(repository, &goal.display_id) {
                for record in records.into_iter().take(3) {
                    output.push_str(&format!(
                        "  Evidence: {} {} {} {}\n",
                        record.verdict,
                        record.kind,
                        record.source,
                        record.freshness.label()
                    ));
                }
            }
        }
        output.push('\n');
    }
    output.push_str("## Past failures\n");
    if failures.is_empty() {
        output.push_str("None found.\n\n");
    } else {
        for failure in failures {
            output.push_str(&format!(
                "- {} [{}]: {}\n",
                failure.display_id, failure.status, failure.title
            ));
        }
        output.push('\n');
    }
    output.push_str("## Ranked context\n\n");
    output.push_str(&base.text);
    output.push_str("\n## Sources\n");
    for goal in &goals {
        output.push_str(&format!("- {}\n", goal.display_id));
    }
    output.push_str(&format!("- {} ranked entries\n", base.included_entries));
    if estimate_tokens(&output) > budget {
        output = truncate_at_boundary(&output, budget);
    }
    Ok(ContextBundle {
        estimated_tokens: estimate_tokens(&output),
        included_entries: base.included_entries + goals.len(),
        text: output,
    })
}

#[derive(Debug)]
struct CompileEntry {
    display_id: String,
    title: String,
    status: crate::entry::EntryStatus,
    body: String,
}

fn compile_goals(
    repository: &Repository,
    task: &str,
    seeds: &[String],
    include_archived: bool,
) -> Result<Vec<CompileEntry>, BelayError> {
    let mut results = Vec::new();
    for seed in seeds {
        let shown = crate::store::show(repository, seed)?;
        if shown.entry.entry_type == EntryType::Goal {
            results.push(CompileEntry {
                display_id: shown.entry.display_id,
                title: shown.entry.title,
                status: shown.entry.status,
                body: shown.entry.body,
            });
        }
    }
    let search_results = search::search(
        repository,
        &SearchRequest {
            query: task.to_owned(),
            entry_type: Some(EntryType::Goal),
            status_include: Vec::new(),
            status_exclude: archived_exclude(include_archived),
            tag: None,
            display_id: None,
            limit: 5,
        },
    )
    .unwrap_or_default();
    for result in search_results {
        if results
            .iter()
            .any(|entry| entry.display_id == result.display_id)
        {
            continue;
        }
        let shown = crate::store::show(repository, &result.display_id)?;
        results.push(CompileEntry {
            display_id: shown.entry.display_id,
            title: shown.entry.title,
            status: shown.entry.status,
            body: shown.entry.body,
        });
    }
    Ok(results)
}

fn compile_failures(repository: &Repository) -> Result<Vec<CompileEntry>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT id
            FROM entries
            WHERE (type = 'work' AND status = 'abandoned')
               OR (type = 'decision' AND status = 'rejected')
            ORDER BY updated_at DESC, display_id
            LIMIT 5
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    ids.into_iter()
        .map(|id| {
            let entry = crate::store::load_entry(&connection, &database_path, id)?;
            Ok(CompileEntry {
                display_id: entry.display_id,
                title: entry.title,
                status: entry.status,
                body: entry.body,
            })
        })
        .collect()
}

fn section_text(body: &str, wanted: &str) -> Option<String> {
    heading_block(body, "## ", wanted)
}

fn subsection_text(body: &str, wanted: &str) -> Option<String> {
    heading_block(body, "### ", wanted)
}

fn heading_block(body: &str, marker: &str, wanted: &str) -> Option<String> {
    let mut in_section = false;
    let mut text = String::new();
    for line in body.lines() {
        if let Some(title) = line.strip_prefix(marker) {
            if in_section {
                break;
            }
            in_section = title.trim().eq_ignore_ascii_case(wanted);
            continue;
        }
        if marker == "### " && line.starts_with("## ") && in_section {
            break;
        }
        if in_section {
            text.push_str(line);
            text.push('\n');
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn archived_exclude(include_archived: bool) -> Vec<crate::entry::EntryStatus> {
    let mut exclude = Vec::new();
    crate::search::apply_default_archived_exclude(&[], &mut exclude, include_archived);
    exclude
}

fn filter_archived(results: Vec<SearchResult>, include_archived: bool) -> Vec<SearchResult> {
    if include_archived {
        return results;
    }
    results
        .into_iter()
        .filter(|result| result.status != crate::entry::EntryStatus::Archived)
        .collect()
}

pub fn compile_working_set(
    repository: &Repository,
    format: ContextFormat,
    budget: usize,
    include_archived: bool,
) -> Result<ContextBundle, BelayError> {
    let live = live_entries(repository, include_archived)?;
    let next = next_actions(repository, &live)?;
    let seeds = live
        .iter()
        .filter(|entry| entry.entry_type == EntryType::Goal)
        .map(|entry| entry.display_id.clone())
        .collect::<Vec<_>>();
    let goals = compile_goals(repository, "working set", &seeds, include_archived)?;
    let failures = compile_failures(repository)?;
    let primary_ids = live
        .iter()
        .map(|entry| entry.display_id.clone())
        .collect::<Vec<_>>();
    let ranked_task = live
        .iter()
        .map(|entry| entry.title.as_str())
        .find(|title| !title.is_empty())
        .unwrap_or("working set");
    let base_budget = budget.saturating_mul(5) / 10;
    let base = if live.is_empty() {
        None
    } else {
        Some(generate(
            repository,
            ranked_task,
            format,
            base_budget.max(MIN_CONTEXT_BUDGET),
            include_archived,
        )?)
    };

    let mut output = match format {
        ContextFormat::Agent => format!("# Working set\n(compiled by belay, budget={budget})\n\n"),
        ContextFormat::Human => format!("# Working set\n\nBudget: {budget}\n\n"),
    };
    output.push_str("## Next\n");
    if next.is_empty() {
        output.push_str("No in-progress or not-started Delivery Map tasks in live plans.\n\n");
    } else {
        for item in &next {
            output.push_str(&format!(
                "- {} [{}] {} — belay show {}\n",
                item.reference, item.state, item.outcome, item.reference
            ));
        }
        output.push('\n');
    }
    output.push_str("## Live entries\n");
    if live.is_empty() {
        output.push_str("No live entries in the working set.\n\n");
    } else {
        for entry in &live {
            output.push_str(&format!(
                "- {} [{}] {}: {}\n",
                entry.display_id, entry.status, entry.entry_type, entry.title
            ));
        }
        output.push('\n');
    }
    output.push_str("## Goals\n");
    if goals.is_empty() {
        output.push_str("No live goals found.\n\n");
    } else {
        for goal in &goals {
            output.push_str(&format!(
                "- {} [{}]: {}\n",
                goal.display_id, goal.status, goal.title
            ));
            for section in ["Success Criteria", "Constraints", "Non-goals"] {
                if let Some(text) = section_text(&goal.body, section) {
                    let text = truncate_at_boundary(&text, 180);
                    if !text.is_empty() {
                        output.push_str(&format!("  {section}: {}\n", text.replace('\n', " ")));
                    }
                }
            }
        }
        output.push('\n');
    }
    output.push_str("## Past failures\n");
    if failures.is_empty() {
        output.push_str("None found.\n\n");
    } else {
        for failure in failures {
            output.push_str(&format!(
                "- {} [{}]: {}\n",
                failure.display_id, failure.status, failure.title
            ));
        }
        output.push('\n');
    }
    if let Some(base) = base {
        output.push_str("## Ranked context\n\n");
        output.push_str(&base.text);
        output.push_str("\n## Sources\n");
        for id in &primary_ids {
            output.push_str(&format!("- {id}\n"));
        }
    }
    if estimate_tokens(&output) > budget {
        output = truncate_at_boundary(&output, budget);
    }
    Ok(ContextBundle {
        estimated_tokens: estimate_tokens(&output),
        included_entries: live.len() + goals.len(),
        text: output,
    })
}

struct LiveEntry {
    display_id: String,
    entry_type: EntryType,
    status: crate::entry::EntryStatus,
    title: String,
    body: String,
}

struct NextAction {
    reference: String,
    state: String,
    outcome: String,
}

fn live_entries(
    repository: &Repository,
    include_archived: bool,
) -> Result<Vec<LiveEntry>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT display_id, type, status, title, body
            FROM entries
            ORDER BY updated_at DESC, display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let mut live = Vec::new();
    for row in rows {
        let (display_id, entry_type, status, title, body) =
            row.map_err(|source| BelayError::sqlite(&database_path, source))?;
        let entry_type = entry_type.parse::<EntryType>()?;
        let status = status.parse::<crate::entry::EntryStatus>()?;
        if !include_archived && status == crate::entry::EntryStatus::Archived {
            continue;
        }
        if entry_type.is_live_status(status) {
            live.push(LiveEntry {
                display_id,
                entry_type,
                status,
                title,
                body,
            });
        }
    }
    Ok(live)
}

fn next_actions(
    repository: &Repository,
    live: &[LiveEntry],
) -> Result<Vec<NextAction>, BelayError> {
    let _ = repository;
    let mut next = Vec::new();
    for entry in live
        .iter()
        .filter(|entry| entry.entry_type == EntryType::Plan)
    {
        let rows = crate::trace_ids::delivery_map_rows(&entry.body);
        let mut in_progress = Vec::new();
        let mut first_not_started = None;
        for row in rows {
            let state = row.cell("State").unwrap_or("").trim().to_ascii_lowercase();
            let outcome = row
                .cell("Outcome / Task")
                .or_else(|| row.cell("Outcome"))
                .or_else(|| row.cell("Task"))
                .unwrap_or("")
                .trim()
                .to_owned();
            let reference = format!("{}#{}", entry.display_id, row.id.to_ascii_lowercase());
            if state == "in-progress" {
                in_progress.push(NextAction {
                    reference,
                    state,
                    outcome,
                });
            } else if state == "not-started" && first_not_started.is_none() {
                first_not_started = Some(NextAction {
                    reference,
                    state,
                    outcome,
                });
            }
        }
        if in_progress.is_empty() {
            if let Some(item) = first_not_started {
                next.push(item);
            }
        } else {
            next.extend(in_progress);
        }
    }
    Ok(next)
}

pub fn compile_focus(
    repository: &Repository,
    focus: &str,
    format: ContextFormat,
    budget: usize,
) -> Result<ContextBundle, BelayError> {
    let shown = crate::store::show(repository, focus)?;
    let Some(fragment) = shown.fragment.as_ref() else {
        return Err(BelayError::Validation {
            message: "--focus requires a canonical fragment such as PLN-...#t-001".to_owned(),
        });
    };
    let canonical = format!("{}#{}", shown.entry.display_id, fragment.fragment);
    let constraints = subsection_text(&shown.entry.body, "Constraints")
        .or_else(|| section_text(&shown.entry.body, "Constraints"));
    let non_goals = subsection_text(&shown.entry.body, "Non-goals")
        .or_else(|| section_text(&shown.entry.body, "Non-goals"));
    let assumptions = subsection_text(&shown.entry.body, "Assumptions")
        .or_else(|| subsection_text(&shown.entry.body, "Assumptions / Unknowns"));

    let mut output = match format {
        ContextFormat::Agent => {
            format!("# Task packet: {canonical}\n(compiled by belay, budget={budget})\n\n")
        }
        ContextFormat::Human => format!("# Task packet: {canonical}\n\nBudget: {budget}\n\n"),
    };
    output.push_str("## Intent Brief\n");
    for (label, text) in [
        ("Constraints", constraints),
        ("Non-goals", non_goals),
        ("Assumptions", assumptions),
    ] {
        match text {
            Some(text) => output.push_str(&format!("### {label}\n{text}\n\n")),
            None => output.push_str(&format!("### {label}\nNone identified\n\n")),
        }
    }
    output.push_str("## Task\n");
    output.push_str(&format!("Definition:\n{}\n\n", fragment.definition));
    match &fragment.section {
        Some(section) => output.push_str(&format!("Section:\n{section}\n\n")),
        None => output.push_str("Section: none\n\n"),
    }

    output.push_str("## Goal item\n");
    match goal_item_for_task(&shown.entry.body, &fragment.fragment) {
        Some(goal_item) => {
            output.push_str(&format!("{goal_item}\n"));
            if let Some(packet) = load_goal_item_packet(repository, &shown.entry, &goal_item)? {
                output.push_str(&packet);
            }
            output.push('\n');
        }
        None => output.push_str("None identified\n\n"),
    }

    output.push_str("## Evidence\n");
    let records = crate::evidence::latest_for_target(repository, &canonical).unwrap_or_default();
    if records.is_empty() {
        output.push_str("None found.\n");
    } else {
        for record in records {
            output.push_str(&format!(
                "- {} {} {} {} {}\n",
                record.display_id,
                record.verdict,
                record.kind,
                record.source,
                record.freshness.label()
            ));
        }
    }

    if estimate_tokens(&output) > budget {
        output = truncate_at_boundary(&output, budget);
    }
    Ok(ContextBundle {
        estimated_tokens: estimate_tokens(&output),
        included_entries: 1,
        text: output,
    })
}

fn goal_item_for_task(body: &str, fragment: &str) -> Option<String> {
    crate::trace_ids::delivery_map_rows(body)
        .into_iter()
        .find(|row| row.id.eq_ignore_ascii_case(fragment))
        .and_then(|row| {
            row.cell("Goal item")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn load_goal_item_packet(
    repository: &Repository,
    plan: &crate::entry::Entry,
    goal_item: &str,
) -> Result<Option<String>, BelayError> {
    let reference = if goal_item.contains('#') || crate::entry::looks_like_entry_id_query(goal_item)
    {
        goal_item.to_owned()
    } else {
        let Some(goal_id) = plan.links.iter().find_map(|link| {
            (link.relation == crate::entry::LinkRelation::Fulfills
                || link.relation == crate::entry::LinkRelation::Implements)
                .then(|| {
                    crate::entry::parse_entry_reference_id(&link.id)
                        .ok()
                        .filter(|reference| {
                            crate::store::show(repository, &reference.display_id)
                                .ok()
                                .is_some_and(|shown| shown.entry.entry_type == EntryType::Goal)
                        })
                        .map(|reference| reference.display_id)
                })
                .flatten()
        }) else {
            return Ok(None);
        };
        format!(
            "{goal_id}#{fragment}",
            fragment = goal_item.to_ascii_lowercase()
        )
    };
    let shown = crate::store::show(repository, &reference)?;
    let mut text = format!("Resolved: {}\n", shown.entry.display_id);
    if let Some(fragment) = shown.fragment {
        text.push_str(&format!("Definition:\n{}\n", fragment.definition));
        if let Some(section) = fragment.section {
            text.push_str(&format!("Section:\n{section}\n"));
        }
    } else {
        text.push_str(&shown.entry.body);
        text.push('\n');
    }
    Ok(Some(text))
}

fn load_candidates(
    repository: &Repository,
    terms: &[String],
    results: Vec<SearchResult>,
    primary: bool,
) -> Result<Vec<Candidate>, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open(&database_path)?;
    let mut candidates = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT section, text FROM entry_chunks
             WHERE entry_id = ?1 ORDER BY ordinal",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;

    for result in results {
        let chunks = statement
            .query_map(params![result.internal_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| BelayError::sqlite(&database_path, source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| BelayError::sqlite(&database_path, source))?;

        let mut evidence = chunks
            .into_iter()
            .flat_map(|(section, text)| evidence_units(&section, &text))
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        evidence.retain(|unit| seen.insert((unit.section.clone(), unit.text.clone())));
        evidence.sort_by_key(|unit| evidence_priority(&result, unit, terms, primary));
        candidates.push(Candidate { result, evidence });
    }
    Ok(candidates)
}

fn evidence_priority(
    result: &SearchResult,
    unit: &EvidenceUnit,
    terms: &[String],
    primary: bool,
) -> (u8, u8, String) {
    let section = unit.section.to_lowercase();
    let text = unit.text.to_lowercase();
    let text_match = terms.iter().any(|term| text.contains(term));
    let section_match = terms.iter().any(|term| section.contains(term));
    let matched_section = unit.section == result.section;
    let important = important_sections(result.entry_type)
        .iter()
        .position(|preferred| section == preferred.to_lowercase());
    let class = if text_match && primary && matched_section {
        0
    } else if text_match {
        1
    } else if section_match && primary && matched_section {
        2
    } else if (!primary && important.is_some()) || (primary && matched_section) {
        3
    } else {
        4
    };
    (
        class,
        important.unwrap_or(usize::MAX).min(255) as u8,
        String::new(),
    )
}

fn important_sections(entry_type: EntryType) -> &'static [&'static str] {
    match entry_type {
        EntryType::Goal => &[
            "Summary",
            "Success Criteria",
            "Constraints",
            "Non-goals",
            "Verification",
            "Risks",
        ],
        EntryType::Decision => &["Decision", "Rationale", "Risks"],
        EntryType::Plan => &["Summary", "Objectives", "Success Criteria"],
        EntryType::Work => &["Changes", "Validation", "Blockers"],
        EntryType::Review => &["Findings", "Risks", "Recommendations"],
        EntryType::Note => &["Summary", "Context", "Notes"],
    }
}

fn evidence_units(section: &str, text: &str) -> Vec<EvidenceUnit> {
    let mut units = Vec::new();
    for paragraph in text
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let mut prose = Vec::new();
        for line in paragraph
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if is_list_item(line) {
                units.push(EvidenceUnit {
                    section: section.to_owned(),
                    text: line.to_owned(),
                });
            } else {
                prose.push(line);
            }
        }
        if !prose.is_empty() {
            units.extend(
                split_sentences(&prose.join(" "))
                    .into_iter()
                    .map(|sentence| EvidenceUnit {
                        section: section.to_owned(),
                        text: sentence,
                    }),
            );
        }
    }
    units
}

fn is_list_item(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return true;
    }
    let Some((number, remainder)) = line.split_once(". ") else {
        return false;
    };
    !number.is_empty()
        && number.chars().all(|character| character.is_ascii_digit())
        && !remainder.is_empty()
}

fn split_sentences(text: &str) -> Vec<String> {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut sentences = Vec::new();
    let mut start = 0;
    for (index, character) in compact.char_indices() {
        if matches!(character, '.' | '!' | '?' | '。' | '！' | '？') {
            let end = index + character.len_utf8();
            let sentence = compact[start..end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_owned());
            }
            start = end;
        }
    }
    let remainder = compact[start..].trim();
    if !remainder.is_empty() {
        sentences.push(remainder.to_owned());
    }
    sentences
}

fn distribute_remaining_budget(
    format: ContextFormat,
    header: &str,
    selection_budget: usize,
    candidates: &[Candidate],
    selected: &mut [(usize, Vec<EvidenceUnit>)],
    terms: &[String],
) {
    if selected.is_empty() {
        return;
    }
    let base_tokens = rendered_tokens(format, header, candidates, selected);
    let remaining = selection_budget.saturating_sub(base_tokens);
    let total_weight = (1..=selected.len())
        .map(|rank| 1.0 / rank as f64)
        .sum::<f64>();
    let mut carry = 0;

    for rank in 0..selected.len() {
        let share =
            ((remaining as f64 * (1.0 / (rank + 1) as f64)) / total_weight) as usize + carry;
        let candidate_index = selected[rank].0;
        let current_first_tokens = estimate_tokens(&selected[rank].1[0].text);
        let expanded_first = truncate_evidence(
            &candidates[candidate_index].evidence[0].text,
            current_first_tokens + share,
            terms,
        );
        selected[rank].1[0].text = expanded_first;
        let mut used =
            estimate_tokens(&selected[rank].1[0].text).saturating_sub(current_first_tokens);
        for unit in candidates[candidate_index].evidence.iter().skip(1) {
            let unit_tokens = estimate_tokens(&unit.text);
            if used + unit_tokens > share {
                let truncated = truncate_evidence(&unit.text, share.saturating_sub(used), terms);
                if !truncated.is_empty() {
                    selected[rank].1.push(EvidenceUnit {
                        section: unit.section.clone(),
                        text: truncated,
                    });
                }
                break;
            }
            selected[rank].1.push(unit.clone());
            used += unit_tokens;
        }
        while rendered_tokens(format, header, candidates, selected) > selection_budget {
            if selected[rank].1.len() == 1 {
                let current = selected[rank].1[0].text.clone();
                let current_tokens = estimate_tokens(&current);
                if current_tokens <= 1 {
                    break;
                }
                selected[rank].1[0].text =
                    truncate_evidence(&current, current_tokens.saturating_sub(1), terms);
                continue;
            }
            selected[rank].1.pop();
        }
        used = estimate_tokens(&selected[rank].1[0].text).saturating_sub(current_first_tokens)
            + selected[rank]
                .1
                .iter()
                .skip(1)
                .map(|unit| estimate_tokens(&unit.text))
                .sum::<usize>();
        carry = share.saturating_sub(used);
    }
}

fn fit_minimum_evidence(
    context: &SelectionContext<'_>,
    selected: &[(usize, Vec<EvidenceUnit>)],
    candidate_index: usize,
    evidence: &EvidenceUnit,
) -> Option<EvidenceUnit> {
    let capped_text = truncate_evidence(
        &evidence.text,
        estimate_tokens(&evidence.text).min(MINIMUM_EVIDENCE_BUDGET),
        context.terms,
    );
    if capped_text.is_empty() {
        return None;
    }
    let capped = EvidenceUnit {
        section: evidence.section.clone(),
        text: capped_text,
    };
    let mut trial = selected.to_vec();
    trial.push((candidate_index, vec![capped.clone()]));
    if rendered_tokens(context.format, context.header, context.candidates, &trial) <= context.budget
    {
        return Some(capped);
    }

    let mut low = 1;
    let mut high = estimate_tokens(&capped.text);
    let mut best = None;
    while low <= high {
        let middle = low + (high - low) / 2;
        let text = truncate_evidence(&evidence.text, middle, context.terms);
        if text.is_empty() {
            low = middle + 1;
            continue;
        }
        trial.last_mut().expect("candidate was added").1[0].text = text.clone();
        if rendered_tokens(context.format, context.header, context.candidates, &trial)
            <= context.budget
        {
            best = Some(EvidenceUnit {
                section: evidence.section.clone(),
                text,
            });
            low = middle + 1;
        } else {
            high = middle.saturating_sub(1);
        }
    }
    best
}

fn truncate_evidence(text: &str, budget: usize, terms: &[String]) -> String {
    if estimate_tokens(text) <= budget {
        return text.to_owned();
    }
    if budget == 0 {
        return String::new();
    }

    let words = text.split_whitespace().collect::<Vec<_>>();
    let marker = list_marker(&words);
    let matched = words.iter().position(|word| {
        let normalized = word.to_lowercase();
        terms.iter().any(|term| normalized.contains(term))
    });
    let Some(matched) = matched else {
        return truncate_at_boundary(text, budget);
    };

    let mut start = matched;
    let mut end = matched + 1;
    let mut best = String::new();
    loop {
        let excerpt = words[start..end].join(" ");
        let truncated = match (start > marker.len(), end < words.len()) {
            (true, true) => format!("... {excerpt} ..."),
            (true, false) => format!("... {excerpt}"),
            (false, true) => format!("{excerpt} ..."),
            (false, false) => excerpt,
        };
        let candidate = if start > 0 && !marker.is_empty() {
            format!("{} {truncated}", marker.join(" "))
        } else {
            truncated
        };
        if estimate_tokens(&candidate) > budget {
            break;
        }
        best = candidate;
        if start == 0 && end == words.len() {
            break;
        }
        start = start.saturating_sub(1);
        if end < words.len() {
            end += 1;
        }
    }
    best
}

fn list_marker<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let Some(first) = words.first() else {
        return &[];
    };
    if matches!(*first, "-" | "*" | "+")
        && words
            .get(1)
            .is_some_and(|marker| matches!(*marker, "[ ]" | "[x]" | "[X]"))
    {
        return &words[..2];
    }
    if matches!(*first, "-" | "*" | "+") && words.get(1) == Some(&"[") && words.get(2) == Some(&"]")
    {
        return &words[..3];
    }
    if matches!(*first, "-" | "*" | "+")
        || first
            .strip_suffix('.')
            .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
    {
        &words[..1]
    } else {
        &[]
    }
}

fn render_header(
    format: ContextFormat,
    task: &str,
    budget: usize,
    selection_budget: usize,
) -> String {
    match format {
        ContextFormat::Agent => format!(
            "Context bundle\nTask: {task}\nFormat: agent\nBudget: {budget} estimated tokens\n"
        ),
        ContextFormat::Human => format!(
            "Context for: {task}\nFormat: human\nBudget: {budget} estimated tokens\nSelection limit: {selection_budget} estimated tokens\n"
        ),
    }
}

fn rendered_tokens(
    format: ContextFormat,
    header: &str,
    candidates: &[Candidate],
    selected: &[(usize, Vec<EvidenceUnit>)],
) -> usize {
    estimate_tokens(&render_selection(
        format,
        header,
        candidates,
        selected,
        candidates.len(),
    ))
}

fn render_selection(
    format: ContextFormat,
    header: &str,
    candidates: &[Candidate],
    selected: &[(usize, Vec<EvidenceUnit>)],
    admission_cap: usize,
) -> String {
    let mut output = header.to_owned();
    let selected_by_type = selected.iter().fold(
        BTreeMap::<EntryType, Vec<(usize, &Vec<EvidenceUnit>)>>::new(),
        |mut groups, (index, evidence)| {
            groups
                .entry(candidates[*index].result.entry_type)
                .or_default()
                .push((*index, evidence));
            groups
        },
    );
    for entry_type in EntryType::ALL {
        let Some(group) = selected_by_type.get(&entry_type) else {
            continue;
        };
        output.push_str(&group_heading(format, entry_type));
        for (index, evidence) in group {
            output.push_str(&render_result(format, &candidates[*index].result, evidence));
        }
    }
    if format == ContextFormat::Agent && !selected.is_empty() {
        let show_ids = selected
            .iter()
            .map(|(index, _)| format!("belay show {}", candidates[*index].result.display_id))
            .collect::<Vec<_>>()
            .join("; ");
        output.push_str(&format!("\nRead more: {show_ids}\n"));
        let related = candidates
            .iter()
            .skip(admission_cap)
            .map(|candidate| candidate.result.display_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if !related.is_empty() {
            output.push_str(&format!("Also related: {related}\n"));
        }
    }
    output
}

fn render_result(
    format: ContextFormat,
    result: &SearchResult,
    evidence: &[EvidenceUnit],
) -> String {
    let tags = if result.tags.is_empty() {
        String::new()
    } else {
        format!("  Tags: {}\n", result.tags.join(", "))
    };
    let excerpt = render_evidence(evidence);
    match format {
        ContextFormat::Agent => format!(
            "- {}: {} [{}]\n  Why: {}\n  Source: {}\n{}  Evidence: {}\n",
            result.display_id,
            result.title,
            result.status,
            result.reason,
            result.source_path,
            tags,
            excerpt
        ),
        ContextFormat::Human => format!(
            "{} - {}\nStatus: {}\nRelevance: {}\nSource: {}\nTags: {}\nEvidence: {}\nFollow-up: belay show {}\n\n",
            result.display_id,
            result.title,
            result.status,
            result.reason,
            result.source_path,
            tags,
            excerpt,
            result.display_id
        ),
    }
}

fn render_evidence(evidence: &[EvidenceUnit]) -> String {
    let mut sections = BTreeSet::new();
    evidence
        .iter()
        .map(|unit| {
            if sections.insert(unit.section.as_str()) {
                format!("{}: {}", unit.section, unit.text)
            } else {
                unit.text.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn group_heading(format: ContextFormat, entry_type: EntryType) -> String {
    let label = match entry_type {
        EntryType::Goal => "Goals",
        EntryType::Plan => "Plans",
        EntryType::Decision => "Decisions",
        EntryType::Work => "Work",
        EntryType::Review => "Reviews",
        EntryType::Note => "Notes",
    };
    match format {
        ContextFormat::Agent => format!("\nRelevant {}:\n", label.to_lowercase()),
        ContextFormat::Human => format!("\n## {label}\n"),
    }
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

fn truncate_at_boundary(text: &str, budget: usize) -> String {
    if estimate_tokens(text) <= budget {
        return text.to_owned();
    }
    if budget == 0 {
        return String::new();
    }
    let mut best = String::new();
    for (index, character) in text.char_indices() {
        let end = index + character.len_utf8();
        if character.is_whitespace()
            || matches!(character, '.' | '!' | '?' | '。' | '！' | '？' | ',' | '、')
        {
            let candidate = format!("{}...", text[..end].trim_end());
            if estimate_tokens(&candidate) <= budget {
                best = candidate;
            } else {
                break;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_units_preserve_lists_and_sentence_boundaries() {
        assert_eq!(
            evidence_units("Findings", "- first finding\n- second finding"),
            vec![
                EvidenceUnit {
                    section: "Findings".to_owned(),
                    text: "- first finding".to_owned(),
                },
                EvidenceUnit {
                    section: "Findings".to_owned(),
                    text: "- second finding".to_owned(),
                },
            ]
        );
        assert_eq!(
            split_sentences("First sentence. 日本語です。 Last one!"),
            vec!["First sentence.", "日本語です。", "Last one!"]
        );
        assert_eq!(
            evidence_units("Steps", "Lead in.\n1. first step\n2. second step"),
            vec![
                EvidenceUnit {
                    section: "Steps".to_owned(),
                    text: "1. first step".to_owned(),
                },
                EvidenceUnit {
                    section: "Steps".to_owned(),
                    text: "2. second step".to_owned(),
                },
                EvidenceUnit {
                    section: "Steps".to_owned(),
                    text: "Lead in.".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn truncation_uses_boundaries_and_obeys_the_estimator() {
        for budget in 0..30 {
            let value = truncate_at_boundary("ASCII words and 日本語。 more context", budget);
            assert!(estimate_tokens(&value) <= budget);
            assert!(
                value.is_empty()
                    || value == "ASCII words and 日本語。 more context"
                    || value.ends_with("...")
            );
        }
    }

    #[test]
    fn evidence_truncation_keeps_ordered_list_marker_and_query() {
        let value = truncate_evidence(
            "1. background background background quartz final action",
            10,
            &["quartz".to_owned()],
        );
        assert!(value.starts_with("1."));
        assert!(value.contains("quartz"));
        assert!(estimate_tokens(&value) <= 10);
    }

    #[test]
    fn evidence_truncation_keeps_task_list_state() {
        let value = truncate_evidence(
            "- [x] background background background quartz final action",
            11,
            &["quartz".to_owned()],
        );
        assert!(value.starts_with("- [x]"));
        assert!(value.contains("quartz"));
        assert!(estimate_tokens(&value) <= 11);

        let unchecked = truncate_evidence(
            "- [ ] background background background quartz final action",
            11,
            &["quartz".to_owned()],
        );
        assert!(unchecked.starts_with("- [ ]"));
        assert!(unchecked.contains("quartz"));
        assert!(estimate_tokens(&unchecked) <= 11);
    }
}
