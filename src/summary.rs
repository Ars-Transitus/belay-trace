//! Authored, source-bound derived artifacts. They never provide verification authority.
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{BelayError, evidence, markdown, repository::Repository, store};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub id: String,
    pub revision: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummarySection {
    pub heading: String,
    pub text: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryArtifact {
    pub schema_version: u32,
    pub artifact_id: String,
    pub artifact_revision: u32,
    pub generator_version: String,
    pub kind: String,
    pub authority: String,
    pub sources: Vec<SourceBinding>,
    pub sections: Vec<SummarySection>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceState {
    pub source: SourceBinding,
    pub state: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryInspection {
    pub artifact: SummaryArtifact,
    pub current: bool,
    pub sources: Vec<SourceState>,
    pub authority: String,
}

fn validation(message: impl Into<String>) -> BelayError {
    BelayError::Validation {
        message: message.into(),
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Bind to canonical original payload, not its storage location or generated index.
pub fn source_binding(repository: &Repository, id: &str) -> Result<SourceBinding, BelayError> {
    if id.starts_with("EVD-") {
        let shown = evidence::show(repository, id)?;
        if shown.record.display_id != id {
            return Err(validation("summary sources require full canonical IDs"));
        }
        let payload = serde_json::to_vec(&shown.record).map_err(|e| validation(e.to_string()))?;
        return Ok(SourceBinding {
            id: id.to_owned(),
            revision: 1,
            sha256: digest(&payload),
        });
    }
    let shown = store::show(repository, id)?;
    if shown.entry.display_id != id || shown.fragment.is_some() {
        return Err(validation(
            "summary sources require full original Entry IDs",
        ));
    }
    let original =
        store::read_managed_file(repository, &repository.belay_dir.join(&shown.source_path))?;
    let entry = markdown::parse(&original)?;
    if entry.display_id != id
        || entry.revision != shown.entry.revision
        || markdown::content_hash(&entry)? != markdown::content_hash(&shown.entry)?
    {
        return Err(BelayError::Conflict {
            message: format!("source {id} has index/mirror drift"),
        });
    }
    Ok(SourceBinding {
        id: id.to_owned(),
        revision: entry.revision,
        sha256: digest(original.as_bytes()),
    })
}

pub fn validate_artifact(artifact: &SummaryArtifact) -> Result<(), BelayError> {
    if artifact.schema_version != 1
        || artifact.kind != "derived-summary"
        || artifact.authority != "derived-only"
    {
        return Err(validation(
            "unsupported summary schema/kind/authority; only derived-only is allowed",
        ));
    }
    if artifact.artifact_id.is_empty()
        || artifact.artifact_id.len() > 80
        || !artifact
            .artifact_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || artifact.artifact_revision == 0
        || artifact.generator_version.trim().is_empty()
        || artifact.sources.is_empty()
        || artifact.sections.is_empty()
    {
        return Err(validation(
            "summary requires safe artifact ID, positive revision, sources and sections",
        ));
    }
    let mut ids = BTreeSet::new();
    for source in &artifact.sources {
        if source.revision == 0
            || source.sha256.len() != 64
            || !source.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(source.id.as_str())
        {
            return Err(validation("invalid or duplicate summary source binding"));
        }
    }
    for section in &artifact.sections {
        if section.heading.trim().is_empty()
            || section.text.trim().is_empty()
            || section.source_ids.is_empty()
            || section
                .source_ids
                .iter()
                .any(|id| !ids.contains(id.as_str()))
        {
            return Err(validation(
                "summary sections require content and citations to bound sources",
            ));
        }
    }
    Ok(())
}

pub fn inspect_artifact(
    repository: &Repository,
    artifact: &SummaryArtifact,
) -> Result<SummaryInspection, BelayError> {
    validate_artifact(artifact)?;
    let sources = artifact
        .sources
        .iter()
        .map(|source| {
            let (state, reason) = match source_binding(repository, &source.id) {
                Ok(current) if current == *source => {
                    ("current", "original revision and digest match".to_owned())
                }
                Ok(_) => ("stale", "original revision or digest changed".to_owned()),
                Err(BelayError::Conflict { message }) => ("stale", message),
                Err(error) => ("missing", error.to_string()),
            };
            SourceState {
                source: source.clone(),
                state: state.to_owned(),
                reason,
            }
        })
        .collect::<Vec<_>>();
    Ok(SummaryInspection {
        artifact: artifact.clone(),
        current: sources.iter().all(|s| s.state == "current"),
        sources,
        authority: "derived-only; inspect original Evidence and Coverage".to_owned(),
    })
}

fn directory(repository: &Repository) -> PathBuf {
    repository.belay_dir.join("summaries")
}

/// Persist an explicitly authored artifact. No generation, overwrite or source edits.
pub fn save(repository: &Repository, artifact: &SummaryArtifact) -> Result<PathBuf, BelayError> {
    let _writer = crate::lifecycle::writer_lock(repository)?;
    let inspection = inspect_artifact(repository, artifact)?;
    if !inspection.current {
        return Err(BelayError::Conflict {
            message: "summary has stale or missing sources".to_owned(),
        });
    }
    let previous = list(repository)?
        .into_iter()
        .filter(|item| item.artifact.artifact_id == artifact.artifact_id)
        .map(|item| item.artifact.artifact_revision)
        .max()
        .unwrap_or(0);
    if artifact.artifact_revision != previous + 1 {
        return Err(validation(
            "summary revision must be the next authored revision",
        ));
    }
    let dir = directory(repository);
    fs::create_dir_all(&dir).map_err(|e| BelayError::io("create summary directory", &dir, e))?;
    if fs::symlink_metadata(&dir)
        .map_err(|e| BelayError::io("inspect summary directory", &dir, e))?
        .file_type()
        .is_symlink()
    {
        return Err(validation("summary directory must not be a symlink"));
    }
    let path = dir.join(format!(
        "{}-{:06}.json",
        artifact.artifact_id, artifact.artifact_revision
    ));
    let payload = serde_json::to_vec_pretty(artifact).map_err(|e| validation(e.to_string()))?;
    // Publish only a completed file; persist_noclobber prevents revision races.
    let temporary = dir.join(format!(
        ".summary-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| BelayError::io("create summary temporary", &temporary, e))?;
        file.write_all(&payload)
            .map_err(|e| BelayError::io("write summary", &path, e))?;
        file.sync_all()
            .map_err(|e| BelayError::io("sync summary", &path, e))?;
        fs::hard_link(&temporary, &path)
            .map_err(|e| BelayError::io("publish summary without overwrite", &path, e))
    })();
    let _ = fs::remove_file(&temporary);
    result?;
    OpenOptions::new()
        .read(true)
        .open(&dir)
        .and_then(|file| file.sync_all())
        .map_err(|e| BelayError::io("sync summary directory", &dir, e))?;
    Ok(path)
}

pub fn list(repository: &Repository) -> Result<Vec<SummaryInspection>, BelayError> {
    let dir = directory(repository);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if fs::symlink_metadata(&dir)
        .map_err(|e| BelayError::io("inspect summary directory", &dir, e))?
        .file_type()
        .is_symlink()
    {
        return Err(validation("summary directory must not be a symlink"));
    }
    let mut paths = fs::read_dir(&dir)
        .map_err(|e| BelayError::io("list summaries", &dir, e))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BelayError::io("list summaries", &dir, e))?;
    paths.sort();
    let mut results = Vec::new();
    for path in paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|s| s == "json"))
    {
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| BelayError::io("inspect summary", &path, e))?;
        if !metadata.file_type().is_file() {
            return Err(validation("summary must be a regular file"));
        }
        let bytes = fs::read(&path).map_err(|e| BelayError::io("read summary", &path, e))?;
        let artifact: SummaryArtifact = serde_json::from_slice(&bytes)
            .map_err(|e| validation(format!("invalid summary {}: {e}", path.display())))?;
        results.push(inspect_artifact(repository, &artifact)?);
    }
    results.sort_by(|a, b| {
        (&a.artifact.artifact_id, a.artifact.artifact_revision)
            .cmp(&(&b.artifact.artifact_id, b.artifact.artifact_revision))
    });
    Ok(results)
}

pub fn get(repository: &Repository, id: &str) -> Result<SummaryInspection, BelayError> {
    list(repository)?
        .into_iter()
        .filter(|item| item.artifact.artifact_id == id)
        .max_by_key(|item| item.artifact.artifact_revision)
        .ok_or_else(|| validation(format!("summary {id:?} is missing")))
}

pub fn preview_regeneration(repository: &Repository) -> Result<Vec<SummaryInspection>, BelayError> {
    Ok(list(repository)?
        .into_iter()
        .filter(|item| !item.current)
        .collect())
}

/// Select only the latest authored artifact per ID. A summary is optional prose;
/// all required context boundaries and original Evidence remain the caller's job.
pub(crate) fn context_details(
    repository: &Repository,
) -> Result<std::collections::BTreeMap<String, (String, Vec<String>)>, BelayError> {
    use crate::entry::{EntryStatus, EntryType};
    let mut latest = std::collections::BTreeMap::new();
    for inspection in list(repository)? {
        latest.insert(inspection.artifact.artifact_id.clone(), inspection);
    }
    let mut details = std::collections::BTreeMap::new();
    for inspection in latest.into_values().filter(|item| item.current) {
        let artifact = inspection.artifact;
        let references = artifact
            .sources
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>();
        let mut prose = format!(
            "Derived summary {} revision {} (authored prose, not verification; original Evidence below):\n",
            artifact.artifact_id, artifact.artifact_revision
        );
        for section in &artifact.sections {
            prose.push_str(&format!("{}: {}\n", section.heading, section.text));
        }
        for limitation in &artifact.limitations {
            prose.push_str(&format!("Limitation: {limitation}\n"));
        }
        prose.push_str("Expand originals with belay show <source ID>.\n");
        for source in &artifact.sources {
            if source.id.starts_with("EVD-") {
                continue;
            }
            let entry = store::show(repository, &source.id)?.entry;
            if matches!(entry.entry_type, EntryType::Work | EntryType::Review)
                && entry.status == EntryStatus::Completed
            {
                details
                    .entry(source.id.clone())
                    .or_insert_with(|| (prose.clone(), references.clone()));
            }
        }
    }
    Ok(details)
}
