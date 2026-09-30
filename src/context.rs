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
    if seeds.is_empty() && crate::entry::looks_like_entry_id_query(task) && task.contains('#') {
        return compile_focus(repository, task, format, budget);
    }
    let pool = compile_entries(repository)?;
    let primary = search::search(
        repository,
        &SearchRequest {
            query: task.into(),
            entry_type: None,
            status_include: vec![],
            status_exclude: archived_exclude(include_archived),
            tag: None,
            display_id: None,
            limit: PRIMARY_RESULT_LIMIT,
        },
    )?;
    let linked = search::linked_results(repository, &primary, LINKED_RESULT_LIMIT)?;
    let query_scopes = primary
        .iter()
        .filter(|r| matches!(r.entry_type, EntryType::Goal | EntryType::Plan))
        .map(|r| r.display_id.clone())
        .collect::<BTreeSet<_>>();
    let mut scopes = BTreeSet::new();
    let mut explicit_ids = BTreeSet::new();
    let mut selected = BTreeMap::<String, String>::new();
    let mut ranks = BTreeMap::<String, u32>::new();
    for (rank, result) in primary.into_iter().chain(linked).enumerate() {
        ranks
            .entry(result.display_id.clone())
            .or_insert(rank as u32 + 1);
        selected.entry(result.display_id).or_insert(result.reason);
    }
    let mut seed_refs = seeds.to_vec();
    if crate::entry::looks_like_entry_id_query(task) {
        seed_refs.push(task.into());
    }
    for seed in seed_refs {
        let reference = crate::store::resolve_reference(repository, &seed)?;
        explicit_ids.insert(reference.display_id.clone());
        if pool.iter().any(|e| {
            e.display_id == reference.display_id
                && matches!(e.entry_type, EntryType::Goal | EntryType::Plan)
        }) {
            scopes.insert(reference.canonical_id());
            if reference.fragment.is_some()
                && pool.iter().any(|e| {
                    e.display_id == reference.display_id && e.entry_type == EntryType::Plan
                })
            {
                let plan = crate::store::show(repository, &reference.canonical_id())?;
                let goal_item = crate::trace_ids::delivery_map_rows(&plan.entry.body)
                    .into_iter()
                    .find(|r| Some(r.id.to_ascii_lowercase()) == reference.fragment)
                    .and_then(|r| r.cell("Goal item").map(str::trim).map(str::to_owned));
                if let Some(item) = goal_item {
                    if let Some(goal) = resolve_goal_item(repository, &plan.entry, &item)? {
                        // Select the mapped criterion, not its siblings. Whole
                        // Goal policies still apply as ancestors of this scope.
                        crate::store::show(repository, &goal)?;
                        scopes.insert(goal);
                    }
                }
            }
        }
        ranks.insert(reference.display_id.clone(), 0);
        selected.insert(reference.display_id, "explicit seed".into());
    }
    // Explicit Goal/Plan scopes define applicability. Query hits still rank
    // candidates, but cannot broaden a selected Task/criterion to its siblings.
    if scopes.is_empty() {
        scopes = query_scopes;
    }
    // Carry an explicitly selected Plan's Goal; do not duplicate its ranked entry.
    let selected_ids = selected.keys().cloned().collect::<BTreeSet<_>>();
    for entry in pool.iter().filter(|e| selected_ids.contains(&e.display_id)) {
        for (target, relation) in &entry.links {
            if matches!(relation.as_str(), "fulfills" | "implements") {
                let bare = target.split('#').next().unwrap_or(target);
                if pool
                    .iter()
                    .any(|e| e.display_id == bare && e.entry_type == EntryType::Goal)
                {
                    selected.entry(bare.into()).or_insert_with(|| {
                        format!("scope from {} via {relation}", entry.display_id)
                    });
                    if entry.entry_type == EntryType::Plan && scopes.contains(&entry.display_id) {
                        scopes.insert(bare.into());
                    }
                }
            }
        }
    }
    let mut evidence = ContextEvidence::load(repository)?;
    let (summary_details, summary_warning) = match crate::summary::context_details(repository) {
        Ok(details) => (details, None),
        Err(error) => (
            BTreeMap::new(),
            Some(format!(
                "Derived summaries unavailable; using original detail: {error}"
            )),
        ),
    };
    let mut sections = Vec::new();
    let mut exclusions = Vec::new();
    if let Some(warning) = summary_warning {
        exclusions.push(warning);
    }
    for entry in &pool {
        let Some(reason) = selected.get(&entry.display_id) else {
            continue;
        };
        if !include_archived && entry.status == crate::entry::EntryStatus::Archived {
            exclusions.push(format!("{}: archived", entry.display_id));
            continue;
        }
        let outside_scope = entry.entry_type == EntryType::Decision
            && !scopes.is_empty()
            && scope_membership(entry, &pool, &scopes) == Some(false);
        if outside_scope && !explicit_ids.contains(&entry.display_id) {
            exclusions.push(format!(
                "{}: outside selected scope (scope={})",
                entry.display_id,
                entry.scope.as_deref().unwrap_or("Unknown")
            ));
            continue;
        }
        let mut content = format!(
            "{} [{}]: {}\nWhy: {reason}\n",
            entry.display_id, entry.status, entry.title
        );
        let mut refs = vec![entry.display_id.clone()];
        let required = matches!(entry.entry_type, EntryType::Goal | EntryType::Plan);
        if required {
            content.push_str(&boundary_text(
                &entry.body,
                entry.entry_type == EntryType::Plan,
            ));
            if entry.entry_type == EntryType::Goal {
                if let Some(sc) = section_text(&entry.body, "Success Criteria") {
                    content.push_str(&format!("\nSuccess Criteria:\n{sc}\n"));
                }
            }
        } else {
            let terms = query_terms(task);
            if let Some((prose, sources)) = summary_details.get(&entry.display_id) {
                content.push_str(prose);
                refs.extend(sources.iter().cloned());
            } else {
                content.push_str(&optional_entry_detail(entry, &terms));
            }
            let boundaries = boundary_text(&entry.body, false);
            if !boundaries.starts_with("Boundaries: Unknown") {
                sections.push(context_section(
                    &format!("boundary:{}", entry.display_id),
                    "Required boundaries",
                    true,
                    5,
                    vec![entry.display_id.clone()],
                    boundaries,
                ));
            }
        }
        if entry.entry_type == EntryType::Decision {
            let (state, decisive) = decision_state(entry, &pool, &evidence);
            refs.extend(decisive);
            content.push_str(&format!(
                "Applicability: {state}; {}; scope={}; {}; freshness is separate.\n",
                decision_reason(entry, &state, &evidence),
                entry.scope.as_deref().unwrap_or("Unknown"),
                if outside_scope {
                    "outside selected scope; included only by explicit seed; current applicability Unknown"
                } else if scopes.is_empty() || scope_membership(entry, &pool, &scopes).is_none() {
                    "current scope applicability Unknown; human scope meaning is not inferred"
                } else {
                    "within selected scope; adoption does not prove current applicability"
                }
            ));
        }
        let (verification, ids) = evidence.summary(repository, &entry.display_id)?;
        refs.extend(ids);
        content.push_str(&format!(
            "Evidence: {verification}; status is not verification.\n"
        ));
        let title = match entry.entry_type {
            EntryType::Goal => "Goals",
            EntryType::Plan => "Plans",
            EntryType::Decision => "Decision",
            EntryType::Work => "Work",
            EntryType::Review => "Review",
            EntryType::Note => "Note",
        };
        sections.push(context_section(
            &format!("entry:{}", entry.display_id),
            title,
            required,
            10 + ranks.get(&entry.display_id).copied().unwrap_or(100),
            refs,
            content,
        ));
    }
    if sections.is_empty() {
        sections.push(context_section(
            "empty",
            "Goals",
            true,
            10,
            vec![],
            "No related entries found.".into(),
        ));
    }
    let excluded_archived = pool
        .iter()
        .filter(|e| e.status == crate::entry::EntryStatus::Archived && !include_archived)
        .count();
    sections.push(context_section("selection","Selection",true,0,vec![],format!("Related by query/link/seed; canonical IDs deduplicated. Summary context: retrieve focus for complete Task boundaries before execution. Unrelated scope excluded; archived excluded={excluded_archived}.{}",if exclusions.is_empty(){String::new()}else{format!("\n{}",exclusions.join("\n"))})));
    bundle_sections(
        &format!(
            "# Context: {}\n(compiled by belay, budget={budget})",
            task.trim()
        ),
        sections,
        format,
        budget,
    )
}

// LC05 can replace this optional prose with a current source-bound derived
// summary. Required boundaries, evidence/status and canonical sources are built
// separately and cannot be displaced by that replacement.
fn optional_entry_detail(entry: &CompileEntry, terms: &[String]) -> String {
    let mut units = crate::markdown::generate_chunks(&entry.body)
        .into_iter()
        .flat_map(|chunk| evidence_units(&chunk.section, &chunk.text))
        .collect::<Vec<_>>();
    units.sort_by_key(|unit| {
        (
            !terms
                .iter()
                .any(|term| unit.text.to_lowercase().contains(term)),
            unit.section.clone(),
            unit.text.clone(),
        )
    });
    let Some(unit) = units.first() else {
        return String::new();
    };
    let excerpt = truncate_evidence(&unit.text, 120, terms);
    format!(
        "{}: {}{}\n",
        unit.section,
        excerpt,
        if excerpt != unit.text {
            " [excerpt truncated; retrieve source]"
        } else {
            ""
        }
    )
}

#[derive(Debug)]
struct CompileEntry {
    display_id: String,
    title: String,
    status: crate::entry::EntryStatus,
    entry_type: EntryType,
    body: String,
    scope: Option<String>,
    links: Vec<(String, String)>,
}
fn compile_entries(repository: &Repository) -> Result<Vec<CompileEntry>, BelayError> {
    let path = repository.database_path();
    let connection = crate::database::open_read_only(&path)?;
    let mut statement=connection.prepare("SELECT display_id,title,status,type,body,metadata_json,source_path,revision,content_hash FROM entries ORDER BY display_id").map_err(|s|BelayError::sqlite(&path,s))?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, u32>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|s| BelayError::sqlite(&path, s))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|s| BelayError::sqlite(&path, s))?;
    let mut entries = Vec::new();
    for (
        display_id,
        title,
        status,
        entry_type,
        body,
        metadata,
        source_path,
        revision,
        content_hash,
    ) in rows
    {
        validate_original(
            repository,
            &display_id,
            source_path.as_deref(),
            revision,
            &content_hash,
        )?;
        let metadata: serde_json::Value =
            serde_json::from_str(&metadata).map_err(|e| BelayError::Validation {
                message: format!("invalid entry metadata for {display_id}: {e}"),
            })?;
        let scope = metadata
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        entries.push(CompileEntry {
            display_id,
            title,
            status: status.parse()?,
            entry_type: entry_type.parse()?,
            body,
            scope,
            links: vec![],
        });
    }
    let mut statement=connection.prepare("SELECT source.display_id,target.display_id,links.to_fragment,links.relation FROM entry_links links JOIN entries source ON source.id=links.from_entry_id JOIN entries target ON target.id=links.to_entry_id ORDER BY source.display_id,target.display_id,links.to_fragment,links.relation").map_err(|s|BelayError::sqlite(&path,s))?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|s| BelayError::sqlite(&path, s))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|s| BelayError::sqlite(&path, s))?;
    for (source, target, fragment, relation) in rows {
        if let Some(entry) = entries.iter_mut().find(|e| e.display_id == source) {
            entry.links.push((
                if fragment.is_empty() {
                    target
                } else {
                    format!("{target}#{fragment}")
                },
                relation,
            ));
        }
    }
    Ok(entries)
}
fn validate_original(
    repository: &Repository,
    id: &str,
    source_path: Option<&str>,
    revision: u32,
    indexed_hash: &str,
) -> Result<(), BelayError> {
    let source_path = source_path.ok_or_else(|| BelayError::Validation {
        message: format!(
            "source binding missing for {id}; reconcile originals before compiling context"
        ),
    })?;
    let path = crate::store::managed_source_path(repository, source_path)?;
    let contents = crate::store::read_managed_file(repository, &path)?;
    let original = crate::markdown::parse(&contents)?;
    if original.display_id != id
        || original.revision != revision
        || crate::markdown::content_hash(&original)? != indexed_hash
    {
        return Err(BelayError::Validation {
            message: format!(
                "stale indexed original for {id}; revision/content hash changed; run belay sync before compiling context"
            ),
        });
    }
    Ok(())
}
fn validate_indexed_source(repository: &Repository, id: &str) -> Result<(), BelayError> {
    let path = repository.database_path();
    let connection = crate::database::open_read_only(&path)?;
    let (source, revision, hash) = connection
        .query_row(
            "SELECT source_path,revision,content_hash FROM entries WHERE display_id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|s| BelayError::sqlite(&path, s))?;
    validate_original(repository, id, source.as_deref(), revision, &hash)
}

fn section_text(body: &str, wanted: &str) -> Option<String> {
    heading_block(body, "## ", wanted)
}
fn subsection_text(body: &str, wanted: &str) -> Option<String> {
    heading_block(body, "### ", wanted)
}
fn heading_block(body: &str, marker: &str, wanted: &str) -> Option<String> {
    let level = marker.chars().take_while(|c| *c == '#').count();
    let mut active = false;
    let mut text = String::new();
    for line in body.lines() {
        if let Some((depth, title)) = markdown_heading(line) {
            if active && depth <= level {
                break;
            }
            if depth == level && title.eq_ignore_ascii_case(wanted) {
                active = true;
                continue;
            }
        }
        if active {
            text.push_str(line);
            text.push('\n');
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| text.into())
}
// Chunks stop at every heading. A focused Task instead owns its entire nested
// Markdown block, bounded by the next heading at the same or a higher level.
fn document_headings(body: &str) -> Vec<(usize, usize, usize, String)> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let mut headings = Vec::new();
    let mut nesting = 0usize;
    let mut heading = None;
    for (event, range) in Parser::new_ext(body, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) if nesting == 0 => {
                heading = Some((range.start, range.end, level as usize, String::new()));
            }
            Event::End(TagEnd::Heading(_)) if heading.is_some() => {
                headings.push(heading.take().unwrap());
            }
            Event::Start(_) => nesting += 1,
            Event::End(_) => nesting -= 1,
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, _, _, title)) = heading.as_mut() {
                    title.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, _, title)) = heading.as_mut() {
                    title.push(' ');
                }
            }
            _ => {}
        }
    }
    headings
}
fn complete_task_section(body: &str, task: &str) -> Result<Option<String>, BelayError> {
    let headings = document_headings(body);
    let matches = headings
        .iter()
        .enumerate()
        .filter(|(_, (_, _, _, title))| title.trim().eq_ignore_ascii_case(task))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(BelayError::Validation {
            message: format!("ambiguous Task section: duplicate heading for {task}"),
        });
    }
    let Some((index, (_, start, level, _))) = matches.first() else {
        return Ok(None);
    };
    let end = headings
        .iter()
        .skip(index + 1)
        .find(|(_, _, depth, _)| depth <= level)
        .map_or(body.len(), |(start, _, _, _)| *start);
    Ok(Some(body[*start..end].trim().into()))
}

// Only a canonical reference to a validated original has mechanically known
// scope. Human labels are preserved without guessing their meaning.
fn scope_membership(
    entry: &CompileEntry,
    pool: &[CompileEntry],
    scopes: &BTreeSet<String>,
) -> Option<bool> {
    let reference = crate::entry::parse_entry_reference_id(entry.scope.as_deref()?).ok()?;
    let target = pool.iter().find(|e| e.display_id == reference.display_id)?;
    if let Some(fragment) = reference.fragment.as_deref() {
        crate::trace_ids::fragment_definition(target.entry_type, &target.body, fragment)?;
    }
    Some(
        scopes.contains(&reference.display_id)
            || scopes.contains(&reference.canonical_id())
            || (reference.fragment.is_none()
                && scopes.iter().any(|scope| {
                    scope.split_once('#').map(|(id, _)| id) == Some(reference.display_id.as_str())
                })),
    )
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let depth = line.chars().take_while(|c| *c == '#').count();
    if depth == 0 || !line[depth..].starts_with(' ') {
        return None;
    }
    Some((depth, line[depth..].trim()))
}
fn boundary_text(body: &str, plan: bool) -> String {
    let body = if plan {
        let task_start = document_headings(body)
            .into_iter()
            .find(|(_, _, _, title)| {
                crate::trace_ids::valid_reference_fragment(
                    EntryType::Plan,
                    &title.trim().to_ascii_lowercase(),
                )
            })
            .map_or(body.len(), |(start, _, _, _)| start);
        &body[..task_start]
    } else {
        body
    };
    let mut output = String::new();
    let mut active_depth = None;
    for line in body.lines() {
        if let Some((depth, title)) = markdown_heading(line) {
            if plan
                && crate::trace_ids::valid_reference_fragment(
                    EntryType::Plan,
                    &title.to_ascii_lowercase(),
                )
            {
                break;
            }
            let name = title.to_ascii_lowercase().replace('-', " ");
            let boundary = matches!(
                name.as_str(),
                "constraints"
                    | "non goals"
                    | "assumptions"
                    | "assumptions / unknowns"
                    | "unknowns"
                    | "unknowns / decisions needed"
                    | "stop"
                    | "stops"
                    | "stop condition"
                    | "stop conditions"
                    | "stopping conditions"
            );
            if boundary {
                active_depth = Some(depth);
            } else if active_depth.is_some_and(|level| depth <= level) {
                active_depth = None;
            }
        }
        let label = line
            .trim()
            .trim_start_matches(['-', '*', '+'])
            .trim()
            .replace("**", "")
            .to_ascii_lowercase();
        if active_depth.is_some()
            || label.starts_with("stop:")
            || label.starts_with("stop condition:")
            || label.starts_with("stop conditions:")
        {
            output.push_str(line);
            output.push('\n');
        }
    }
    if output.is_empty() {
        "Boundaries: Unknown (not recorded).\n".into()
    } else {
        output
    }
}
fn unknown_text(body: &str) -> Option<String> {
    subsection_text(body, "Unknowns / Decisions Needed")
        .or_else(|| section_text(body, "Unknowns / Decisions Needed"))
        .or_else(|| section_text(body, "Unknowns"))
}
fn archived_exclude(include_archived: bool) -> Vec<crate::entry::EntryStatus> {
    let mut exclude = Vec::new();
    crate::search::apply_default_archived_exclude(&[], &mut exclude, include_archived);
    exclude
}
fn filter_archived(results: Vec<SearchResult>, include_archived: bool) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|r| include_archived || r.status != crate::entry::EntryStatus::Archived)
        .collect()
}
fn context_section(
    id: &str,
    title: &str,
    required: bool,
    order: u32,
    sources: Vec<String>,
    content: String,
) -> crate::context_sections::ContextSection {
    crate::context_sections::ContextSection {
        id: id.into(),
        title: title.into(),
        requirement: if required {
            crate::context_sections::SectionRequirement::Required
        } else {
            crate::context_sections::SectionRequirement::Optional
        },
        order,
        sources,
        content,
    }
}
fn bundle_sections(
    header: &str,
    sections: Vec<crate::context_sections::ContextSection>,
    format: ContextFormat,
    budget: usize,
) -> Result<ContextBundle, BelayError> {
    let rendered = crate::context_sections::render_sections(header, &sections, format, budget)
        .map_err(|e| BelayError::Validation {
            message: e.to_string(),
        })?;
    Ok(ContextBundle {
        included_entries: rendered.source_ids.len(),
        estimated_tokens: rendered.estimated_tokens,
        text: rendered.text,
    })
}

pub fn compile_working_set(
    repository: &Repository,
    format: ContextFormat,
    budget: usize,
    include_archived: bool,
) -> Result<ContextBundle, BelayError> {
    use crate::entry::EntryStatus;
    let pool = compile_entries(repository)?;
    let mut evidence = ContextEvidence::load(repository)?;
    let current = pool
        .iter()
        .filter(|e| {
            matches!(e.entry_type, EntryType::Goal | EntryType::Plan)
                && (matches!(
                    e.status,
                    EntryStatus::Active | EntryStatus::Approved | EntryStatus::Blocked
                ) || (e.entry_type == EntryType::Plan
                    && e.status == EntryStatus::Draft
                    && crate::trace_ids::delivery_map_rows(&e.body)
                        .iter()
                        .any(|r| {
                            r.cell("State").is_some_and(|v| {
                                matches!(v.trim(), "in-progress" | "not-started" | "blocked")
                            })
                        })))
        })
        .collect::<Vec<_>>();
    let mut sections = Vec::new();
    let mut current_text = String::new();
    let mut refs = Vec::new();
    let mut next = String::new();
    let mut next_refs = Vec::new();
    let mut blockers = String::new();
    let mut blocker_refs = Vec::new();
    for entry in &current {
        refs.push(entry.display_id.clone());
        let (verification, evd) = evidence.summary(repository, &entry.display_id)?;
        refs.extend(evd);
        current_text.push_str(&format!(
            "- {} [{}] {}; evidence={}\n",
            entry.display_id, entry.status, entry.entry_type, verification
        ));
        if entry.status == EntryStatus::Blocked {
            blockers.push_str(&format!("- {}: blocked\n", entry.display_id));
            blocker_refs.push(entry.display_id.clone());
        }
        if let Some(unknown) = unknown_text(&entry.body) {
            if !is_empty_marker(&unknown) {
                blockers.push_str(&format!(
                    "- {}: {}\n",
                    entry.display_id,
                    unknown.replace('\n', " ")
                ));
                blocker_refs.push(entry.display_id.clone());
            }
        }
        if entry.entry_type == EntryType::Plan {
            let rows = crate::trace_ids::delivery_map_rows(&entry.body);
            let in_progress = rows.iter().any(|r| {
                r.cell("State")
                    .is_some_and(|s| s.eq_ignore_ascii_case("in-progress"))
            });
            let mut first = false;
            for row in rows {
                let state = row.cell("State").unwrap_or("").trim().to_ascii_lowercase();
                let reference = format!("{}#{}", entry.display_id, row.id.to_ascii_lowercase());
                if state == "blocked" {
                    blockers.push_str(&format!("- {reference}: blocked\n"));
                    blocker_refs.push(reference.clone());
                }
                if state == "in-progress" || (!in_progress && !first && state == "not-started") {
                    first = true;
                    let outcome = row
                        .cell("Outcome / Task")
                        .or_else(|| row.cell("Outcome"))
                        .or_else(|| row.cell("Task"))
                        .unwrap_or("");
                    let excerpt = truncate_at_boundary(outcome, 32);
                    next.push_str(&format!("- {reference} [{state}]: {excerpt}\n"));
                    next_refs.push(reference);
                }
            }
        }
    }
    // Blocked Work records remain visible even without an active Plan.
    for entry in pool
        .iter()
        .filter(|e| e.entry_type == EntryType::Work && e.status == EntryStatus::Blocked)
    {
        blockers.push_str(&format!(
            "- {}: blocked — {}\n",
            entry.display_id, entry.title
        ));
        blocker_refs.push(entry.display_id.clone());
    }
    if current_text.is_empty() {
        current_text = "No active Goal/Plan.\n".into();
    }
    current_text.push_str("Status is not Goal verification; criterion coverage: belay coverage.\n");
    sections.push(context_section(
        "current",
        "Goals / Plans",
        true,
        0,
        refs,
        current_text,
    ));
    if next.is_empty() {
        next = "No in-progress or not-started tasks.\n".into();
    }
    next.push_str("Candidates only; execution authorization is Unknown. Before execution: belay context compile --focus <Task-ID>.\n");
    sections.push(context_section("next", "Next", true, 1, next_refs, next));
    if blockers.is_empty() {
        blockers = "No recorded blockers/unknowns; absence is not proof of readiness.\n".into();
    }
    sections.push(context_section(
        "blockers",
        "Blockers / Unknowns",
        true,
        2,
        blocker_refs,
        blockers,
    ));
    let scopes = current
        .iter()
        .map(|e| e.display_id.clone())
        .collect::<BTreeSet<_>>();
    for entry in pool
        .iter()
        .filter(|e| e.entry_type == EntryType::Decision && e.status == EntryStatus::Accepted)
    {
        let membership = scope_membership(entry, &pool, &scopes);
        let relevant = membership != Some(false)
            && (entry.scope.is_some()
                || entry
                    .links
                    .iter()
                    .any(|(id, _)| scopes.contains(id.split('#').next().unwrap_or(id))));
        if !relevant {
            continue;
        }
        let (state, mut sources) = decision_state(entry, &pool, &evidence);
        sources.push(entry.display_id.clone());
        let (verification, evd) = evidence.summary(repository, &entry.display_id)?;
        sources.extend(evd);
        sections.push(context_section(&format!("decision:{}",entry.display_id),"Decision applicability",false,3,sources,format!("{}: {state}; {}; scope={}; evidence={verification}; {}; freshness does not establish applicability.",entry.display_id,decision_reason(entry,&state,&evidence),entry.scope.as_deref().unwrap_or("Unknown"),if membership.is_none(){"current scope applicability Unknown; human scope meaning is not inferred"}else{"within current scope; adoption does not prove current applicability"})));
    }
    let archived = pool
        .iter()
        .filter(|e| e.status == EntryStatus::Archived)
        .count();
    sections.push(context_section("detail","Details",true,4,vec![],format!("Summary only; task boundaries omitted until focus. Inactive/unrelated history omitted; {} archived entries {}. Retrieve a known original with belay show <ID>; historical acceptance alone is not current applicability.",archived,if include_archived{"available by explicit retrieval"}else{"excluded"})));
    bundle_sections(
        &format!("# Working set\n(compiled by belay, budget={budget})"),
        sections,
        format,
        budget,
    )
}
fn is_empty_marker(text: &str) -> bool {
    let t = text
        .trim()
        .trim_start_matches('-')
        .trim()
        .to_ascii_lowercase();
    matches!(
        t.as_str(),
        "none"
            | "none."
            | "none identified"
            | "none identified."
            | "none found"
            | "none found."
            | "なし"
    )
}

pub fn compile_focus(
    repository: &Repository,
    focus: &str,
    format: ContextFormat,
    budget: usize,
) -> Result<ContextBundle, BelayError> {
    let shown = crate::store::show(repository, focus)?;
    validate_indexed_source(repository, &shown.entry.display_id)?;
    let Some(fragment) = shown.fragment.as_ref() else {
        return Err(BelayError::Validation {
            message: "--focus requires a canonical fragment such as PLN-...#t-001".into(),
        });
    };
    let canonical = format!("{}#{}", shown.entry.display_id, fragment.fragment);
    let task_section = if shown.entry.entry_type == EntryType::Plan {
        complete_task_section(&shown.entry.body, &fragment.fragment)?
    } else {
        fragment.section.clone()
    };
    let mut sections = vec![context_section(
        "plan-boundaries",
        "Intent Brief",
        true,
        0,
        vec![shown.entry.display_id.clone()],
        boundary_text(&shown.entry.body, shown.entry.entry_type == EntryType::Plan),
    )];
    sections.push(context_section(
        "task",
        "Task",
        true,
        1,
        vec![canonical.clone()],
        format!(
            "Definition:\n{}\nSection:\n{}",
            fragment.definition,
            task_section
                .as_deref()
                .unwrap_or("Unknown (task section missing)")
        ),
    ));
    if shown.entry.entry_type == EntryType::Plan {
        let rows = crate::trace_ids::delivery_map_rows(&shown.entry.body)
            .into_iter()
            .filter(|r| r.id.eq_ignore_ascii_case(&fragment.fragment))
            .collect::<Vec<_>>();
        if rows.len() > 1 {
            return Err(BelayError::Validation {
                message: format!(
                    "ambiguous Task mapping: duplicate row for {}",
                    fragment.fragment
                ),
            });
        }
        let goal_item = rows
            .first()
            .and_then(|r| r.cell("Goal item").map(str::trim).map(str::to_owned));
        if let Some(goal_item) = goal_item {
            let reference = resolve_goal_item(repository, &shown.entry, &goal_item)?;
            if let Some(reference) = reference {
                let goal = crate::store::show(repository, &reference)?;
                validate_indexed_source(repository, &goal.entry.display_id)?;
                let goal_fragment =
                    goal.fragment
                        .as_ref()
                        .ok_or_else(|| BelayError::Validation {
                            message: "Task Goal item must identify one SC fragment".into(),
                        })?;
                let full = format!("{}#{}", goal.entry.display_id, goal_fragment.fragment);
                sections.push(context_section(
                    "goal-item",
                    "Goal item",
                    true,
                    2,
                    vec![full],
                    format!(
                        "Definition:\n{}\n{}",
                        goal_fragment.definition,
                        goal_fragment.section.as_deref().unwrap_or("")
                    ),
                ));
                sections.push(context_section(
                    "goal-boundaries",
                    "Goal boundaries",
                    true,
                    3,
                    vec![goal.entry.display_id.clone()],
                    boundary_text(&goal.entry.body, false),
                ));
            } else {
                sections.push(context_section("goal-item","Goal item",true,2,vec![],format!("{goal_item}: Unknown (no linked Goal; retrieve complete Plan before execution).")));
            }
        } else {
            sections.push(context_section(
                "goal-item",
                "Goal item",
                true,
                2,
                vec![],
                "Unknown (no Task mapping).".into(),
            ));
        }
    }
    let mut evidence = ContextEvidence::load(repository)?;
    let (summary, refs) = evidence.full(repository, &canonical)?;
    sections.push(context_section(
        "evidence",
        "Evidence",
        true,
        4,
        refs,
        format!(
            "{summary}\nRecorded state does not establish verification or execution authorization."
        ),
    ));
    bundle_sections(
        &format!("# Task packet: {canonical}\n(compiled by belay, budget={budget})"),
        sections,
        format,
        budget,
    )
}
fn resolve_goal_item(
    repository: &Repository,
    plan: &crate::entry::Entry,
    item: &str,
) -> Result<Option<String>, BelayError> {
    if item.contains(',') || item.split_whitespace().count() != 1 {
        return Err(BelayError::Validation {
            message: "Task must map to exactly one Goal criterion".into(),
        });
    }
    if item.contains('#') {
        let reference = crate::store::resolve_reference(repository, item)?;
        let shown = crate::store::show(repository, &reference.canonical_id())?;
        if shown.entry.entry_type != EntryType::Goal {
            return Err(BelayError::Validation {
                message: "Task Goal item must resolve to a Goal SC".into(),
            });
        }
        return Ok(Some(reference.canonical_id()));
    }
    if !item.to_ascii_lowercase().starts_with("sc-") {
        return Err(BelayError::Validation {
            message: "Task Goal item must identify one SC fragment".into(),
        });
    }
    let mut goals = BTreeSet::new();
    for link in &plan.links {
        if matches!(
            link.relation,
            crate::entry::LinkRelation::Fulfills | crate::entry::LinkRelation::Implements
        ) {
            let reference = crate::entry::parse_entry_reference_id(&link.id)?;
            let shown = crate::store::show(repository, &reference.display_id)?;
            if shown.entry.entry_type == EntryType::Goal {
                goals.insert(shown.entry.display_id);
            }
        }
    }
    if goals.len() > 1 {
        return Err(BelayError::Validation {
            message: "ambiguous Task mapping: Plan links to multiple Goals; qualify Goal item"
                .into(),
        });
    }
    Ok(goals
        .into_iter()
        .next()
        .map(|id| format!("{id}#{}", item.to_ascii_lowercase())))
}

#[derive(Clone)]
struct ContextEvidenceRecord {
    id: String,
    kind: String,
    verdict: String,
    source: String,
    commit: String,
    captured: String,
}
struct ContextEvidence {
    records: BTreeMap<String, Vec<ContextEvidenceRecord>>,
    head: Option<String>,
    freshness: BTreeMap<(String, String), String>,
    git_batch: Option<GitFreshness>,
}
impl ContextEvidence {
    fn load(repository: &Repository) -> Result<Self, BelayError> {
        // Original reader is pack-aware. Corruption is an error, never an empty result.
        let originals = crate::evidence::read_located_mirrors(repository)?;
        let mut records = BTreeMap::<String, Vec<ContextEvidenceRecord>>::new();
        for shown in originals {
            let r = shown.record;
            for link in r.links.iter().filter(|l| l.relation == "verifies") {
                records
                    .entry(link.target.clone())
                    .or_default()
                    .push(ContextEvidenceRecord {
                        id: r.display_id.clone(),
                        kind: r.kind.clone(),
                        verdict: r.verdict.clone(),
                        source: r.source.clone(),
                        commit: r.commit_sha.clone(),
                        captured: r.captured_at.clone(),
                    });
            }
        }
        for items in records.values_mut() {
            items.sort_by(|a, b| {
                let time = |r: &ContextEvidenceRecord| {
                    chrono::DateTime::parse_from_rfc3339(&r.captured)
                        .map(|d| d.timestamp_millis())
                        .unwrap_or(i64::MIN)
                };
                (time(b), &b.id).cmp(&(time(a), &a.id))
            });
        }
        let head = if records.is_empty() {
            None
        } else {
            crate::evidence::current_head(repository).ok()
        };
        let git_batch = head
            .as_ref()
            .and_then(|h| GitFreshness::load(repository, h, &records));
        Ok(Self {
            records,
            head,
            git_batch,
            freshness: BTreeMap::new(),
        })
    }
    fn label(&mut self, repository: &Repository, record: &ContextEvidenceRecord) -> String {
        let key = (record.commit.clone(), record.captured.clone());
        if let Some(label) = self.freshness.get(&key) {
            return label.clone();
        }
        let value = if record.commit != "unknown" && self.head.is_some() {
            // Reuse all timestamp validation and keep adoption independent of freshness.
            let temporal = crate::evidence::freshness(
                repository,
                Some(&record.commit),
                &record.commit,
                &record.captured,
            );
            if !temporal.is_fresh() {
                temporal
            } else if let Some(batch) = &self.git_batch {
                if let Some(behind) = batch.behind(&record.commit) {
                    if behind <= repository.config.verify.stale_after_commits as usize {
                        crate::evidence::Freshness::Fresh
                    } else {
                        crate::evidence::Freshness::Stale(format!("{behind} commits behind"))
                    }
                } else if batch.missing.contains(&record.commit) {
                    crate::evidence::Freshness::Stale("not HEAD".into())
                } else {
                    crate::evidence::freshness(
                        repository,
                        self.head.as_deref(),
                        &record.commit,
                        &record.captured,
                    )
                }
            } else {
                crate::evidence::freshness(
                    repository,
                    self.head.as_deref(),
                    &record.commit,
                    &record.captured,
                )
            }
        } else {
            crate::evidence::freshness(
                repository,
                self.head.as_deref(),
                &record.commit,
                &record.captured,
            )
        };
        let label = value.label();
        self.freshness.insert(key, label.clone());
        label
    }
    fn summary(
        &mut self,
        repository: &Repository,
        target: &str,
    ) -> Result<(String, Vec<String>), BelayError> {
        let Some(record) = self.records.get(target).and_then(|rs| rs.first()).cloned() else {
            return Ok(("missing (unverified)".into(), vec![]));
        };
        let freshness = self.label(repository, &record);
        Ok((
            format!("{} {} {freshness}", record.verdict, record.kind),
            vec![record.id],
        ))
    }
    fn full(
        &mut self,
        repository: &Repository,
        target: &str,
    ) -> Result<(String, Vec<String>), BelayError> {
        let records = self.records.get(target).cloned().unwrap_or_default();
        if records.is_empty() {
            return Ok(("None found; verification Unknown.".into(), vec![]));
        }
        let mut text = String::new();
        let mut refs = Vec::new();
        for record in records {
            let freshness = self.label(repository, &record);
            text.push_str(&format!(
                "- {} {} {} {} {}\n",
                record.id, record.verdict, record.kind, record.source, freshness
            ));
            refs.push(record.id);
        }
        Ok((text, refs))
    }
}
// One bounded graph read and one object probe replace per-entry git processes.
// Incomplete/unsupported graph data falls back to the authoritative freshness API.
struct GitFreshness {
    parents: BTreeMap<String, Vec<String>>,
    head_reachable: BTreeSet<String>,
    missing: BTreeSet<String>,
}
impl GitFreshness {
    fn load(
        repository: &Repository,
        head: &str,
        records: &BTreeMap<String, Vec<ContextEvidenceRecord>>,
    ) -> Option<Self> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let output = Command::new("git")
            .args(["rev-list", "--all", "--parents", "--max-count=10000"])
            .current_dir(&repository.root)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let mut parents = BTreeMap::new();
        for line in String::from_utf8(output.stdout).ok()?.lines() {
            let mut parts = line.split_whitespace();
            parents.insert(
                parts.next()?.to_owned(),
                parts.map(str::to_owned).collect::<Vec<_>>(),
            );
        }
        if !parents.contains_key(head)
            || parents.values().flatten().any(|p| !parents.contains_key(p))
        {
            return None;
        }
        let commits = records
            .values()
            .flatten()
            .map(|r| r.commit.clone())
            .filter(|c| {
                (c.len() == 40 || c.len() == 64)
                    && c.chars().all(|x| x.is_ascii_hexdigit())
                    && !parents.contains_key(c)
            })
            .collect::<BTreeSet<_>>();
        let mut missing = BTreeSet::new();
        if !commits.is_empty() {
            let mut child = Command::new("git")
                .arg("cat-file")
                .arg("--batch-check=%(objectname) %(objecttype)")
                .current_dir(&repository.root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let mut input = child.stdin.take()?;
            let payload = commits.into_iter().collect::<Vec<_>>().join("\n") + "\n";
            // Drain stdout while feeding stdin: large inventories must not fill
            // both pipe buffers and block the reader/writer pair.
            let writer = std::thread::spawn(move || input.write_all(payload.as_bytes()));
            let output = child.wait_with_output();
            writer.join().ok()?.ok()?;
            let output = output.ok()?;
            if !output.status.success() {
                return None;
            }
            for line in String::from_utf8(output.stdout).ok()?.lines() {
                if let Some((id, kind)) = line.split_once(' ') {
                    if kind != "commit" {
                        missing.insert(id.into());
                    }
                }
            }
        }
        let head_reachable = Self::reachable(&parents, head);
        Some(Self {
            parents,
            head_reachable,
            missing,
        })
    }
    fn reachable(parents: &BTreeMap<String, Vec<String>>, start: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![start.to_owned()];
        while let Some(id) = todo.pop() {
            if seen.insert(id.clone()) {
                if let Some(ps) = parents.get(&id) {
                    todo.extend(ps.iter().cloned());
                }
            }
        }
        seen
    }
    fn behind(&self, commit: &str) -> Option<usize> {
        self.parents.contains_key(commit).then(|| {
            self.head_reachable
                .difference(&Self::reachable(&self.parents, commit))
                .count()
        })
    }
}

fn decision_reason(entry: &CompileEntry, state: &str, evidence: &ContextEvidence) -> &'static str {
    match state {
        "replaced" => "accepted successor supersedes/refutes this record",
        "conflict-candidate" => {
            "same explicit scope among adopted decisions; semantic comparison required"
        }
        "explicit-scope" => "adoption recorded by human-approval Evidence",
        _ if entry.status != crate::entry::EntryStatus::Accepted => "accepted status missing",
        _ if entry.scope.is_none() => "scope missing",
        _ if !evidence.records.get(&entry.display_id).is_some_and(|rs| {
            rs.iter()
                .any(|r| r.kind == "human-approval" && r.verdict == "pass")
        }) =>
        {
            "human-approval Evidence missing"
        }
        _ => "applicability Unknown",
    }
}

fn decision_state(
    entry: &CompileEntry,
    pool: &[CompileEntry],
    evidence: &ContextEvidence,
) -> (String, Vec<String>) {
    use crate::entry::EntryStatus;
    let mut refs = vec![];
    for successor in pool
        .iter()
        .filter(|e| e.entry_type == EntryType::Decision && e.status == EntryStatus::Accepted)
    {
        for (target, relation) in &successor.links {
            if target.split('#').next() == Some(entry.display_id.as_str())
                && matches!(relation.as_str(), "supersedes" | "refutes")
            {
                refs.push(successor.display_id.clone());
            }
        }
    }
    if !refs.is_empty() {
        return ("replaced".into(), refs);
    }
    let Some(scope) = entry.scope.as_deref() else {
        return ("unconfirmed".into(), refs);
    };
    if entry.status != EntryStatus::Accepted {
        return ("unconfirmed".into(), refs);
    }
    let approval = evidence.records.get(&entry.display_id).and_then(|rs| {
        rs.iter()
            .find(|r| r.kind == "human-approval" && r.verdict == "pass")
    });
    let Some(approval) = approval else {
        return ("unconfirmed".into(), refs);
    };
    refs.push(approval.id.clone());
    let conflicts = pool
        .iter()
        .filter(|e| {
            e.entry_type == EntryType::Decision
                && e.status == EntryStatus::Accepted
                && e.display_id != entry.display_id
                && e.scope.as_deref() == Some(scope)
                && evidence.records.get(&e.display_id).is_some_and(|rs| {
                    rs.iter()
                        .any(|r| r.kind == "human-approval" && r.verdict == "pass")
                })
        })
        .filter(|e| {
            !pool.iter().any(|s| {
                s.entry_type == EntryType::Decision
                    && s.status == EntryStatus::Accepted
                    && s.links.iter().any(|(id, relation)| {
                        id.split('#').next() == Some(e.display_id.as_str())
                            && matches!(relation.as_str(), "supersedes" | "refutes")
                    })
            })
        })
        .flat_map(|e| {
            let approval = evidence
                .records
                .get(&e.display_id)
                .and_then(|rs| {
                    rs.iter()
                        .find(|r| r.kind == "human-approval" && r.verdict == "pass")
                })
                .expect("eligible peer has adoption Evidence");
            [e.display_id.clone(), approval.id.clone()]
        })
        .collect::<Vec<_>>();
    if !conflicts.is_empty() {
        refs.extend(conflicts);
        ("conflict-candidate".into(), refs)
    } else {
        ("explicit-scope".into(), refs)
    }
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

    fn git(root: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    fn commit(root: &std::path::Path, text: &str) -> String {
        std::fs::write(root.join("fixture.txt"), text).unwrap();
        git(root, &["add", "fixture.txt"]);
        git(root, &["commit", "-qm", text]);
        git(root, &["rev-parse", "HEAD"])
    }
    #[test]
    fn batch_freshness_matches_authoritative_counts_across_branches_and_missing_objects() {
        let dir = tempfile::tempdir().unwrap();
        crate::repository::initialize(dir.path()).unwrap();
        let mut repository = crate::repository::discover(dir.path()).unwrap();
        repository.config.verify.stale_after_commits = 1;
        git(dir.path(), &["init", "-q"]);
        git(
            dir.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(dir.path(), &["config", "user.name", "Fixture"]);
        let root = commit(dir.path(), "root");
        git(dir.path(), &["checkout", "-qb", "side"]);
        let side = commit(dir.path(), "side");
        git(dir.path(), &["checkout", "-qb", "primary", &root]);
        let primary = commit(dir.path(), "primary");
        // Keep side unmerged: behind must count head ancestors absent from side,
        // not distance on one path or timestamp order.
        let head = commit(dir.path(), "head");
        let now = chrono::Utc::now().to_rfc3339();
        let mut records = BTreeMap::new();
        let commits = [
            root,
            side,
            primary,
            head.clone(),
            "f".repeat(40),
            "unknown".into(),
            head[..8].into(),
        ];
        for (i, sha) in commits.iter().enumerate() {
            records.insert(
                format!("target-{i}"),
                vec![ContextEvidenceRecord {
                    id: format!("record-{i}"),
                    kind: "test".into(),
                    verdict: "pass".into(),
                    source: "fixture".into(),
                    commit: sha.clone(),
                    captured: now.clone(),
                }],
            );
        }
        let batch = GitFreshness::load(&repository, &head, &records).unwrap();
        let mut context = ContextEvidence {
            records: records.clone(),
            head: Some(head.clone()),
            freshness: BTreeMap::new(),
            git_batch: Some(batch),
        };
        for record in records.values().flatten() {
            let expected = crate::evidence::freshness(
                &repository,
                Some(&head),
                &record.commit,
                &record.captured,
            )
            .label();
            assert_eq!(
                context.label(&repository, record),
                expected,
                "{}",
                record.commit
            );
        }
        for captured in ["invalid", "2099-01-01T00:00:00Z", "2020-01-01T00:00:00Z"] {
            for sha in [&head, &"unknown".into(), &"f".repeat(40)] {
                let record = ContextEvidenceRecord {
                    id: "temporal".into(),
                    kind: "test".into(),
                    verdict: "pass".into(),
                    source: "fixture".into(),
                    commit: sha.clone(),
                    captured: captured.into(),
                };
                assert_eq!(
                    context.label(&repository, &record),
                    crate::evidence::freshness(&repository, Some(&head), sha, captured).label()
                );
            }
        }
        // A shallow boundary is a root for rev-list too. Compare using a real
        // shallow clone rather than assuming full parent history exists.
        let shallow_dir = tempfile::tempdir().unwrap();
        let destination = shallow_dir.path().join("clone");
        git(
            shallow_dir.path(),
            &[
                "clone",
                "--quiet",
                "--depth=1",
                &format!("file://{}", dir.path().display()),
                destination.to_str().unwrap(),
            ],
        );
        crate::repository::initialize(&destination).unwrap();
        let shallow = crate::repository::discover(&destination).unwrap();
        let batch = GitFreshness::load(&shallow, &head, &records).unwrap();
        let mut context = ContextEvidence {
            records: records.clone(),
            head: Some(head.clone()),
            freshness: BTreeMap::new(),
            git_batch: Some(batch),
        };
        for record in records.values().flatten() {
            assert_eq!(
                context.label(&shallow, record),
                crate::evidence::freshness(&shallow, Some(&head), &record.commit, &record.captured)
                    .label()
            );
        }
        let unavailable = tempfile::tempdir().unwrap();
        crate::repository::initialize(unavailable.path()).unwrap();
        let unavailable = crate::repository::discover(unavailable.path()).unwrap();
        let mut context = ContextEvidence {
            records: records.clone(),
            head: None,
            freshness: BTreeMap::new(),
            git_batch: None,
        };
        for record in records.values().flatten() {
            assert_eq!(
                context.label(&unavailable, record),
                crate::evidence::freshness(&unavailable, None, &record.commit, &record.captured)
                    .label()
            );
        }
    }

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
