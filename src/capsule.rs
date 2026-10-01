//! Deterministic, read-only Outcome Capsule production.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;

use crate::contract::{self, Lifecycle};
use crate::error::BelayError;
use crate::evidence::{self, Freshness};
use crate::repository::Repository;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Execution {
    NotStarted,
    Running,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verification {
    Unverified,
    Partial,
    Verified,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceState {
    Passing,
    Missing,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBinding {
    pub id: String,
    pub artifact_sha256: String,
    pub state: EvidenceState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub verification: Verification,
    pub evidence: Vec<EvidenceBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capsule {
    pub schema_version: u32,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_digest: String,
    pub evaluated_as_of: String,
    pub execution: Execution,
    pub verification: Verification,
    pub human_acceptance: HumanAcceptance,
    pub lifecycle: Lifecycle,
    pub source_freshness: SourceFreshness,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HumanAcceptance {
    Pending,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFreshness {
    Unknown,
}

pub fn produce(
    repository: &Repository,
    contract_id: &str,
    revision: u32,
    as_of: &str,
    execution: Execution,
) -> Result<Capsule, BelayError> {
    let as_of = DateTime::parse_from_rfc3339(as_of)
        .map_err(|_| invalid("as_of must be an RFC3339 timestamp"))?
        .with_timezone(&Utc);
    let (contract, receipt) = contract::completed_binding(repository, contract_id, revision)?;
    let records = evidence::read_located_mirrors(repository)?;
    let mut criteria = Vec::with_capacity(contract.acceptance_criteria.len());
    for (index, acceptance) in contract.acceptance_criteria.iter().enumerate() {
        let target = format!("{}#sc-{:03}", receipt.goal_id, index + 1);
        let mut bindings = Vec::new();
        for shown in &records {
            let record = &shown.record;
            let captured = DateTime::parse_from_rfc3339(&record.captured_at)
                .map_err(|_| invalid("stored Evidence has an invalid captured_at"))?
                .with_timezone(&Utc);
            if captured > as_of
                || !record
                    .links
                    .iter()
                    .any(|link| link.relation == "verifies" && link.target == target)
            {
                continue;
            }
            let state = match record.verdict.as_str() {
                "fail" => EvidenceState::Failed,
                "pass" => match capsule_freshness(repository, record, as_of) {
                    Freshness::Fresh => EvidenceState::Passing,
                    Freshness::Stale(_) | Freshness::Unknown(_) => EvidenceState::Stale,
                },
                // Informational verdicts are retained as bindings but cannot
                // establish verification under capsule-v1.
                "warn" | "info" => EvidenceState::Missing,
                _ => EvidenceState::Missing,
            };
            let bytes = contract::canonical_bytes(record)?;
            bindings.push(EvidenceBinding {
                id: record.display_id.clone(),
                artifact_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
                state,
            });
        }
        bindings.sort_by(|a, b| a.id.cmp(&b.id));
        let verification = aggregate_evidence(&bindings);
        criteria.push(Criterion {
            id: acceptance.id.clone(),
            verification,
            evidence: bindings,
        });
    }
    let verification = aggregate_criteria(&criteria);
    let lifecycle = if contract.lifecycle == Lifecycle::Active
        && contract.expires_at.as_deref().is_some_and(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|time| time.with_timezone(&Utc) <= as_of)
                .unwrap_or(false)
        }) {
        Lifecycle::Expired
    } else {
        contract.lifecycle
    };
    Ok(Capsule {
        schema_version: 1,
        contract_id: contract.contract_id,
        contract_revision: contract.revision,
        contract_digest: contract.contract_digest,
        evaluated_as_of: as_of.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        execution,
        verification,
        human_acceptance: HumanAcceptance::Pending,
        lifecycle,
        source_freshness: SourceFreshness::Unknown,
        criteria,
    })
}

fn capsule_freshness(
    repository: &Repository,
    record: &evidence::EvidenceRecord,
    as_of: DateTime<Utc>,
) -> Freshness {
    let head = match git(repository, &["rev-parse", "--verify", "HEAD^{commit}"]) {
        Some(value) if !value.is_empty() => value,
        _ => return Freshness::Unknown("git HEAD unavailable".into()),
    };
    if git_status(
        repository,
        &[
            "cat-file",
            "-e",
            &format!("{}^{{commit}}", record.commit_sha),
        ],
    ) != Some(true)
    {
        return Freshness::Unknown("evidence commit unavailable".into());
    }
    if git_status(
        repository,
        &["merge-base", "--is-ancestor", &record.commit_sha, &head],
    ) != Some(true)
    {
        return Freshness::Stale("evidence commit is not an ancestor of HEAD".into());
    }
    let Some(status) = git_bytes(
        repository,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    ) else {
        return Freshness::Unknown("working tree state unavailable".into());
    };
    match porcelain_has_product_changes(&status) {
        Ok(true) => return Freshness::Stale("uncommitted product state".into()),
        Ok(false) => {}
        Err(()) => return Freshness::Unknown("working tree status malformed".into()),
    }
    evidence::record_freshness_at(repository, record, as_of)
}

fn porcelain_has_product_changes(status: &[u8]) -> Result<bool, ()> {
    let mut offset = 0;
    let mut dirty = false;
    while offset < status.len() {
        let end = status[offset..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|position| offset + position)
            .ok_or(())?;
        let record = &status[offset..end];
        if record.len() < 4 || record[2] != b' ' {
            return Err(());
        }
        dirty |= product_path(&record[3..])?;
        let renamed = matches!(record[0], b'R' | b'C') || matches!(record[1], b'R' | b'C');
        offset = end + 1;
        if renamed {
            let old_end = status[offset..]
                .iter()
                .position(|byte| *byte == 0)
                .map(|position| offset + position)
                .ok_or(())?;
            let old_path = &status[offset..old_end];
            if old_path.is_empty() {
                return Err(());
            }
            dirty |= product_path(old_path)?;
            offset = old_end + 1;
        }
    }
    Ok(dirty)
}

fn product_path(path: &[u8]) -> Result<bool, ()> {
    std::str::from_utf8(path).map_err(|_| ())?;
    if path.is_empty() {
        return Err(());
    }
    Ok(path != b".belay" && !path.starts_with(b".belay/"))
}

fn git(repository: &Repository, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(&repository.root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn git_status(repository: &Repository, args: &[&str]) -> Option<bool> {
    Command::new("git")
        .args(args)
        .current_dir(&repository.root)
        .output()
        .ok()
        .map(|output| output.status.success())
}

fn git_bytes(repository: &Repository, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(&repository.root)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// Render uncovered criteria without adding explanatory fields to capsule-v1.
pub fn render_uncovered(capsule: &Capsule) -> String {
    let mut output = format!(
        "Outcome Capsule {} revision {} at {}\n",
        capsule.contract_id, capsule.contract_revision, capsule.evaluated_as_of
    );
    for criterion in &capsule.criteria {
        if criterion.verification != Verification::Verified {
            output.push_str(&format!(
                "- {}: {:?} ({} evidence binding(s))\n",
                criterion.id,
                criterion.verification,
                criterion.evidence.len()
            ));
        }
    }
    output
}

fn aggregate_evidence(bindings: &[EvidenceBinding]) -> Verification {
    if bindings
        .iter()
        .any(|item| item.state == EvidenceState::Failed)
    {
        Verification::Failed
    } else if !bindings.is_empty()
        && bindings
            .iter()
            .all(|item| item.state == EvidenceState::Passing)
    {
        Verification::Verified
    } else if bindings
        .iter()
        .any(|item| item.state == EvidenceState::Passing)
    {
        Verification::Partial
    } else {
        Verification::Unverified
    }
}

fn aggregate_criteria(criteria: &[Criterion]) -> Verification {
    if criteria
        .iter()
        .any(|item| item.verification == Verification::Failed)
    {
        Verification::Failed
    } else if criteria
        .iter()
        .all(|item| item.verification == Verification::Verified)
    {
        Verification::Verified
    } else if criteria
        .iter()
        .all(|item| item.verification == Verification::Unverified)
    {
        Verification::Unverified
    } else {
        Verification::Partial
    }
}

fn invalid(message: impl Into<String>) -> BelayError {
    BelayError::Validation {
        message: message.into(),
    }
}
