//! Validated, content-addressed originals and resumable loose-file retirement.
use crate::{
    BelayError,
    entry::{Entry, EntryStatus, EntryType},
    evidence::EvidenceRecord,
    lifecycle::{self, hash, invalid},
    markdown,
    repository::Repository,
    store,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
thread_local! { static FAILPOINT: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") }; }
fn checkpoint(stage: &str) -> Result<(), BelayError> {
    #[cfg(test)]
    if FAILPOINT.with(|f| f.get() == stage) {
        return invalid(format!("injected interruption at {stage}"));
    }
    let _ = stage;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackedRecord {
    pub kind: String,
    pub display_id: String,
    pub revision: u32,
    pub original_path: String,
    pub source_line: usize,
    pub raw_hash: String,
    pub raw_payload: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub original_path: String,
    pub raw_hash: String,
    pub raw_payload: String,
    pub retire: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    schema_version: u32,
    manifest_hash: String,
    records: Vec<PackedRecord>,
    sources: Vec<SourceSnapshot>,
}
#[derive(Debug, Clone)]
pub struct PackedEntry {
    pub entry: Entry,
    pub raw_payload: String,
    pub original_path: String,
    pub raw_hash: String,
    pub pack_hash: String,
}
#[derive(Debug, Clone)]
pub struct PackedEvidence {
    pub record: EvidenceRecord,
    pub raw_payload: String,
    pub original_path: String,
    pub source_line: usize,
    pub raw_hash: String,
    pub pack_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackPreview {
    pub schema_version: u32,
    pub preview_hash: String,
    pub records: Vec<PackedRecord>,
    pub sources: Vec<SourceSnapshot>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub preview_hash: String,
    pub pack_hash: String,
    pub phase: String,
    pub sources: Vec<SourceSnapshot>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PackOutcome {
    pub receipt: String,
    pub pack_hash: String,
    pub records: usize,
    pub retired: usize,
    pub preserved: Vec<String>,
}

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, BelayError> {
    serde_json::to_vec(value).map_err(|e| BelayError::Validation {
        message: e.to_string(),
    })
}
fn manifest_hash(
    records: &[PackedRecord],
    sources: &[SourceSnapshot],
) -> Result<String, BelayError> {
    let records = records
        .iter()
        .map(|r| {
            (
                &r.kind,
                &r.display_id,
                r.revision,
                &r.original_path,
                r.source_line,
                &r.raw_hash,
            )
        })
        .collect::<Vec<_>>();
    let sources = sources
        .iter()
        .map(|s| (&s.original_path, &s.raw_hash, s.retire))
        .collect::<Vec<_>>();
    Ok(hash(&serialize(&(records, sources))?))
}
fn source_path(repository: &Repository, source: &str) -> Result<PathBuf, BelayError> {
    let path = Path::new(source);
    lifecycle::validate_relative(path)?;
    if !(path.starts_with(&repository.config.storage.entries) || path.starts_with("evidence")) {
        return invalid(format!(
            "pack source {source:?} is outside managed originals"
        ));
    }
    Ok(repository.belay_dir.join(path))
}
fn read_text(repository: &Repository, path: &Path) -> Result<String, BelayError> {
    store::read_loose_managed_file(repository, path)
}
fn source_name(repository: &Repository, path: &Path) -> Result<String, BelayError> {
    store::path_to_storage_string(path.strip_prefix(&repository.belay_dir).map_err(|_| {
        BelayError::Validation {
            message: "source outside managed root".into(),
        }
    })?)
}
fn validate_receipt(receipt: &PackReceipt) -> Result<(), BelayError> {
    if receipt.schema_version != 1
        || receipt.operation_id.is_empty()
        || receipt.operation_id.len() > 128
        || !receipt
            .operation_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !["published", "completed", "restored"].contains(&receipt.phase.as_str())
    {
        return invalid("unsupported or unsafe lifecycle receipt");
    }
    for source in &receipt.sources {
        if hash(source.raw_payload.as_bytes()) != source.raw_hash {
            return invalid("receipt source hash mismatch");
        }
    }
    Ok(())
}
fn validate_pack(repository: &Repository, pack: &Pack) -> Result<(), BelayError> {
    if pack.schema_version != 1 {
        return invalid(format!("unsupported pack schema {}", pack.schema_version));
    }
    if manifest_hash(&pack.records, &pack.sources)? != pack.manifest_hash {
        return invalid("pack manifest hash mismatch");
    }
    let mut keys = BTreeMap::new();
    for record in &pack.records {
        source_path(repository, &record.original_path)?;
        if hash(record.raw_payload.as_bytes()) != record.raw_hash {
            return invalid(format!(
                "pack payload hash mismatch for {}",
                record.display_id
            ));
        }
        let original = Path::new(&record.original_path);
        match record.kind.as_str() {
            "evidence" => {
                let parsed: EvidenceRecord =
                    serde_json::from_str(&record.raw_payload).map_err(|e| {
                        BelayError::Validation {
                            message: format!("invalid packed Evidence: {e}"),
                        }
                    })?;
                crate::evidence::validate_record(&parsed)?;
                if parsed.display_id != record.display_id || record.revision != 1 {
                    return invalid("packed Evidence identity/revision mismatch");
                }
            }
            "work" | "review" => {
                let parsed = markdown::parse(&record.raw_payload)?;
                if parsed.display_id != record.display_id
                    || parsed.revision != record.revision
                    || parsed.entry_type.to_string() != record.kind
                {
                    return invalid("packed Entry identity/revision mismatch");
                }
                if !original.starts_with(
                    repository
                        .config
                        .storage
                        .entries
                        .join(parsed.entry_type.directory()),
                ) || original.file_name().and_then(|s| s.to_str())
                    != Some(format!("{}.md", parsed.display_id).as_str())
                {
                    return invalid("packed Entry source path/type mismatch");
                }
            }
            _ => return invalid(format!("unsupported packed original kind {}", record.kind)),
        }
        let key = (&record.display_id, record.revision);
        if let Some(previous) = keys.insert(key, &record.raw_hash) {
            if previous != &record.raw_hash {
                return invalid(format!(
                    "conflicting pack ID/revision {}@{}",
                    record.display_id, record.revision
                ));
            }
        }
    }
    for source in &pack.sources {
        source_path(repository, &source.original_path)?;
        if hash(source.raw_payload.as_bytes()) != source.raw_hash {
            return invalid("pack source hash mismatch");
        }
        let selected = pack
            .records
            .iter()
            .filter(|r| r.original_path == source.original_path)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return invalid("pack source has no selected originals");
        }
        if source.original_path.ends_with(".ndjson") {
            if !Path::new(&source.original_path).starts_with("evidence") {
                return invalid("monthly source outside Evidence");
            }
            let all = source
                .raw_payload
                .split('\n')
                .enumerate()
                .filter_map(|(i, l)| {
                    let l = l.strip_suffix('\r').unwrap_or(l);
                    (!l.trim().is_empty()).then_some((i + 1, l))
                })
                .collect::<Vec<_>>();
            for r in &selected {
                if r.kind != "evidence"
                    || !all
                        .iter()
                        .any(|(line, raw)| *line == r.source_line && *raw == r.raw_payload)
                {
                    return invalid("packed record does not match original monthly source");
                }
            }
            if source.retire && selected.len() != all.len() {
                return invalid("partial monthly source cannot retire");
            }
        } else {
            if selected.iter().any(|r| r.raw_payload != source.raw_payload) {
                return invalid("packed individual differs from original source");
            }
            if selected.iter().any(|r| r.kind == "evidence")
                && !Path::new(&source.original_path).starts_with("evidence/records")
            {
                return invalid("individual Evidence outside record directory");
            }
        }
    }
    for r in &pack.records {
        if !pack
            .sources
            .iter()
            .any(|s| s.original_path == r.original_path)
        {
            return invalid("pack record missing original source snapshot");
        }
    }
    Ok(())
}
fn list_files(
    repository: &Repository,
    relative: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, BelayError> {
    let directory = repository.belay_dir.join(relative);
    match fs::symlink_metadata(&directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(BelayError::io("inspect original directory", &directory, e)),
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
            return invalid(format!("{} must be a real directory", directory.display()));
        }
        _ => {}
    }
    let mut files = vec![];
    for item in fs::read_dir(&directory)
        .map_err(|e| BelayError::io("read original directory", &directory, e))?
    {
        let path = item
            .map_err(|e| BelayError::io("read original directory", &directory, e))?
            .path();
        let m = fs::symlink_metadata(&path)
            .map_err(|e| BelayError::io("inspect original", &path, e))?;
        if m.file_type().is_symlink() {
            return invalid(format!("{} must not be a symlink", path.display()));
        }
        if m.is_dir() {
            files.extend(list_files(
                repository,
                path.strip_prefix(&repository.belay_dir).expect("managed"),
                extension,
            )?);
        } else if m.is_file()
            && path.extension().and_then(|x| x.to_str()) == Some(extension)
            && !path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .starts_with('.')
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
fn read_packs(repository: &Repository) -> Result<Vec<(String, Pack)>, BelayError> {
    let mut result = vec![];
    for path in list_files(repository, Path::new("packs"), "json")? {
        let bytes = read_text(repository, &path)?;
        let pack_hash = hash(bytes.as_bytes());
        if path.file_stem().and_then(|s| s.to_str()) != Some(pack_hash.as_str()) {
            return invalid(format!("pack hash mismatch at {}", path.display()));
        }
        let pack: Pack = serde_json::from_str(&bytes).map_err(|e| BelayError::Validation {
            message: format!("invalid pack {}: {e}", path.display()),
        })?;
        validate_pack(repository, &pack)?;
        result.push((pack_hash, pack));
    }
    let present = result
        .iter()
        .map(|(h, _)| h.as_str())
        .collect::<BTreeSet<_>>();
    for path in list_files(repository, Path::new("lifecycle/receipts"), "json")? {
        let raw = read_text(repository, &path)?;
        let receipt: PackReceipt =
            serde_json::from_str(&raw).map_err(|e| BelayError::Validation {
                message: format!("invalid lifecycle receipt {}: {e}", path.display()),
            })?;
        validate_receipt(&receipt)?;
        if !present.contains(receipt.pack_hash.as_str()) {
            return invalid(format!(
                "missing or unsupported pack for receipt {}",
                path.display()
            ));
        }
        let (_, pack) = result
            .iter()
            .find(|(h, _)| h == &receipt.pack_hash)
            .expect("present");
        if receipt.preview_hash != pack.manifest_hash
            || receipt.sources.iter().any(|s| {
                !pack.sources.iter().any(|p| {
                    p.original_path == s.original_path
                        && p.raw_hash == s.raw_hash
                        && p.retire == s.retire
                })
            })
        {
            return invalid("lifecycle receipt disagrees with pack manifest");
        }
    }
    Ok(result)
}
pub fn read_entries(repository: &Repository) -> Result<Vec<PackedEntry>, BelayError> {
    let mut by_revision = BTreeMap::<(String, u32), PackedEntry>::new();
    for (pack_hash, pack) in read_packs(repository)? {
        for r in pack.records {
            if r.kind == "evidence" {
                continue;
            }
            let entry = markdown::parse(&r.raw_payload)?;
            let key = (entry.display_id.clone(), entry.revision);
            if let Some(old) = by_revision.get(&key) {
                if old.raw_hash != r.raw_hash {
                    return invalid(format!(
                        "conflicting packed ID/revision {}@{}",
                        key.0, key.1
                    ));
                }
                continue;
            }
            by_revision.insert(
                key,
                PackedEntry {
                    entry,
                    raw_payload: r.raw_payload,
                    original_path: r.original_path,
                    raw_hash: r.raw_hash,
                    pack_hash: pack_hash.clone(),
                },
            );
        }
    }
    Ok(by_revision.into_values().collect())
}
pub fn read_evidence(repository: &Repository) -> Result<Vec<PackedEvidence>, BelayError> {
    let mut records = BTreeMap::<String, PackedEvidence>::new();
    for (pack_hash, pack) in read_packs(repository)? {
        for r in pack.records {
            if r.kind != "evidence" {
                continue;
            }
            if let Some(old) = records.get(&r.display_id) {
                if old.raw_hash != r.raw_hash {
                    return invalid(format!("conflicting evidence ID {} in packs", r.display_id));
                }
                continue;
            }
            let record =
                serde_json::from_str(&r.raw_payload).map_err(|e| BelayError::Validation {
                    message: e.to_string(),
                })?;
            records.insert(
                r.display_id,
                PackedEvidence {
                    record,
                    raw_payload: r.raw_payload,
                    original_path: r.original_path,
                    source_line: r.source_line,
                    raw_hash: r.raw_hash,
                    pack_hash: pack_hash.clone(),
                },
            );
        }
    }
    Ok(records.into_values().collect())
}

/// Retrieve exact original bytes by identity and revision, including history.
pub fn resolve_original(
    repository: &Repository,
    id: &str,
    revision: u32,
) -> Result<PackedRecord, BelayError> {
    let mut selected: Option<PackedRecord> = None;
    let mut candidates = loose_evidence(repository)?
        .into_iter()
        .map(|(r, _)| r)
        .collect::<Vec<_>>();
    if !id.starts_with("EVD-") {
        for path in list_files(repository, &repository.config.storage.entries, "md")? {
            let raw = read_text(repository, &path)?;
            let entry = markdown::parse(&raw)?;
            if entry.display_id == id && entry.revision == revision {
                candidates.push(PackedRecord {
                    kind: entry.entry_type.to_string(),
                    display_id: entry.display_id,
                    revision: entry.revision,
                    original_path: source_name(repository, &path)?,
                    source_line: 0,
                    raw_hash: hash(raw.as_bytes()),
                    raw_payload: raw,
                });
            }
        }
    }
    for (_, pack) in read_packs(repository)? {
        candidates.extend(pack.records);
    }
    for candidate in candidates {
        if candidate.display_id != id || candidate.revision != revision {
            continue;
        }
        if let Some(old) = &selected {
            if old.raw_hash != candidate.raw_hash {
                return invalid(format!("conflicting original ID/revision {id}@{revision}"));
            }
        } else {
            selected = Some(candidate);
        }
    }
    selected.ok_or_else(|| BelayError::Validation {
        message: format!("original {id}@{revision} was not found"),
    })
}
pub fn original_at(repository: &Repository, path: &Path) -> Result<Option<String>, BelayError> {
    let source = source_name(repository, path)?;
    let mut selected: Option<(u32, String)> = None;
    for entry in read_entries(repository)? {
        if entry.original_path == source
            && selected
                .as_ref()
                .is_none_or(|(revision, _)| entry.entry.revision > *revision)
        {
            selected = Some((entry.entry.revision, entry.raw_payload));
        }
    }
    Ok(selected.map(|(_, raw)| raw))
}
pub(crate) fn loose_entries(repository: &Repository) -> Result<Vec<Entry>, BelayError> {
    let mut entries = vec![];
    for path in list_files(repository, &repository.config.storage.entries, "md")? {
        entries.push(markdown::parse(&read_text(repository, &path)?)?);
    }
    Ok(entries)
}
pub fn loose_evidence(
    repository: &Repository,
) -> Result<Vec<(PackedRecord, SourceSnapshot)>, BelayError> {
    let mut result = vec![];
    let mut originals =
        store::read_directory_files(repository, &repository.evidence_path(), "ndjson")?;
    originals.extend(store::read_directory_files(
        repository,
        &repository.evidence_path().join("records"),
        "json",
    )?);
    originals.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, raw) in originals {
        let name = source_name(repository, &path)?;
        let source = SourceSnapshot {
            original_path: name.clone(),
            raw_hash: hash(raw.as_bytes()),
            raw_payload: raw.clone(),
            retire: true,
        };
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let record: EvidenceRecord =
                serde_json::from_str(&raw).map_err(|e| BelayError::Validation {
                    message: format!("invalid evidence JSON in {name}: {e}"),
                })?;
            crate::evidence::validate_record(&record)?;
            if path.file_stem().and_then(|s| s.to_str()) != Some(record.display_id.as_str()) {
                return invalid("Evidence filename differs from record ID");
            }
            result.push((
                PackedRecord {
                    kind: "evidence".into(),
                    display_id: record.display_id,
                    revision: 1,
                    original_path: name,
                    source_line: 1,
                    raw_hash: hash(raw.as_bytes()),
                    raw_payload: raw,
                },
                source,
            ));
        } else {
            for (index, line) in raw.split('\n').enumerate() {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if line.trim().is_empty() {
                    continue;
                }
                let record: EvidenceRecord =
                    serde_json::from_str(line).map_err(|e| BelayError::Validation {
                        message: format!("invalid evidence JSON in {name}:{}: {e}", index + 1),
                    })?;
                crate::evidence::validate_record(&record)?;
                result.push((
                    PackedRecord {
                        kind: "evidence".into(),
                        display_id: record.display_id,
                        revision: 1,
                        original_path: name.clone(),
                        source_line: index + 1,
                        raw_hash: hash(line.as_bytes()),
                        raw_payload: line.into(),
                    },
                    source.clone(),
                ));
            }
        }
    }
    Ok(result)
}
pub fn preview(repository: &Repository, ids: &[String]) -> Result<PackPreview, BelayError> {
    // Validate existing mixed placement before selecting a new operation.
    crate::evidence::read_located_mirrors(repository)?;
    read_entries(repository)?;
    let wanted = ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut records = vec![];
    let mut sources = BTreeMap::new();
    let evidence = loose_evidence(repository)?;
    let all_per_source = evidence
        .iter()
        .fold(BTreeMap::<String, usize>::new(), |mut m, (_, s)| {
            *m.entry(s.original_path.clone()).or_default() += 1;
            m
        });
    for (record, source) in evidence {
        if wanted.is_empty() || wanted.contains(&record.display_id) {
            records.push(record);
            sources.insert(source.original_path.clone(), source);
        }
    }
    for entry_type in [EntryType::Work, EntryType::Review] {
        for path in list_files(
            repository,
            &repository
                .config
                .storage
                .entries
                .join(entry_type.directory()),
            "md",
        )? {
            let raw = read_text(repository, &path)?;
            let entry = markdown::parse(&raw)?;
            if !wanted.is_empty() && !wanted.contains(&entry.display_id) {
                continue;
            }
            if !matches!(
                entry.status,
                EntryStatus::Completed | EntryStatus::Abandoned | EntryStatus::Archived
            ) {
                if wanted.contains(&entry.display_id) {
                    return invalid(format!(
                        "{} is not a terminal Work/Review",
                        entry.display_id
                    ));
                }
                continue;
            }
            let live = store::show(repository, &entry.display_id)?;
            if live.entry.revision != entry.revision
                || markdown::content_hash(&live.entry)? != markdown::content_hash(&entry)?
            {
                return Err(BelayError::Conflict {
                    message: format!("{} is not synchronized and stable", entry.display_id),
                });
            }
            for link in &entry.links {
                store::resolve_reference(repository, &link.id)?;
            }
            let name = source_name(repository, &path)?;
            let raw_hash = hash(raw.as_bytes());
            records.push(PackedRecord {
                kind: entry.entry_type.to_string(),
                display_id: entry.display_id,
                revision: entry.revision,
                original_path: name.clone(),
                source_line: 0,
                raw_hash: raw_hash.clone(),
                raw_payload: raw.clone(),
            });
            sources.insert(
                name.clone(),
                SourceSnapshot {
                    original_path: name,
                    raw_hash,
                    raw_payload: raw,
                    retire: true,
                },
            );
        }
    }
    if !wanted.is_empty() {
        let found = records
            .iter()
            .map(|r| r.display_id.clone())
            .collect::<BTreeSet<_>>();
        let missing = wanted.difference(&found).collect::<Vec<_>>();
        if !missing.is_empty() {
            return invalid(format!(
                "selected originals are missing or already packed: {missing:?}"
            ));
        }
    }
    for (name, source) in &mut sources {
        if let Some(total) = all_per_source.get(name) {
            source.retire = records.iter().filter(|r| &r.original_path == name).count() == *total;
        }
    }
    records.sort_by(|a, b| {
        (&a.display_id, a.revision, &a.original_path, a.source_line).cmp(&(
            &b.display_id,
            b.revision,
            &b.original_path,
            b.source_line,
        ))
    });
    let sources = sources.into_values().collect::<Vec<_>>();
    let preview_hash = manifest_hash(&records, &sources)?;
    Ok(PackPreview {
        schema_version: 1,
        preview_hash,
        records,
        sources,
    })
}
fn validate_preview(repository: &Repository, preview: &PackPreview) -> Result<(), BelayError> {
    let pack = Pack {
        schema_version: preview.schema_version,
        manifest_hash: preview.preview_hash.clone(),
        records: preview.records.clone(),
        sources: preview.sources.clone(),
    };
    validate_pack(repository, &pack)?;
    for source in &preview.sources {
        let path = source_path(repository, &source.original_path)?;
        let raw = read_text(repository, &path)?;
        if hash(raw.as_bytes()) != source.raw_hash {
            return Err(BelayError::Conflict {
                message: format!("{} changed since pack preview", source.original_path),
            });
        }
    }
    Ok(())
}
fn publish_receipt(repository: &Repository, receipt: &PackReceipt) -> Result<String, BelayError> {
    lifecycle::ensure_directory(repository, Path::new("lifecycle/receipts"))?;
    let name = format!("{}-{}.json", receipt.operation_id, receipt.phase);
    let path = repository.belay_dir.join("lifecycle/receipts").join(&name);
    let raw = serialize(receipt)?;
    if path
        .try_exists()
        .map_err(|e| BelayError::io("inspect receipt", &path, e))?
    {
        if read_text(repository, &path)?.as_bytes() != raw {
            return invalid("conflicting lifecycle receipt");
        }
    } else {
        store::write_new_file(repository, &path, &raw)?;
    }
    Ok(format!("lifecycle/receipts/{name}"))
}
pub fn apply(repository: &Repository, preview: &PackPreview) -> Result<PackOutcome, BelayError> {
    let _guard = lifecycle::writer_lock(repository)?;
    let pending = format!("lifecycle/receipts/{}-published.json", preview.preview_hash);
    if repository
        .belay_dir
        .join(&pending)
        .try_exists()
        .map_err(|e| BelayError::io("inspect pack retry", &pending, e))?
    {
        return recover(repository, &pending);
    }
    validate_preview(repository, preview)?;
    for r in &preview.records {
        if r.kind != "evidence" {
            let entry = markdown::parse(&r.raw_payload)?;
            if !matches!(
                entry.status,
                EntryStatus::Completed | EntryStatus::Abandoned | EntryStatus::Archived
            ) {
                return invalid("preview selects nonterminal Entry");
            }
            let live = store::show(repository, &r.display_id)?;
            if live.entry.revision != r.revision
                || markdown::content_hash(&live.entry)? != markdown::content_hash(&entry)?
            {
                return Err(BelayError::Conflict {
                    message: format!("{} changed since pack preview", r.display_id),
                });
            }
        }
    }
    if preview.records.is_empty() {
        return invalid("pack preview has no selected originals");
    }
    lifecycle::ensure_v2(repository)?;
    lifecycle::ensure_directory(repository, Path::new("packs"))?;
    let pack = Pack {
        schema_version: 1,
        manifest_hash: preview.preview_hash.clone(),
        records: preview.records.clone(),
        sources: preview.sources.clone(),
    };
    let raw = serialize(&pack)?;
    let pack_hash = hash(&raw);
    let path = repository
        .belay_dir
        .join("packs")
        .join(format!("{pack_hash}.json"));
    if path
        .try_exists()
        .map_err(|e| BelayError::io("inspect pack", &path, e))?
    {
        if read_text(repository, &path)?.as_bytes() != raw {
            return invalid("existing pack contents differ");
        }
    } else {
        store::write_new_file(repository, &path, &raw)?;
    }
    validate_pack(
        repository,
        &serde_json::from_str(&read_text(repository, &path)?).map_err(|e| {
            BelayError::Validation {
                message: e.to_string(),
            }
        })?,
    )?;
    checkpoint("pack-published")?;
    let receipt = PackReceipt {
        schema_version: 1,
        operation_id: preview.preview_hash.clone(),
        preview_hash: preview.preview_hash.clone(),
        pack_hash: pack_hash.clone(),
        phase: "published".into(),
        sources: preview.sources.clone(),
    };
    let receipt_name = publish_receipt(repository, &receipt)?;
    checkpoint("journal-published")?;
    retire(repository, &receipt, &receipt_name, pack.records.len())
}
fn retire(
    repository: &Repository,
    receipt: &PackReceipt,
    receipt_name: &str,
    records: usize,
) -> Result<PackOutcome, BelayError> {
    let mut retired = 0;
    let mut preserved = vec![];
    for (index, source) in receipt.sources.iter().enumerate() {
        if !source.retire {
            preserved.push(source.original_path.clone());
            continue;
        }
        let path = source_path(repository, &source.original_path)?;
        let parent = path.parent().expect("managed parent");
        let staged = parent.join(format!(".tmp-pack-{}-{index}", receipt.operation_id));
        if !staged
            .try_exists()
            .map_err(|e| BelayError::io("inspect staged original", &staged, e))?
        {
            match read_text(repository, &path) {
                Err(BelayError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    continue;
                }
                Err(e) => return Err(e),
                Ok(raw) if hash(raw.as_bytes()) != source.raw_hash => {
                    preserved.push(source.original_path.clone());
                    continue;
                }
                Ok(_) => {}
            }
            fs::rename(&path, &staged)
                .map_err(|e| BelayError::io("stage packed original", &path, e))?;
            lifecycle::sync_directory(parent)?;
            checkpoint("source-staged")?;
        }
        let raw = read_text(repository, &staged)?;
        if hash(raw.as_bytes()) != source.raw_hash {
            if !path
                .try_exists()
                .map_err(|e| BelayError::io("inspect changed source", &path, e))?
            {
                fs::hard_link(&staged, &path)
                    .map_err(|e| BelayError::io("restore changed original", &path, e))?;
                lifecycle::sync_directory(parent)?;
                fs::remove_file(&staged)
                    .map_err(|e| BelayError::io("remove recovered staging name", &staged, e))?;
                lifecycle::sync_directory(parent)?;
            }
            preserved.push(source.original_path.clone());
            continue;
        }
        fs::remove_file(&staged)
            .map_err(|e| BelayError::io("retire verified packed original", &staged, e))?;
        lifecycle::sync_directory(parent)?;
        retired += 1;
        checkpoint("source-retired")?;
    }
    let mut completed = receipt.clone();
    completed.phase = "completed".into();
    let final_receipt = publish_receipt(repository, &completed)?;
    Ok(PackOutcome {
        receipt: if preserved.is_empty() {
            final_receipt
        } else {
            receipt_name.into()
        },
        pack_hash: receipt.pack_hash.clone(),
        records,
        retired,
        preserved,
    })
}
pub fn recover(repository: &Repository, receipt_name: &str) -> Result<PackOutcome, BelayError> {
    let _guard = lifecycle::writer_lock(repository)?;
    let name = if receipt_name.starts_with("lifecycle/receipts/") {
        receipt_name.to_owned()
    } else {
        format!("lifecycle/receipts/{receipt_name}")
    };
    lifecycle::validate_relative(Path::new(&name))?;
    let path = repository.belay_dir.join(&name);
    let receipt: PackReceipt =
        serde_json::from_str(&read_text(repository, &path)?).map_err(|e| {
            BelayError::Validation {
                message: e.to_string(),
            }
        })?;
    validate_receipt(&receipt)?;
    let packs = read_packs(repository)?;
    let (_, pack) = packs
        .into_iter()
        .find(|(h, _)| h == &receipt.pack_hash)
        .ok_or_else(|| BelayError::Validation {
            message: "recover pack missing".into(),
        })?;
    if receipt.preview_hash != pack.manifest_hash
        || receipt.sources.iter().any(|s| {
            !pack.sources.iter().any(|p| {
                p.original_path == s.original_path
                    && p.raw_hash == s.raw_hash
                    && p.retire == s.retire
            })
        })
    {
        return invalid("receipt sources differ from verified pack");
    }
    if receipt.phase == "restored" {
        return Ok(PackOutcome {
            receipt: name,
            pack_hash: receipt.pack_hash,
            records: pack.records.len(),
            retired: 0,
            preserved: vec![],
        });
    }
    let completed = format!("lifecycle/receipts/{}-completed.json", receipt.operation_id);
    if repository
        .belay_dir
        .join(&completed)
        .try_exists()
        .map_err(|e| BelayError::io("inspect completed operation", &completed, e))?
    {
        return Ok(PackOutcome {
            receipt: completed,
            pack_hash: receipt.pack_hash,
            records: pack.records.len(),
            retired: 0,
            preserved: vec![],
        });
    }
    retire(repository, &receipt, &name, pack.records.len())
}
pub fn restore(
    repository: &Repository,
    pack_id: &str,
    id: Option<&str>,
) -> Result<PackOutcome, BelayError> {
    let _guard = lifecycle::writer_lock(repository)?;
    let pack_id = pack_id.strip_suffix(".json").unwrap_or(pack_id);
    let packs = read_packs(repository)?;
    let (pack_hash, pack) = packs
        .into_iter()
        .find(|(h, _)| h == pack_id)
        .ok_or_else(|| BelayError::Validation {
            message: format!("pack {pack_id} not found"),
        })?;
    let mut restored = 0;
    let mut preserved = vec![];
    let selected = pack
        .records
        .iter()
        .filter(|r| id.is_none_or(|id| r.display_id == id))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return invalid("restore selected no packed original");
    }
    let selected_paths = selected
        .iter()
        .map(|r| r.original_path.as_str())
        .collect::<BTreeSet<_>>();
    for source in &pack.sources {
        if !selected_paths.contains(source.original_path.as_str()) {
            continue;
        }
        let path = source_path(repository, &source.original_path)?;
        if path
            .try_exists()
            .map_err(|e| BelayError::io("inspect restore source", &path, e))?
        {
            let live = read_text(repository, &path)?;
            if hash(live.as_bytes()) == source.raw_hash {
                continue;
            }
            if let Ok(entry) = markdown::parse(&live) {
                let old = selected
                    .iter()
                    .filter(|r| r.original_path == source.original_path)
                    .map(|r| r.revision)
                    .max()
                    .unwrap_or(0);
                if entry.revision > old {
                    preserved.push(source.original_path.clone());
                    continue;
                }
            }
            return Err(BelayError::Conflict {
                message: format!(
                    "restore refuses conflicting live source {}",
                    source.original_path
                ),
            });
        }
        if let Some(parent) = path.parent() {
            lifecycle::ensure_directory(
                repository,
                parent.strip_prefix(&repository.belay_dir).expect("managed"),
            )?;
        }
        store::write_new_file(repository, &path, source.raw_payload.as_bytes())?;
        restored += 1;
    }
    let receipt = PackReceipt {
        schema_version: 1,
        operation_id: format!("restore-{}", lifecycle::random_id()?),
        preview_hash: pack.manifest_hash,
        pack_hash: pack_hash.clone(),
        phase: "restored".into(),
        sources: pack.sources,
    };
    let receipt = publish_receipt(repository, &receipt)?;
    Ok(PackOutcome {
        receipt,
        pack_hash,
        records: selected.len(),
        retired: restored,
        preserved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{self, RecordInput};
    fn fixture() -> (tempfile::TempDir, Repository, String) {
        let dir = tempfile::tempdir().unwrap();
        let repo = crate::repository::initialize(dir.path())
            .unwrap()
            .repository;
        let goal = store::create(
            &repo,
            EntryType::Goal,
            "crash-test".into(),
            "fixture".into(),
        )
        .unwrap();
        for n in 0..2 {
            evidence::record(
                &repo,
                RecordInput {
                    kind: "test".into(),
                    verdict: "pass".into(),
                    commit_sha: Some("abc".into()),
                    captured_at: Some("2026-09-01T12:00:00Z".into()),
                    source: "fixture".into(),
                    issuer: "test".into(),
                    summary: format!("record {n}"),
                    detail: serde_json::Value::Null,
                    verifies: vec![goal.display_id.clone()],
                },
            )
            .unwrap();
        }
        (dir, repo, goal.display_id)
    }
    #[test]
    fn each_interrupted_phase_recovers_exact_originals() {
        for phase in [
            "pack-published",
            "journal-published",
            "source-staged",
            "source-retired",
        ] {
            let (_dir, repo, _goal) = fixture();
            let preview = preview(&repo, &[]).unwrap();
            let original = preview
                .sources
                .iter()
                .map(|s| (s.original_path.clone(), s.raw_hash.clone()))
                .collect::<BTreeMap<_, _>>();
            FAILPOINT.with(|p| p.set(phase));
            let failed = apply(&repo, &preview);
            FAILPOINT.with(|p| p.set(""));
            assert!(failed.is_err(), "{phase}");
            assert_eq!(
                evidence::read_located_mirrors(&repo).unwrap().len(),
                2,
                "{phase}"
            );
            let outcome = apply(&repo, &preview).unwrap();
            assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
            restore(&repo, &outcome.pack_hash, None).unwrap();
            for (path, expected) in original {
                assert_eq!(
                    hash(&fs::read(repo.belay_dir.join(path)).unwrap()),
                    expected,
                    "{phase}"
                );
            }
            assert_eq!(evidence::read_located_mirrors(&repo).unwrap().len(), 2);
        }
    }
    #[test]
    fn changed_source_after_journal_stays_loose_and_conflicting() {
        let (_dir, repo, _) = fixture();
        let preview = preview(&repo, &[]).unwrap();
        FAILPOINT.with(|p| p.set("journal-published"));
        assert!(apply(&repo, &preview).is_err());
        FAILPOINT.with(|p| p.set(""));
        let source = &preview.sources[0];
        let path = repo.belay_dir.join(&source.original_path);
        let changed = source
            .raw_payload
            .replace("record 0", "changed record")
            .replace("record 1", "changed record");
        fs::write(&path, &changed).unwrap();
        let outcome = apply(&repo, &preview).unwrap();
        assert_eq!(outcome.preserved, vec![source.original_path.clone()]);
        assert_eq!(fs::read_to_string(path).unwrap(), changed);
        assert!(
            evidence::read_located_mirrors(&repo)
                .unwrap_err()
                .to_string()
                .contains("conflicting")
        );
    }
}
