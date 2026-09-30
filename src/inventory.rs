//! Reproducible mechanical inventory and explicitly selected reversible status operations.
use crate::{
    BelayError, database,
    entry::{Entry, EntryStatus, EntryType, LinkRelation, MetadataValue},
    evidence, markdown, plan,
    repository::Repository,
    store,
    summary::{self, SourceBinding},
    trace_ids,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub now: DateTime<Utc>,
    pub long_active_days: u64,
}
impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            now: Utc::now(),
            long_active_days: 90,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub rule_version: u32,
    pub references: Vec<SourceBinding>,
    pub reason: String,
    pub actor: String,
    pub class: String,
    pub recommendation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub indexed: bool,
    pub source: SourceBinding,
    pub entry_type: String,
    pub status: String,
    pub source_path: String,
    pub links: Vec<crate::entry::EntryLink>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEvidence {
    pub source: SourceBinding,
    pub verdict: String,
    pub freshness: String,
    pub targets: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionApplicability {
    pub id: String,
    pub scope: Option<String>,
    pub state: String,
    pub evidence_ids: Vec<String>,
    pub successor_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReport {
    pub schema_version: u32,
    pub repository: String,
    pub base_commit: Option<String>,
    pub generated_at: String,
    pub long_active_days: u64,
    pub counts: BTreeMap<String, usize>,
    pub statuses: BTreeMap<String, usize>,
    pub entries: Vec<InventoryEntry>,
    pub evidence: Vec<InventoryEvidence>,
    pub decisions: Vec<DecisionApplicability>,
    pub findings: Vec<Finding>,
}
fn validation(message: impl Into<String>) -> BelayError {
    BelayError::Validation {
        message: message.into(),
    }
}
fn entry_binding(entry: &Entry) -> Result<SourceBinding, BelayError> {
    Ok(SourceBinding {
        id: entry.display_id.clone(),
        revision: entry.revision,
        sha256: markdown::content_hash(entry)?,
    })
}
fn finding(
    rule: &str,
    refs: Vec<SourceBinding>,
    reason: String,
    class: &str,
    recommendation: &str,
) -> Finding {
    let key = format!(
        "{rule}:{}:{reason}",
        refs.iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>()
            .join("|")
    );
    Finding {
        id: format!("INV-{}", &summary::digest(key.as_bytes())[..20]),
        rule: rule.to_owned(),
        rule_version: 1,
        references: refs,
        reason,
        actor: "core".to_owned(),
        class: class.to_owned(),
        recommendation: recommendation.to_owned(),
    }
}

pub fn collect(
    repository: &Repository,
    options: InventoryOptions,
) -> Result<InventoryReport, BelayError> {
    let path = repository.database_path();
    let connection = database::open_read_only(&path)?;
    let mut statement = connection
        .prepare("SELECT id, source_path, content_hash FROM entries ORDER BY display_id")
        .map_err(|e| BelayError::sqlite(&path, e))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| BelayError::sqlite(&path, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BelayError::sqlite(&path, e))?;
    let mut entries = Vec::new();
    let mut originals = Vec::new();
    let mut findings = Vec::new();
    let mut counts = BTreeMap::new();
    let mut statuses = BTreeMap::new();
    let mut missing_originals = BTreeSet::new();
    let mut unconfirmed_originals = BTreeSet::new();
    let mut unavailable_entries = Vec::new();
    let indexed_count = rows.len();
    let mut indexed_ids = BTreeSet::new();
    let mut current_originals = BTreeSet::new();
    for (internal_id, source_path, indexed_hash) in rows {
        let indexed = store::load_entry(&connection, &path, internal_id)?;
        indexed_ids.insert(indexed.display_id.clone());
        // An unavailable original keeps its indexed identity for diagnostics only.
        // Never attach current raw bytes to an indexed revision or status.
        let mut source = entry_binding(&indexed)?;
        let mut current = None;
        match store::read_managed_file(repository, &repository.belay_dir.join(&source_path)) {
            Ok(payload) => match markdown::parse(&payload) {
                Ok(original) => {
                    if original.display_id == indexed.display_id {
                        source = SourceBinding {
                            id: original.display_id.clone(),
                            revision: original.revision,
                            sha256: summary::digest(payload.as_bytes()),
                        };
                    }
                    let original_hash = markdown::content_hash(&original)?;
                    let content_drift = original.display_id != indexed.display_id
                        || original_hash != indexed_hash
                        || markdown::content_hash(&indexed)? != indexed_hash;
                    let revision_drift = original.revision != indexed.revision
                        || original.updated_at != indexed.updated_at;
                    if content_drift || revision_drift {
                        unconfirmed_originals.insert(indexed.display_id.clone());
                        findings.push(finding(
                            "index-drift",
                            vec![source.clone()],
                            if content_drift {
                                "original/index ID or canonical digest differs"
                            } else {
                                "original/index revision or update timestamp differs"
                            }
                            .to_owned(),
                            "fact",
                            "inspect conflict and sync intentionally",
                        ));
                    }
                    if original.display_id == indexed.display_id {
                        current_originals.insert(original.display_id.clone());
                        current = Some(original);
                    }
                }
                Err(error) => {
                    unconfirmed_originals.insert(indexed.display_id.clone());
                    findings.push(finding(
                        "invalid-original",
                        vec![source.clone()],
                        error.to_string(),
                        "fact",
                        "repair original without losing history",
                    ));
                }
            },
            Err(error) => {
                missing_originals.insert(indexed.display_id.clone());
                unconfirmed_originals.insert(indexed.display_id.clone());
                findings.push(finding(
                    "missing-original",
                    vec![source.clone()],
                    error.to_string(),
                    "fact",
                    "restore original; do not infer deletion",
                ));
            }
        }
        let entry = current.as_ref().unwrap_or(&indexed);
        entries.push(InventoryEntry {
            indexed: true,
            source,
            entry_type: entry.entry_type.to_string(),
            status: entry.status.to_string(),
            source_path,
            links: entry.links.clone(),
        });
        if let Some(original) = current {
            originals.push(original);
        } else {
            unconfirmed_originals.insert(indexed.display_id.clone());
            unavailable_entries.push(indexed);
        }
    }
    // Enumerate originals independently: a missing/stale index is not the corpus.
    let mut loaded_ids = current_originals.clone();
    let mut directories = vec![repository.entries_path()];
    let mut loose_count = 0usize;
    let mut unindexed_count = 0usize;
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(BelayError::io(
                    "inspect original directory",
                    &directory,
                    error,
                ));
            }
        };
        if !metadata.file_type().is_dir() {
            return Err(validation("original directory must be a real directory"));
        }
        for item in std::fs::read_dir(&directory)
            .map_err(|error| BelayError::io("list originals", &directory, error))?
        {
            let item = item.map_err(|error| BelayError::io("list originals", &directory, error))?;
            let kind = item
                .file_type()
                .map_err(|error| BelayError::io("inspect original", item.path(), error))?;
            if kind.is_dir() {
                directories.push(item.path());
            } else if item
                .path()
                .extension()
                .is_some_and(|extension| extension == "md")
            {
                files.push(item.path());
            }
        }
    }
    files.sort();
    let mut seen_loose = BTreeMap::<String, String>::new();
    for path in files {
        let payload = match store::read_managed_file(repository, &path) {
            Ok(payload) => payload,
            Err(error) => {
                findings.push(finding(
                    "invalid-original-location",
                    Vec::new(),
                    format!("{}: {error}", path.display()),
                    "fact",
                    "repair original location; never follow symlink",
                ));
                continue;
            }
        };
        let original = match markdown::parse(&payload) {
            Ok(original) => original,
            Err(error) => {
                findings.push(finding(
                    "invalid-unindexed-original",
                    Vec::new(),
                    format!("{}: {error}", path.display()),
                    "fact",
                    "repair original metadata without deleting history",
                ));
                continue;
            }
        };
        loose_count += 1;
        current_originals.insert(original.display_id.clone());
        let raw_hash = summary::digest(payload.as_bytes());
        if let Some(previous) = seen_loose.insert(original.display_id.clone(), raw_hash.clone()) {
            if previous != raw_hash {
                unconfirmed_originals.insert(original.display_id.clone());
                findings.push(finding(
                    "conflicting-original-locations",
                    vec![SourceBinding {
                        id: original.display_id.clone(),
                        revision: original.revision,
                        sha256: raw_hash,
                    }],
                    format!(
                        "different original bytes share ID {} at {}",
                        original.display_id,
                        path.display()
                    ),
                    "fact",
                    "resolve original conflict; do not merge by ID",
                ));
            }
            continue;
        }
        let source = SourceBinding {
            id: original.display_id.clone(),
            revision: original.revision,
            sha256: raw_hash,
        };
        if !indexed_ids.contains(&original.display_id) {
            unindexed_count += 1;
            findings.push(finding(
                "unindexed-original",
                vec![source.clone()],
                format!("original {} absent from SQLite index", path.display()),
                "fact",
                "inspect and sync or rebuild explicitly; report does not mutate index",
            ));
        }
        if loaded_ids.insert(original.display_id.clone()) {
            let item = InventoryEntry {
                indexed: indexed_ids.contains(&original.display_id),
                source,
                entry_type: original.entry_type.to_string(),
                status: original.status.to_string(),
                source_path: path
                    .strip_prefix(&repository.belay_dir)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                links: original.links.clone(),
            };
            if let Some(indexed) = entries
                .iter_mut()
                .find(|entry| entry.source.id == original.display_id)
            {
                *indexed = item;
                unavailable_entries.retain(|entry| entry.display_id != original.display_id);
                missing_originals.remove(&original.display_id);
            } else {
                entries.push(item);
            }
            originals.push(original);
        }
    }
    // Packs retain historical revisions; classify only the latest original per ID.
    let packed = crate::pack::read_entries(repository)?;
    let packed_revision_count = packed.len();
    let mut latest_packed = BTreeMap::new();
    for item in packed {
        if let Some(binding) = entries.iter().find(|entry| {
            entry.source.id == item.entry.display_id
                && entry.source.revision == item.entry.revision
                && !missing_originals.contains(&entry.source.id)
        }) {
            if binding.source.sha256 != item.raw_hash {
                return Err(BelayError::Conflict {
                    message: format!(
                        "conflicting original {}@{}",
                        item.entry.display_id, item.entry.revision
                    ),
                });
            }
        }
        latest_packed.insert(item.entry.display_id.clone(), item);
    }
    let mut unindexed_packed = 0usize;
    for (id, packed) in latest_packed {
        current_originals.insert(id.clone());
        let source = SourceBinding {
            id: id.clone(),
            revision: packed.entry.revision,
            sha256: packed.raw_hash,
        };
        if let Some(position) = originals.iter().position(|entry| entry.display_id == id) {
            let previous = &originals[position];
            if previous.revision > source.revision {
                continue;
            }
            let binding = entries
                .iter()
                .find(|entry| entry.source.id == id)
                .expect("original has binding");
            if previous.revision == source.revision {
                if binding.source.sha256 != source.sha256 {
                    return Err(BelayError::Conflict {
                        message: format!(
                            "conflicting loose/packed original {id}@{}",
                            source.revision
                        ),
                    });
                }
                continue;
            }
            originals.remove(position);
            unconfirmed_originals.insert(id.clone());
            findings.push(finding(
                "index-drift",
                vec![source.clone()],
                "newer packed revision supersedes indexed or loose revision".into(),
                "fact",
                "inspect and rebuild explicitly",
            ));
        }
        if !indexed_ids.contains(&id) {
            unindexed_packed += 1;
            findings.push(finding(
                "unindexed-original",
                vec![source.clone()],
                format!(
                    "packed original {} is absent from SQLite index",
                    packed.original_path
                ),
                "fact",
                "inspect and rebuild explicitly; report does not mutate index",
            ));
        }
        let item = InventoryEntry {
            indexed: indexed_ids.contains(&id),
            source,
            entry_type: packed.entry.entry_type.to_string(),
            status: packed.entry.status.to_string(),
            source_path: packed.original_path,
            links: packed.entry.links.clone(),
        };
        if let Some(existing) = entries.iter_mut().find(|entry| entry.source.id == id) {
            *existing = item;
        } else {
            entries.push(item);
        }
        unavailable_entries.retain(|entry| entry.display_id != id);
        missing_originals.remove(&id);
        originals.push(packed.entry);
    }
    counts.insert("packed-entry-revisions".to_owned(), packed_revision_count);
    counts.insert("unindexed-packed-entries".to_owned(), unindexed_packed);
    counts.insert("indexed-entries".to_owned(), indexed_count);
    counts.insert("original-entries".to_owned(), current_originals.len());
    counts.insert("loose-original-files".to_owned(), loose_count);
    counts.insert("unindexed-original-files".to_owned(), unindexed_count);
    entries.sort_by(|a, b| a.source.id.cmp(&b.source.id));
    originals.sort_by(|a, b| a.display_id.cmp(&b.display_id));
    let by_id = originals
        .iter()
        .map(|e| (e.display_id.as_str(), e))
        .collect::<BTreeMap<_, _>>();
    let bindings = entries
        .iter()
        .map(|entry| (entry.source.id.as_str(), entry.source.clone()))
        .collect::<BTreeMap<_, _>>();
    // Apply the same mechanical classification to all readable originals,
    // regardless of whether SQLite has indexed them.
    for entry in &originals {
        let source = bindings[entry.display_id.as_str()].clone();
        *counts.entry(entry.entry_type.to_string()).or_default() += 1;
        *statuses.entry(entry.status.to_string()).or_default() += 1;
        if entry.entry_type.is_live_status(entry.status) {
            let updated = DateTime::parse_from_rfc3339(&entry.updated_at)
                .map_err(|e| validation(e.to_string()))?;
            let age = options.now.signed_duration_since(updated).num_days();
            if age >= 0 && age as u64 >= options.long_active_days {
                findings.push(finding(
                    "long-active",
                    vec![source.clone()],
                    format!(
                        "live status unchanged for {age} days; threshold {} days",
                        options.long_active_days
                    ),
                    "heuristic",
                    "review original; age does not establish completion",
                ));
            }
        }
        if entry.entry_type == EntryType::Plan {
            // The Plan linter's graph checks use SQLite. Inventory checks
            // references below against the union of readable raw originals.
            for item in plan::lint_entry(&connection, entry)?
                .findings
                .into_iter()
                .filter(|item| item.layer != "graph")
            {
                findings.push(finding(
                    "invalid-task-map",
                    vec![source.clone()],
                    format!("{}: {}", item.field, item.message),
                    "fact",
                    "repair explicit Task and Goal mapping",
                ));
            }
        }
    }
    let mut inbound = BTreeSet::new();
    for entry in &originals {
        for link in &entry.links {
            let parsed = crate::entry::parse_entry_reference_id(&link.id)?;
            inbound.insert(parsed.display_id.clone());
            match by_id.get(parsed.display_id.as_str()) {
                None => findings.push(finding(
                    "missing-reference",
                    vec![bindings[entry.display_id.as_str()].clone()],
                    format!("{} -> {}", link.relation, link.id),
                    "fact",
                    "recover referenced original or correct explicit reference",
                )),
                Some(target)
                    if parsed.fragment.as_ref().is_some_and(|f| {
                        !trace_ids::fragment_exists(target.entry_type, &target.body, f)
                    }) =>
                {
                    findings.push(finding(
                        "invalid-task-reference",
                        vec![
                            bindings[entry.display_id.as_str()].clone(),
                            bindings[target.display_id.as_str()].clone(),
                        ],
                        format!("fragment {} does not resolve uniquely", link.id),
                        "fact",
                        "repair explicit fragment reference",
                    ))
                }
                _ => (),
            }
        }
    }
    for entry in &originals {
        if entry.links.is_empty() && !inbound.contains(&entry.display_id) {
            findings.push(finding(
                "orphan-entry",
                vec![bindings[entry.display_id.as_str()].clone()],
                "no inbound or outbound Entry links".to_owned(),
                "fact",
                "review relationships; isolation does not imply irrelevance",
            ));
        }
    }
    let mut duplicate_groups: BTreeMap<(String, String), Vec<SourceBinding>> = BTreeMap::new();
    for entry in &originals {
        let normalized = entry.body.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalized.is_empty() {
            duplicate_groups
                .entry((entry.entry_type.to_string(), normalized))
                .or_default()
                .push(bindings[entry.display_id.as_str()].clone());
        }
    }
    for refs in duplicate_groups.into_values().filter(|r| r.len() > 1) {
        findings.push(finding(
            "normalized-exact-duplicate",
            refs,
            "same type and whitespace-normalized body".to_owned(),
            "heuristic",
            "read originals for semantic review; never auto-merge",
        ));
    }
    let head = evidence::current_head(repository).ok();
    let records = evidence::read_located_mirrors(repository)?;
    let evidence_ids = records
        .iter()
        .map(|shown| shown.record.display_id.as_str())
        .collect::<BTreeSet<_>>();
    for entry in &originals {
        for span in trace_ids::reference_spans(&entry.body) {
            if span.evidence {
                if !evidence_ids.contains(span.value.as_str()) {
                    findings.push(finding(
                        "missing-evidence-reference",
                        vec![bindings[entry.display_id.as_str()].clone()],
                        format!("body references unavailable Evidence {}", span.value),
                        "fact",
                        "restore original Evidence; do not replace verdict with prose",
                    ));
                }
            } else {
                let parsed = crate::entry::parse_entry_reference_id(&span.value)?;
                match by_id.get(parsed.display_id.as_str()) {
                    None => findings.push(finding(
                        "missing-body-reference",
                        vec![bindings[entry.display_id.as_str()].clone()],
                        format!("body references unavailable Entry {}", span.value),
                        "fact",
                        "recover original or review reference",
                    )),
                    Some(target)
                        if parsed.fragment.as_ref().is_some_and(|fragment| {
                            !trace_ids::fragment_exists(target.entry_type, &target.body, fragment)
                        }) =>
                    {
                        findings.push(finding(
                            "invalid-body-fragment",
                            vec![
                                bindings[entry.display_id.as_str()].clone(),
                                bindings[target.display_id.as_str()].clone(),
                            ],
                            format!("body fragment {} does not resolve uniquely", span.value),
                            "fact",
                            "repair explicit fragment reference",
                        ))
                    }
                    _ => (),
                }
            }
        }
    }
    let mut evidence_items = Vec::new();
    for shown in &records {
        let record = &shown.record;
        let source = SourceBinding {
            id: record.display_id.clone(),
            revision: 1,
            sha256: summary::digest(
                &serde_json::to_vec(record).map_err(|e| validation(e.to_string()))?,
            ),
        };
        let freshness = evidence::freshness_at(
            repository,
            head.as_deref(),
            &record.commit_sha,
            &record.captured_at,
            options.now,
        )
        .label();
        if freshness.starts_with("stale") || freshness.starts_with("unknown") {
            findings.push(finding(
                "evidence-freshness",
                vec![source.clone()],
                freshness.clone(),
                "fact",
                "inspect original Evidence and current verification need",
            ));
        }
        for link in &record.links {
            let parsed = crate::entry::parse_entry_reference_id(&link.target)?;
            if !by_id.contains_key(parsed.display_id.as_str())
                || missing_originals.contains(&parsed.display_id)
            {
                findings.push(finding(
                    "missing-evidence-target",
                    vec![source.clone()],
                    format!("{} target {} unavailable", link.relation, link.target),
                    "fact",
                    "restore original target",
                ));
            } else if let Some(fragment) = parsed.fragment {
                let target = by_id[parsed.display_id.as_str()];
                if !trace_ids::fragment_exists(target.entry_type, &target.body, &fragment) {
                    findings.push(finding(
                        "invalid-evidence-target",
                        vec![source.clone(), bindings[target.display_id.as_str()].clone()],
                        format!("target {} does not resolve uniquely", link.target),
                        "fact",
                        "repair target fragment without changing verdict",
                    ));
                }
            }
        }
        evidence_items.push(InventoryEvidence {
            source,
            verdict: record.verdict.clone(),
            freshness,
            targets: record.links.iter().map(|l| l.target.clone()).collect(),
        });
    }
    evidence_items.sort_by(|a, b| a.source.id.cmp(&b.source.id));
    counts.insert("evidence".to_owned(), evidence_items.len());
    for entry in originals
        .iter()
        .filter(|e| e.entry_type == EntryType::Goal || e.entry_type == EntryType::Work)
    {
        if !records.iter().any(|shown| {
            shown.record.links.iter().any(|l| {
                l.relation == "verifies"
                    && (l.target == entry.display_id
                        || l.target.starts_with(&format!("{}#", entry.display_id)))
            })
        }) {
            findings.push(finding(
                "missing-evidence",
                vec![bindings[entry.display_id.as_str()].clone()],
                "no original verification Evidence targets this Entry or its fragments".to_owned(),
                "fact",
                "record appropriate verification; status is not proof",
            ));
        }
    }
    let mut decisions = Vec::new();
    for entry in originals
        .iter()
        .chain(&unavailable_entries)
        .filter(|e| e.entry_type == EntryType::Decision)
    {
        let scope = match entry.metadata.get("scope") {
            Some(MetadataValue::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
            _ => None,
        };
        let mut evidence_ids = records
            .iter()
            .filter(|s| {
                s.record.kind == "human-approval"
                    && s.record.verdict == "pass"
                    && s.record
                        .links
                        .iter()
                        .any(|l| l.relation == "verifies" && l.target == entry.display_id)
            })
            .map(|s| s.record.display_id.clone())
            .collect::<Vec<_>>();
        evidence_ids.sort();
        let successor_ids = originals
            .iter()
            .filter(|e| {
                e.entry_type == EntryType::Decision
                    && !unconfirmed_originals.contains(&e.display_id)
                    && e.status == EntryStatus::Accepted
                    && e.links.iter().any(|l| {
                        matches!(l.relation, LinkRelation::Supersedes | LinkRelation::Refutes)
                            && l.id == entry.display_id
                    })
            })
            .map(|e| e.display_id.clone())
            .collect::<Vec<_>>();
        let state = if unconfirmed_originals.contains(&entry.display_id) {
            "unconfirmed"
        } else if !successor_ids.is_empty() {
            "replaced"
        } else if entry.status == EntryStatus::Accepted
            && scope.is_some()
            && !evidence_ids.is_empty()
        {
            "explicit-scope"
        } else {
            "unconfirmed"
        };
        decisions.push(DecisionApplicability {
            id: entry.display_id.clone(),
            scope,
            state: state.to_owned(),
            evidence_ids,
            successor_ids,
        });
    }
    decisions.sort_by(|a, b| a.id.cmp(&b.id));
    let scopes = decisions
        .iter()
        .filter(|d| d.state == "explicit-scope")
        .filter_map(|d| d.scope.as_ref().map(|s| (s.clone(), d.id.clone())))
        .fold(
            BTreeMap::<String, Vec<String>>::new(),
            |mut acc, (s, id)| {
                acc.entry(s).or_default().push(id);
                acc
            },
        );
    for (scope, ids) in scopes.into_iter().filter(|(_, ids)| ids.len() > 1) {
        let refs = ids
            .iter()
            .map(|id| bindings[id.as_str()].clone())
            .collect::<Vec<_>>();
        findings.push(finding(
            "decision-scope-conflict",
            refs,
            format!("multiple explicitly adopted Decisions share scope {scope:?}"),
            "semantic-review-proposal",
            "review originals; equal scope is not semantic contradiction",
        ));
        for decision in &mut decisions {
            if ids.contains(&decision.id) {
                decision.state = "conflict-candidate".to_owned();
            }
        }
    }
    findings.sort_by(|a, b| (&a.rule, &a.id).cmp(&(&b.rule, &b.id)));
    findings.dedup_by(|a, b| a.id == b.id);
    Ok(InventoryReport {
        schema_version: 1,
        repository: repository.root.display().to_string(),
        base_commit: head,
        generated_at: options.now.to_rfc3339(),
        long_active_days: options.long_active_days,
        counts,
        statuses,
        entries,
        evidence: evidence_items,
        decisions,
        findings,
    })
}

pub fn render_report(report: &InventoryReport) -> String {
    let mut output = format!(
        "Inventory {} at {} (base {})\nCounts: {:?}\nStatuses: {:?}\n",
        report.repository,
        report.generated_at,
        report.base_commit.as_deref().unwrap_or("unknown"),
        report.counts,
        report.statuses
    );
    for entry in &report.entries {
        output.push_str(&format!(
            "Entry {}@{} [{} {}] sha256:{} links:{}\n",
            entry.source.id,
            entry.source.revision,
            entry.entry_type,
            entry.status,
            entry.source.sha256,
            entry
                .links
                .iter()
                .map(|link| format!("{}:{}", link.relation, link.id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for evidence in &report.evidence {
        output.push_str(&format!(
            "Evidence {} [{}; {}] sha256:{} targets:{}\n",
            evidence.source.id,
            evidence.verdict,
            evidence.freshness,
            evidence.source.sha256,
            evidence.targets.join(", ")
        ));
    }
    for finding in &report.findings {
        output.push_str(&format!(
            "- {} {} v{} [{} / {}] {}\n  Sources: {}\n  Recommendation: {}\n",
            finding.id,
            finding.rule,
            finding.rule_version,
            finding.actor,
            finding.class,
            finding.reason,
            finding
                .references
                .iter()
                .map(|r| format!("{}@{} sha256:{}", r.id, r.revision, r.sha256))
                .collect::<Vec<_>>()
                .join(", "),
            finding.recommendation
        ));
    }
    for decision in &report.decisions {
        output.push_str(&format!(
            "Decision {}: {} scope={:?} adoption={:?} successors={:?}\n",
            decision.id,
            decision.state,
            decision.scope,
            decision.evidence_ids,
            decision.successor_ids
        ));
    }
    output
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusPreview {
    pub schema_version: u32,
    pub operation_id: String,
    pub source: SourceBinding,
    pub before: EntryStatus,
    pub after: EntryStatus,
    pub inbound_references: Vec<String>,
    pub preview_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReceipt {
    pub preview: StatusPreview,
    pub post_revision: u32,
    pub changed: bool,
    pub replayed: bool,
}
fn preview_hash(preview: &StatusPreview) -> Result<String, BelayError> {
    let mut copy = preview.clone();
    copy.preview_hash.clear();
    Ok(summary::digest(
        &serde_json::to_vec(&copy).map_err(|e| validation(e.to_string()))?,
    ))
}
pub fn preview_status_change(
    repository: &Repository,
    id: &str,
    after: EntryStatus,
) -> Result<StatusPreview, BelayError> {
    let source = summary::source_binding(repository, id)?;
    let shown = store::show(repository, id)?;
    if !shown.entry.entry_type.allows_status(after) {
        return Err(validation("invalid target status"));
    }
    let mut inbound_references = shown
        .inbound_links
        .iter()
        .map(|l| format!("{}:{}", l.relation, l.id))
        .collect::<Vec<_>>();
    inbound_references.sort();
    let operation_id = format!("status-{}-{}", &source.sha256[..20], after);
    let mut preview = StatusPreview {
        schema_version: 1,
        operation_id,
        source,
        before: shown.entry.status,
        after,
        inbound_references,
        preview_hash: String::new(),
    };
    preview.preview_hash = preview_hash(&preview)?;
    Ok(preview)
}
pub fn apply_status_preview(
    repository: &Repository,
    preview: &StatusPreview,
) -> Result<StatusReceipt, BelayError> {
    let _writer = crate::lifecycle::writer_lock(repository)?;
    if preview.schema_version != 1 || preview_hash(preview)? != preview.preview_hash {
        return Err(validation("invalid or tampered status preview"));
    }
    let run_id = format!("inventory-{}", preview.preview_hash);
    // Existing transactional receipts are checked before preconditions on retry.
    let receipts = store::route_receipt_targets(repository, &run_id, &preview.preview_hash)?;
    if !receipts.contains_key(&preview.operation_id)
        && summary::source_binding(repository, &preview.source.id)? != preview.source
    {
        return Err(BelayError::Conflict {
            message: "status preview is stale".to_owned(),
        });
    }
    let outcome = store::route_set_status_if_revision(
        repository,
        &run_id,
        &preview.operation_id,
        &preview.preview_hash,
        &preview.source.id,
        preview.after,
        preview.source.revision,
    )?;
    Ok(StatusReceipt {
        preview: preview.clone(),
        post_revision: outcome.post_revision,
        changed: outcome.outcome == store::MutationOutcome::Changed,
        replayed: outcome.replayed,
    })
}
pub fn preview_restore(
    repository: &Repository,
    receipt: &StatusReceipt,
) -> Result<StatusPreview, BelayError> {
    let _writer = crate::lifecycle::writer_lock(repository)?;
    if preview_hash(&receipt.preview)? != receipt.preview.preview_hash {
        return Err(validation("tampered receipt preview"));
    }
    let path = repository.database_path();
    let connection = database::open_read_only(&path)?;
    let run_id = format!("inventory-{}", receipt.preview.preview_hash);
    let recorded: (String,u32) = connection.query_row("SELECT target, post_revision FROM route_operation_receipts WHERE run_id=?1 AND operation_id=?2 AND preview_hash=?3",rusqlite::params![run_id,receipt.preview.operation_id,receipt.preview.preview_hash],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|e|BelayError::sqlite(&path,e))?;
    if recorded.0 != receipt.preview.source.id || recorded.1 != receipt.post_revision {
        return Err(validation(
            "restore receipt does not match recorded operation",
        ));
    }
    let shown = store::show(repository, &receipt.preview.source.id)?;
    if shown.entry.revision != receipt.post_revision || shown.entry.status != receipt.preview.after
    {
        return Err(BelayError::Conflict {
            message: "entry changed after selected status operation; restore requires review"
                .to_owned(),
        });
    }
    preview_status_change(
        repository,
        &receipt.preview.source.id,
        receipt.preview.before,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheCandidate {
    pub path: String,
    pub reason: String,
    pub regeneration: String,
}
pub fn cache_preview(repository: &Repository) -> Vec<CacheCandidate> {
    if repository.database_path().is_file() {
        vec![CacheCandidate {path:repository.database_path().display().to_string(),reason:"SQLite original index is reconstructable; preserve operation receipts before any replacement".to_owned(),regeneration:"belay rebuild (in place; never blanket-remove state)".to_owned()}]
    } else {
        Vec::new()
    }
}
pub fn regenerate_cache(
    repository: &Repository,
    selected_path: &str,
) -> Result<crate::reconcile::RebuildOutcome, BelayError> {
    let _writer = crate::lifecycle::writer_lock(repository)?;
    if selected_path != repository.database_path().display().to_string() {
        return Err(validation(
            "unknown cache path; only explicit in-place index rebuild is allowed",
        ));
    }
    crate::reconcile::rebuild(repository)
}
