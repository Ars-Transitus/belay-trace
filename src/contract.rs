//! Strict Omnia Contract validation and crash-recoverable Belay projection.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::entry::{Entry, EntryStatus, EntryType, MetadataValue};
use crate::error::BelayError;
use crate::repository::Repository;

pub const COMPILER_VERSION: &str = "omnia-contract-compiler/1";
const EDITABLE_TASKS_MARKER: &str = "<!-- belay-contract:editable-tasks-v1 -->";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub contract_id: String,
    pub revision: u32,
    pub issued_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub compiler_version: String,
    pub outcome: String,
    pub requirements: Vec<SourcedItem>,
    pub acceptance_criteria: Vec<SourcedItem>,
    pub constraints: Vec<String>,
    pub non_goals: Vec<String>,
    pub unknowns: Vec<String>,
    pub delegation: Delegation,
    pub verification: Vec<String>,
    pub stop_conditions: Vec<String>,
    pub target: Target,
    pub source_bundle: SourceBundle,
    pub lifecycle: Lifecycle,
    pub contract_digest: String,
    pub input_id: String,
    pub input_revision: u32,
    pub confirmation_ref: String,
    pub assumptions: Vec<String>,
    pub context_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourcedItem {
    pub id: String,
    pub text: String,
    pub source_refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Delegation {
    pub actor: String,
    pub scope: String,
    pub allowed_operations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub repository: String,
    pub base_commit: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceBundle {
    pub bundle_id: String,
    pub bundle_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition_digest: Option<String>,
    pub sources: Vec<Source>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub url: String,
    pub retrieved_at: String,
    pub raw_snapshot_ref: String,
    pub raw_sha256: String,
    pub normalized_sha256: String,
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    Active,
    Cancelled,
    Expired,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFreshness {
    ObservedCurrent,
    Changed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractContext {
    pub source_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveContractContext {
    pub projection_ids: Vec<String>,
    pub boundary: ContractContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyFault {
    AfterSchemaUpgrade,
    AfterOriginal,
    AfterIntent,
    AfterGoal,
    AfterPlan,
    AfterReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionFile {
    pub id: String,
    pub relative_path: String,
    pub sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContractPreview {
    pub schema_version: u32,
    pub operation: String,
    pub repository: String,
    pub base_commit: String,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_digest: String,
    pub goal: ProjectionFile,
    pub plan: ProjectionFile,
    pub preview_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ApplyIntent {
    schema_version: u32,
    preview: ContractPreview,
    original_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApplyReceipt {
    pub schema_version: u32,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_digest: String,
    pub preview_digest: String,
    pub goal_id: String,
    pub plan_id: String,
    pub outcome: ApplyOutcome,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyOutcome {
    Applied,
    Recovered,
    Unchanged,
}

pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, BelayError> {
    let value = serde_json::to_value(value).map_err(json_error)?;
    let sorted = sort_json(value);
    let mut bytes = serde_json::to_vec(&sorted).map_err(json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<_> = object.into_iter().collect();
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                keys.into_iter()
                    .map(|(k, v)| (k, sort_json(v)))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        other => other,
    }
}

pub fn load_contract(path: &Path) -> Result<Contract, BelayError> {
    let bytes = fs::read(path).map_err(|e| BelayError::io("read Contract", path, e))?;
    let contract: Contract =
        serde_json::from_slice(&bytes).map_err(|e| BelayError::Validation {
            message: format!("invalid Contract JSON: {e}"),
        })?;
    validate(&contract)?;
    Ok(contract)
}

pub fn validate(contract: &Contract) -> Result<(), BelayError> {
    require(
        contract.schema_version == 1,
        "schema_version must be integer 1",
    )?;
    require(
        contract.compiler_version == COMPILER_VERSION,
        "unsupported compiler_version",
    )?;
    require(
        valid_contract_id(&contract.contract_id),
        "contract_id has invalid format",
    )?;
    require(
        contract.revision >= 1 && contract.input_revision >= 1,
        "revision fields must be at least 1",
    )?;
    timestamp(&contract.issued_at, "issued_at")?;
    if let Some(expires) = &contract.expires_at {
        require(
            timestamp(expires, "expires_at")? > timestamp(&contract.issued_at, "issued_at")?,
            "expires_at must be later than issued_at",
        )?;
    }
    nonempty(&contract.outcome, "outcome")?;
    nonempty(&contract.input_id, "input_id")?;
    nonempty(&contract.confirmation_ref, "confirmation_ref")?;
    require(
        !contract.requirements.is_empty(),
        "requirements must not be empty",
    )?;
    require(
        !contract.acceptance_criteria.is_empty(),
        "acceptance_criteria must not be empty",
    )?;
    validate_strings(&contract.constraints, "constraints", true)?;
    validate_strings(&contract.non_goals, "non_goals", true)?;
    validate_strings(&contract.unknowns, "unknowns", false)?;
    require(
        contract.unknowns.is_empty(),
        "unresolved Contract cannot be applied",
    )?;
    validate_strings(&contract.assumptions, "assumptions", false)?;
    validate_strings(&contract.context_refs, "context_refs", false)?;
    validate_strings(&contract.verification, "verification", true)?;
    validate_strings(&contract.stop_conditions, "stop_conditions", true)?;
    nonempty(&contract.delegation.actor, "delegation.actor")?;
    nonempty(&contract.delegation.scope, "delegation.scope")?;
    validate_strings(
        &contract.delegation.allowed_operations,
        "delegation.allowed_operations",
        true,
    )?;
    nonempty(&contract.target.repository, "target.repository")?;
    require(
        is_lower_hex(&contract.target.base_commit, 40),
        "target.base_commit must be 40 lowercase hex characters",
    )?;
    validate_sources(contract)?;
    let mut without = serde_json::to_value(contract).map_err(json_error)?;
    without
        .as_object_mut()
        .expect("contract object")
        .remove("contract_digest");
    let expected = digest(&canonical_bytes(&without)?);
    require(
        contract.contract_digest == expected,
        "contract_digest does not match canonical payload",
    )
}

fn validate_sources(contract: &Contract) -> Result<(), BelayError> {
    nonempty(&contract.source_bundle.bundle_id, "source_bundle.bundle_id")?;
    if let Some(value) = &contract.source_bundle.acquisition_digest {
        require(
            valid_sha(value),
            "source_bundle.acquisition_digest must be a sha256 digest",
        )?;
    }
    require(
        !contract.source_bundle.sources.is_empty(),
        "source_bundle.sources must not be empty",
    )?;
    let mut ids = BTreeSet::new();
    for source in &contract.source_bundle.sources {
        nonempty(&source.id, "source.id")?;
        nonempty(&source.url, "source.url")?;
        nonempty(&source.raw_snapshot_ref, "source.raw_snapshot_ref")?;
        timestamp(&source.retrieved_at, "source.retrieved_at")?;
        if let Some(edited) = &source.edited_at {
            timestamp(edited, "source.edited_at")?;
        }
        require(source.complete, "incomplete source cannot be applied")?;
        require(ids.insert(source.id.clone()), "duplicate source id")?;
        require(
            valid_sha(&source.raw_sha256) && valid_sha(&source.normalized_sha256),
            "source hashes must be sha256 digests",
        )?;
    }
    for item in contract
        .requirements
        .iter()
        .chain(&contract.acceptance_criteria)
    {
        require(
            valid_item_id(&item.id),
            "sourced item id has invalid format",
        )?;
        nonempty(&item.text, "sourced item text")?;
        validate_strings(&item.source_refs, "source_refs", true)?;
        require(
            item.source_refs.iter().all(|id| ids.contains(id)),
            "sourced item references a missing source",
        )?;
    }
    unique_item_ids(&contract.requirements)?;
    unique_item_ids(&contract.acceptance_criteria)?;
    require(
        contract.context_refs.iter().all(|id| ids.contains(id)),
        "context_refs contains a missing source",
    )?;
    let mut bundle = serde_json::to_value(&contract.source_bundle).map_err(json_error)?;
    bundle
        .as_object_mut()
        .expect("bundle object")
        .remove("bundle_digest");
    require(
        contract.source_bundle.bundle_digest == digest(&canonical_bytes(&bundle)?),
        "source_bundle.bundle_digest does not match canonical manifest",
    )
}

pub fn preview(
    repository: &Repository,
    contract: &Contract,
    freshness: SourceFreshness,
) -> Result<ContractPreview, BelayError> {
    validate_preconditions(repository, contract, freshness)?;
    let repository_id = repository_identity(&repository.root)?;
    let (goal, plan) = projections(contract)?;
    let mut preview = ContractPreview {
        schema_version: 1,
        operation: "contract-apply".into(),
        repository: repository_id,
        base_commit: contract.target.base_commit.clone(),
        contract_id: contract.contract_id.clone(),
        contract_revision: contract.revision,
        contract_digest: contract.contract_digest.clone(),
        goal,
        plan,
        preview_digest: String::new(),
    };
    preview.preview_digest = digest(&canonical_bytes(&preview)?);
    Ok(preview)
}

pub fn apply(
    repository: &Repository,
    contract: &Contract,
    approved: &str,
    freshness: SourceFreshness,
    legacy_writers_quiesced: bool,
) -> Result<ApplyReceipt, BelayError> {
    apply_with_fault(
        repository,
        contract,
        approved,
        freshness,
        legacy_writers_quiesced,
        None,
    )
}

pub fn apply_with_fault(
    repository: &Repository,
    contract: &Contract,
    approved: &str,
    freshness: SourceFreshness,
    legacy_writers_quiesced: bool,
    fault: Option<ApplyFault>,
) -> Result<ApplyReceipt, BelayError> {
    let _guard = crate::lifecycle::writer_lock(repository)?;
    let planned = preview(repository, contract, freshness)?;
    require(
        planned.preview_digest == approved,
        "approved preview digest is stale or does not match",
    )?;
    let dir = state_dir(repository, contract);
    reject_identity_collision(repository, contract)?;
    let had_original = managed_exists(repository, &dir.join("original.json"))?;
    let had_intent = managed_exists(repository, &dir.join("intent.json"))?;
    let goal_path = repository.belay_dir.join(&planned.goal.relative_path);
    let plan_path = repository.belay_dir.join(&planned.plan.relative_path);
    if !had_intent
        && (managed_exists(repository, &goal_path)?
            || managed_exists(repository, &plan_path)?
            || managed_exists(repository, &dir.join("receipt.json"))?)
    {
        return conflict("orphan Contract projection or receipt exists without durable intent");
    }
    crate::lifecycle::ensure_contract_v3(repository, legacy_writers_quiesced)?;
    inject(fault, ApplyFault::AfterSchemaUpgrade)?;
    crate::lifecycle::ensure_directory(
        repository,
        dir.strip_prefix(&repository.belay_dir)
            .expect("managed state"),
    )?;
    let original = canonical_bytes(contract)?;
    let original_path = dir.join("original.json");
    ensure_exact_file(repository, &original_path, &original, "Contract original")?;
    inject(fault, ApplyFault::AfterOriginal)?;
    let intent = ApplyIntent {
        schema_version: 1,
        preview: planned.clone(),
        original_sha256: digest(&original),
    };
    let intent_bytes = canonical_bytes(&intent)?;
    ensure_exact_file(
        repository,
        &dir.join("intent.json"),
        &intent_bytes,
        "apply intent",
    )?;
    inject(fault, ApplyFault::AfterIntent)?;
    ensure_projection(repository, &planned.goal, false)?;
    inject(fault, ApplyFault::AfterGoal)?;
    ensure_projection(repository, &planned.plan, true)?;
    inject(fault, ApplyFault::AfterPlan)?;
    index_projection(repository, &planned.goal)?;
    index_projection(repository, &planned.plan)?;
    let receipt_path = dir.join("receipt.json");
    let existed = managed_exists(repository, &receipt_path)?;
    let recovered = !existed && (had_original || had_intent);
    let durable_receipt = ApplyReceipt {
        schema_version: 1,
        contract_id: contract.contract_id.clone(),
        contract_revision: contract.revision,
        contract_digest: contract.contract_digest.clone(),
        preview_digest: planned.preview_digest.clone(),
        goal_id: planned.goal.id.clone(),
        plan_id: planned.plan.id.clone(),
        outcome: ApplyOutcome::Applied,
    };
    if existed {
        validate_completed_state(repository, contract, &planned)?;
    } else {
        let bytes = canonical_bytes(&durable_receipt)?;
        ensure_exact_file(repository, &receipt_path, &bytes, "apply receipt")?;
    }
    inject(fault, ApplyFault::AfterReceipt)?;
    Ok(ApplyReceipt {
        outcome: if existed {
            ApplyOutcome::Unchanged
        } else if recovered {
            ApplyOutcome::Recovered
        } else {
            ApplyOutcome::Applied
        },
        ..durable_receipt
    })
}

pub fn show(
    repository: &Repository,
    contract_id: &str,
    revision: u32,
) -> Result<ApplyReceipt, BelayError> {
    let path = repository
        .belay_dir
        .join("state/contracts")
        .join(safe_component(contract_id)?)
        .join(revision.to_string())
        .join("receipt.json");
    let original_path = path.parent().expect("receipt parent").join("original.json");
    let original: Contract = serde_json::from_str(&crate::store::read_loose_managed_file(
        repository,
        &original_path,
    )?)
    .map_err(json_error)?;
    validate(&original)?;
    require(
        original.contract_id == contract_id && original.revision == revision,
        "stored Contract identity does not match requested identity",
    )?;
    let planned = stored_preview(repository, &original)?;
    validate_completed_state(repository, &original, &planned)
}

/// Load and verify the immutable Contract original and its completed receipt.
/// Consumers receive no path-based escape hatch and therefore cannot replace
/// either artifact with caller-supplied data.
pub(crate) fn completed_binding(
    repository: &Repository,
    contract_id: &str,
    revision: u32,
) -> Result<(Contract, ApplyReceipt), BelayError> {
    let path = repository
        .belay_dir
        .join("state/contracts")
        .join(safe_component(contract_id)?)
        .join(revision.to_string())
        .join("original.json");
    let original: Contract =
        serde_json::from_str(&crate::store::read_loose_managed_file(repository, &path)?)
            .map_err(json_error)?;
    validate(&original)?;
    require(
        original.contract_id == contract_id && original.revision == revision,
        "stored Contract identity does not match requested identity",
    )?;
    let planned = stored_preview(repository, &original)?;
    let receipt = validate_completed_state(repository, &original, &planned)?;
    Ok((original, receipt))
}

/// Return the verified immutable boundary for a Contract-derived entry.
pub fn context_for_entry(
    repository: &Repository,
    entry: &Entry,
) -> Result<Option<ContractContext>, BelayError> {
    let Some((original, planned)) = projection_binding(repository, &entry.display_id)? else {
        if entry.metadata.contains_key("contract_id")
            || entry.metadata.contains_key("contract_revision")
            || entry.metadata.contains_key("contract_digest")
        {
            return validation(format!(
                "Contract projection {} has provenance but no durable intent binding",
                entry.display_id
            ));
        }
        return Ok(None);
    };
    validate_bound_entry(entry, &planned)?;
    Ok(Some(ContractContext {
        source_id: format!("contract:{}:r{}", original.contract_id, original.revision),
        content: render_context_boundary(&original)?,
    }))
}

pub fn validate_entry_projection(
    repository: &Repository,
    entry: &Entry,
) -> Result<bool, BelayError> {
    let Some((_original, planned)) = projection_binding(repository, &entry.display_id)? else {
        if entry.metadata.contains_key("contract_id")
            || entry.metadata.contains_key("contract_revision")
            || entry.metadata.contains_key("contract_digest")
        {
            return validation(format!(
                "Contract projection {} has provenance but no durable intent binding",
                entry.display_id
            ));
        }
        return Ok(false);
    };
    validate_bound_entry(entry, &planned)?;
    Ok(true)
}

/// Return completed active Contract boundaries independently of mutable Goal
/// status or Plan task rows.
pub fn active_contexts(repository: &Repository) -> Result<Vec<ActiveContractContext>, BelayError> {
    let mut contexts = Vec::new();
    for (original, planned) in stored_bindings(repository)? {
        if original.lifecycle != Lifecycle::Active {
            continue;
        }
        contexts.push(ActiveContractContext {
            projection_ids: vec![planned.goal.id, planned.plan.id],
            boundary: ContractContext {
                source_id: format!("contract:{}:r{}", original.contract_id, original.revision),
                content: render_context_boundary(&original)?,
            },
        });
    }
    Ok(contexts)
}

fn projection_binding(
    repository: &Repository,
    display_id: &str,
) -> Result<Option<(Contract, ContractPreview)>, BelayError> {
    Ok(stored_bindings(repository)?
        .into_iter()
        .find(|(_, planned)| planned.goal.id == display_id || planned.plan.id == display_id))
}

fn stored_bindings(
    repository: &Repository,
) -> Result<Vec<(Contract, ContractPreview)>, BelayError> {
    let root = repository.belay_dir.join("state/contracts");
    match fs::symlink_metadata(&root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(BelayError::io("inspect Contract state", &root, error)),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return validation("managed Contract state must be a real directory");
        }
        Ok(_) => {}
    }
    let mut bindings = Vec::new();
    for contract_dir in real_directories(&root)? {
        for revision_dir in real_directories(&contract_dir)? {
            let intent_path = revision_dir.join("intent.json");
            let raw = match crate::store::read_loose_managed_file(repository, &intent_path) {
                Ok(raw) => raw,
                Err(BelayError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    if managed_exists(repository, &revision_dir.join("receipt.json"))? {
                        return validation(format!(
                            "Contract state {} has a receipt but is missing intent.json",
                            revision_dir.display()
                        ));
                    }
                    let original_path = revision_dir.join("original.json");
                    if managed_exists(repository, &original_path)? {
                        let original: Contract = serde_json::from_str(
                            &crate::store::read_loose_managed_file(repository, &original_path)?,
                        )
                        .map_err(json_error)?;
                        validate(&original)?;
                        let planned = stored_preview(repository, &original)?;
                        let goal = repository.belay_dir.join(&planned.goal.relative_path);
                        let plan = repository.belay_dir.join(&planned.plan.relative_path);
                        if managed_exists(repository, &goal)? || managed_exists(repository, &plan)?
                        {
                            return validation(format!(
                                "Contract state {} has projections but is missing intent.json",
                                revision_dir.display()
                            ));
                        }
                    }
                    continue;
                }
                Err(error) => return Err(error),
            };
            let intent: ApplyIntent = serde_json::from_str(&raw).map_err(json_error)?;
            let original_path = revision_dir.join("original.json");
            let original: Contract = serde_json::from_str(&crate::store::read_loose_managed_file(
                repository,
                &original_path,
            )?)
            .map_err(json_error)?;
            validate(&original)?;
            let planned = stored_preview(repository, &original)?;
            let expected_intent = ApplyIntent {
                schema_version: 1,
                preview: planned.clone(),
                original_sha256: digest(&canonical_bytes(&original)?),
            };
            require(
                canonical_bytes(&intent)? == canonical_bytes(&expected_intent)?,
                "stored Contract intent does not match original and projections",
            )?;
            show(repository, &original.contract_id, original.revision)?;
            bindings.push((original, planned));
        }
    }
    Ok(bindings)
}

fn real_directories(parent: &Path) -> Result<Vec<PathBuf>, BelayError> {
    let mut directories = Vec::new();
    for item in fs::read_dir(parent)
        .map_err(|error| BelayError::io("read Contract state", parent, error))?
    {
        let item = item.map_err(|error| BelayError::io("read Contract state", parent, error))?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| BelayError::io("inspect Contract state path", &path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return validation(format!(
                "managed Contract state path {} must be a real directory",
                path.display()
            ));
        }
        directories.push(path);
    }
    directories.sort();
    Ok(directories)
}

fn validate_bound_entry(entry: &Entry, planned: &ContractPreview) -> Result<(), BelayError> {
    let (expected, editable_tasks) = if entry.display_id == planned.goal.id {
        (&planned.goal, false)
    } else if entry.display_id == planned.plan.id {
        (&planned.plan, true)
    } else {
        return validation(format!(
            "Contract projection {} is not named by its durable intent",
            entry.display_id
        ));
    };
    validate_projection_entry(entry, expected, editable_tasks)
}

pub fn render_context_boundary(original: &Contract) -> Result<String, BelayError> {
    validate(original)?;
    let lifecycle = match original.lifecycle {
        Lifecycle::Active => {
            if original
                .expires_at
                .as_deref()
                .map(|value| timestamp(value, "expires_at").map(|at| Utc::now() >= at))
                .transpose()?
                .unwrap_or(false)
            {
                "expired by expires_at; mutation and dispatch blocked"
            } else {
                "active in retained original"
            }
        }
        Lifecycle::Cancelled => "cancelled; mutation and dispatch blocked",
        Lifecycle::Expired => "expired; mutation and dispatch blocked",
        Lifecycle::Superseded => "superseded; mutation and dispatch blocked",
    };
    let acceptance = original
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "- {} -> SC-{:03}: {} [sources: {}]",
                item.id,
                index + 1,
                item.text,
                item.source_refs.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let requirements = original
        .requirements
        .iter()
        .map(|item| {
            format!(
                "- {}: {} [sources: {}]",
                item.id,
                item.text,
                item.source_refs.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let sources = original
        .source_bundle
        .sources
        .iter()
        .map(|source| {
            format!(
                "- {}: {} retrieved={} complete={} raw={} normalized={}",
                source.id,
                source.url,
                source.retrieved_at,
                source.complete,
                source.raw_sha256,
                source.normalized_sha256
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "Contract: {} revision {}\nOutcome: {}\nLifecycle: {}\nSource freshness: Unknown (only the retained local original was observed; current remote source state was not checked)\nContract digest: {}\nSource bundle digest: {}\nInput: {} revision {}\nConfirmation: {}\nTarget repository: {}\nTarget base commit: {}\n\nRequirements:\n{}\n\nAcceptance to success provenance:\n{}\n\nConstraints:\n{}\n\nNon-goals:\n{}\n\nAssumptions:\n{}\n\nContext references:\n{}\n\nDelegation:\n- actor: {}\n- scope: {}\n- allowed operations: {}\n\nVerification:\n{}\n\nStop conditions:\n{}\n\nSources:\n{}",
        original.contract_id,
        original.revision,
        original.outcome,
        lifecycle,
        original.contract_digest,
        original.source_bundle.bundle_digest,
        original.input_id,
        original.input_revision,
        original.confirmation_ref,
        original.target.repository,
        original.target.base_commit,
        requirements,
        acceptance,
        bullets(&original.constraints),
        bullets(&original.non_goals),
        bullets(&original.assumptions),
        bullets(&original.context_refs),
        original.delegation.actor,
        original.delegation.scope,
        original.delegation.allowed_operations.join(", "),
        bullets(&original.verification),
        bullets(&original.stop_conditions),
        sources,
    ))
}

fn validate_preconditions(
    repository: &Repository,
    contract: &Contract,
    freshness: SourceFreshness,
) -> Result<(), BelayError> {
    validate(contract)?;
    require(
        contract.lifecycle == Lifecycle::Active,
        "Contract lifecycle blocks mutation",
    )?;
    require(
        freshness == SourceFreshness::ObservedCurrent,
        "source freshness is not observed-current; mutation is blocked",
    )?;
    if let Some(expires) = &contract.expires_at {
        require(
            Utc::now() < timestamp(expires, "expires_at")?,
            "Contract is expired",
        )?;
    }
    let identity = repository_identity(&repository.root)?;
    require(
        identity == contract.target.repository,
        format!(
            "target repository mismatch: expected {}, found {identity}",
            contract.target.repository
        ),
    )?;
    let head =
        crate::git_provenance::read_head(&repository.root).map_err(|e| BelayError::Validation {
            message: format!("cannot establish target HEAD: {e}"),
        })?;
    require(
        head == contract.target.base_commit,
        format!(
            "target base commit mismatch: expected {}, found {head}",
            contract.target.base_commit
        ),
    )
}

fn repository_identity(root: &Path) -> Result<String, BelayError> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(root)
        .output()
        .map_err(|e| BelayError::io("read repository identity", root, e))?;
    require(
        output.status.success(),
        "repository identity is unavailable; configure remote.origin.url",
    )?;
    let raw = String::from_utf8(output.stdout).map_err(|_| BelayError::Validation {
        message: "repository identity is not UTF-8".into(),
    })?;
    normalize_repository(raw.trim()).ok_or_else(|| BelayError::Validation {
        message: "remote.origin.url cannot be normalized to a repository identity".into(),
    })
}

fn normalize_repository(value: &str) -> Option<String> {
    let value = value.trim_end_matches('/').trim_end_matches(".git");
    if let Some(rest) = value.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("{host}/{path}"));
    }
    for prefix in ["https://", "http://", "ssh://git@", "ssh://"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return Some(rest.replace(':', "/"));
        }
    }
    (!value.is_empty()).then(|| value.to_owned())
}

fn projections(contract: &Contract) -> Result<(ProjectionFile, ProjectionFile), BelayError> {
    let sequence =
        (u16::from_str_radix(&contract.contract_digest[7..11], 16).unwrap_or(0) % 999) + 1;
    let stamp = contract
        .issued_at
        .replace(['-', ':'], "")
        .trim_end_matches('Z')
        .to_owned();
    let slug = format!(
        "{}-r{}-{}",
        slug(&contract.outcome).chars().take(30).collect::<String>(),
        contract.revision,
        &contract.contract_digest[7..15]
    );
    let goal_id = format!("GOAL-{stamp}-{sequence:03}-{slug}");
    let plan_id = format!("PLN-{stamp}-{sequence:03}-{slug}");
    let projection_digest = digest(&canonical_bytes(&serde_json::json!({
        "contract_id": contract.contract_id, "contract_revision": contract.revision,
        "goal_id": goal_id, "plan_id": plan_id, "acceptance_to_success": contract.acceptance_criteria.iter().enumerate()
            .map(|(index, item)| serde_json::json!({"acceptance_criterion":item.id,"success_criterion":format!("SC-{:03}",index+1),"source_refs":item.source_refs})).collect::<Vec<_>>()
    }))?);
    let metadata = provenance(contract, &projection_digest);
    let goal_body = goal_body(contract);
    let plan_body = plan_body(contract, &goal_id);
    let goal = Entry {
        display_id: goal_id.clone(),
        entry_type: EntryType::Goal,
        title: contract.outcome.clone(),
        status: EntryStatus::Draft,
        created_at: contract.issued_at.clone(),
        updated_at: contract.issued_at.clone(),
        revision: 1,
        tags: vec![],
        links: vec![],
        metadata: metadata.clone(),
        body: goal_body,
    }
    .normalized()?;
    let plan = Entry {
        display_id: plan_id.clone(),
        entry_type: EntryType::Plan,
        title: contract.outcome.clone(),
        status: EntryStatus::Draft,
        created_at: contract.issued_at.clone(),
        updated_at: contract.issued_at.clone(),
        revision: 1,
        tags: vec![],
        links: vec![crate::entry::EntryLink {
            relation: crate::entry::LinkRelation::Implements,
            id: goal_id.clone(),
            metadata: BTreeMap::new(),
        }],
        metadata,
        body: plan_body,
    }
    .normalized()?;
    let goal_content = crate::markdown::render(&goal)?;
    let plan_content = crate::markdown::render(&plan)?;
    Ok((
        projection_file(&goal, goal_content),
        projection_file(&plan, plan_content),
    ))
}

fn stored_preview(
    repository: &Repository,
    contract: &Contract,
) -> Result<ContractPreview, BelayError> {
    let (goal, plan) = projections(contract)?;
    let mut preview = ContractPreview {
        schema_version: 1,
        operation: "contract-apply".into(),
        repository: repository_identity(&repository.root)?,
        base_commit: contract.target.base_commit.clone(),
        contract_id: contract.contract_id.clone(),
        contract_revision: contract.revision,
        contract_digest: contract.contract_digest.clone(),
        goal,
        plan,
        preview_digest: String::new(),
    };
    preview.preview_digest = digest(&canonical_bytes(&preview)?);
    Ok(preview)
}

fn validate_completed_state(
    repository: &Repository,
    contract: &Contract,
    planned: &ContractPreview,
) -> Result<ApplyReceipt, BelayError> {
    let dir = state_dir(repository, contract);
    let original_bytes = canonical_bytes(contract)?;
    require(
        crate::store::read_loose_managed_file(repository, &dir.join("original.json"))?.as_bytes()
            == original_bytes,
        "stored Contract original does not match canonical intended Contract",
    )?;
    let intent = ApplyIntent {
        schema_version: 1,
        preview: planned.clone(),
        original_sha256: digest(&original_bytes),
    };
    require(
        crate::store::read_loose_managed_file(repository, &dir.join("intent.json"))?.as_bytes()
            == canonical_bytes(&intent)?,
        "stored Contract intent does not match original and projections",
    )?;
    validate_projection(repository, &planned.goal, false)?;
    validate_projection(repository, &planned.plan, true)?;
    let expected = ApplyReceipt {
        schema_version: 1,
        contract_id: contract.contract_id.clone(),
        contract_revision: contract.revision,
        contract_digest: contract.contract_digest.clone(),
        preview_digest: planned.preview_digest.clone(),
        goal_id: planned.goal.id.clone(),
        plan_id: planned.plan.id.clone(),
        outcome: ApplyOutcome::Applied,
    };
    let actual = crate::store::read_loose_managed_file(repository, &dir.join("receipt.json"))?;
    require(
        actual.as_bytes() == canonical_bytes(&expected)?,
        "completed receipt does not match canonical intended apply",
    )?;
    Ok(expected)
}

fn provenance(c: &Contract, projection_digest: &str) -> BTreeMap<String, MetadataValue> {
    BTreeMap::from([
        (
            "contract_id".into(),
            MetadataValue::String(c.contract_id.clone()),
        ),
        (
            "contract_revision".into(),
            MetadataValue::Integer(i64::from(c.revision)),
        ),
        (
            "contract_digest".into(),
            MetadataValue::String(c.contract_digest.clone()),
        ),
        (
            "source_bundle_digest".into(),
            MetadataValue::String(c.source_bundle.bundle_digest.clone()),
        ),
        (
            "projection_digest".into(),
            MetadataValue::String(projection_digest.to_owned()),
        ),
    ])
}
fn goal_body(c: &Contract) -> String {
    let sc = c
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, a)| format!("- [SC-{:03}] {}\n", i + 1, a.text))
        .collect::<String>();
    let map = c
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "| {} | SC-{:03} | {} |\n",
                a.id,
                i + 1,
                a.source_refs.join(", ")
            )
        })
        .collect::<String>();
    format!(
        "## Summary\n- {}\n\n## Success Criteria\n{}\n## Acceptance to Success Map\n| Acceptance criterion | Success criterion | Source refs |\n| --- | --- | --- |\n{}\n## Constraints\n{}\n\n## Non-goals\n{}\n\n## Verification\n{}\n\n## Risks\n{}\n",
        c.outcome,
        sc,
        map,
        bullets(&c.constraints),
        bullets(&c.non_goals),
        bullets(&c.verification),
        bullets(&c.stop_conditions)
    )
}
fn plan_body(c: &Contract, goal_id: &str) -> String {
    let requirements = c
        .requirements
        .iter()
        .map(|item| {
            format!(
                "[{}] {} (sources: {})",
                item.id,
                item.text,
                item.source_refs.join(", ")
            )
        })
        .collect::<Vec<_>>();
    let rows = c
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "| T-{:03} | {}#sc-{:03} | {} | {} | not-started | {} |",
                i + 1,
                goal_id,
                i + 1,
                a.text,
                c.delegation.actor,
                c.verification.join("; ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let sections=c.acceptance_criteria.iter().enumerate().map(|(i,a)|format!("## T-{:03}\n- Objective: {}\n- Scope: {}\n- Steps: Perform only {}.\n- Acceptance: {}\n- Verification: {}\n- Stop: {}\n",i+1,a.text,c.delegation.scope,c.delegation.allowed_operations.join(", "),a.text,c.verification.join("; "),c.stop_conditions.join("; "))).collect::<Vec<_>>().join("\n");
    let sources = c
        .source_bundle
        .sources
        .iter()
        .map(|source| {
            format!(
                "- {}: {} (retrieved {}; raw {}; normalized {}; complete={})",
                source.id,
                source.url,
                source.retrieved_at,
                source.raw_sha256,
                source.normalized_sha256,
                source.complete
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let acceptance_map = c
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "| {} | {}#sc-{:03} | {} |",
                a.id,
                goal_id,
                i + 1,
                a.source_refs.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "## Intent Brief\n\n### Problem\n{}\n\n### Desired Outcome\n- {}\n\n### Success Signals\n{}\n\n### Constraints\n{}\n\n### Non-goals\n{}\n\n### Assumptions\n{}\n\n## Contract Boundaries\n- Delegated actor: {}\n- Delegated scope: {}\n- Allowed operations: {}\n- Verification: {}\n- Stop conditions: {}\n- Context refs: {}\n- Target repository: {}\n- Target base commit: {}\n- Contract: {} revision {} ({})\n- Source bundle: {}\n\n## Contract Sources\n{}\n\n## Acceptance Provenance\n| Acceptance criterion | Success criterion | Source refs |\n| --- | --- | --- |\n{}\n\n{}\n\n## Delivery Map\n| ID | Goal item | Outcome / Task | Actor | State | Verification / Evidence |\n| --- | --- | --- | --- | --- | --- |\n{}\n\n{}",
        bullets(&requirements),
        c.outcome,
        bullets(
            &c.acceptance_criteria
                .iter()
                .map(|x| x.text.clone())
                .collect::<Vec<_>>()
        ),
        bullets(&c.constraints),
        bullets(&c.non_goals),
        bullets(&c.assumptions),
        c.delegation.actor,
        c.delegation.scope,
        c.delegation.allowed_operations.join(", "),
        c.verification.join("; "),
        c.stop_conditions.join("; "),
        c.context_refs.join(", "),
        c.target.repository,
        c.target.base_commit,
        c.contract_id,
        c.revision,
        c.contract_digest,
        c.source_bundle.bundle_digest,
        sources,
        acceptance_map,
        EDITABLE_TASKS_MARKER,
        rows,
        sections
    )
}
fn bullets(v: &[String]) -> String {
    v.iter()
        .map(|x| format!("- {x}"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn projection_file(entry: &Entry, content: String) -> ProjectionFile {
    ProjectionFile {
        id: entry.display_id.clone(),
        relative_path: format!(
            "entries/{}/{}.md",
            entry.entry_type.directory(),
            entry.display_id
        ),
        sha256: digest(content.as_bytes()),
        content,
    }
}

fn state_dir(repository: &Repository, c: &Contract) -> PathBuf {
    repository
        .belay_dir
        .join("state/contracts")
        .join(safe_component(&c.contract_id).expect("validated id"))
        .join(c.revision.to_string())
}
fn ensure_projection(
    repository: &Repository,
    p: &ProjectionFile,
    editable_tasks: bool,
) -> Result<(), BelayError> {
    let path = repository.belay_dir.join(&p.relative_path);
    match crate::store::read_loose_managed_file(repository, &path) {
        Ok(_) => validate_projection(repository, p, editable_tasks),
        Err(BelayError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            ensure_exact_file(
                repository,
                &path,
                p.content.as_bytes(),
                "Contract projection",
            )
        }
        Err(error) => Err(error),
    }
}

fn validate_projection(
    repository: &Repository,
    expected: &ProjectionFile,
    editable_tasks: bool,
) -> Result<(), BelayError> {
    let path = repository.belay_dir.join(&expected.relative_path);
    let actual = crate::store::read_loose_managed_file(repository, &path)?;
    if !editable_tasks {
        return require(
            actual.as_bytes() == expected.content.as_bytes(),
            format!(
                "stored Contract projection {} contains unexpected bytes",
                expected.id
            ),
        );
    }
    let actual_entry = crate::markdown::parse(&actual)?;
    validate_projection_entry(&actual_entry, expected, editable_tasks)
}

fn validate_projection_entry(
    actual_entry: &Entry,
    expected: &ProjectionFile,
    editable_tasks: bool,
) -> Result<(), BelayError> {
    if !editable_tasks {
        let actual = crate::markdown::render(actual_entry)?;
        return require(
            actual.as_bytes() == expected.content.as_bytes(),
            format!(
                "stored Contract projection {} contains unexpected bytes",
                expected.id
            ),
        );
    }
    let expected_entry = crate::markdown::parse(&expected.content)?;
    require(
        actual_entry.display_id == expected_entry.display_id
            && actual_entry.entry_type == expected_entry.entry_type
            && actual_entry.title == expected_entry.title
            && actual_entry.created_at == expected_entry.created_at
            && actual_entry.tags == expected_entry.tags
            && actual_entry.links == expected_entry.links
            && actual_entry.metadata == expected_entry.metadata,
        format!(
            "stored Contract projection {} protected frontmatter drifted",
            expected.id
        ),
    )?;
    let expected_prefix = protected_prefix(&expected_entry.body)?;
    let actual_prefix = protected_prefix(&actual_entry.body)?;
    require(
        actual_prefix == expected_prefix,
        format!(
            "stored Contract projection {} protected Contract region drifted",
            expected.id
        ),
    )
}

fn protected_prefix(body: &str) -> Result<&str, BelayError> {
    let mut matches = body.match_indices(EDITABLE_TASKS_MARKER);
    let Some((offset, _)) = matches.next() else {
        return conflict("Contract Plan is missing the editable task delimiter");
    };
    require(
        matches.next().is_none(),
        "Contract Plan contains more than one editable task delimiter",
    )?;
    Ok(&body[..offset + EDITABLE_TASKS_MARKER.len()])
}

fn index_projection(
    repository: &Repository,
    projection: &ProjectionFile,
) -> Result<(), BelayError> {
    let id = &projection.id;
    let path = repository.belay_dir.join(&projection.relative_path);
    let raw = crate::store::read_loose_managed_file(repository, &path)?;
    let entry = crate::markdown::parse(&raw)?;
    let relative =
        path.strip_prefix(&repository.belay_dir)
            .map_err(|_| BelayError::Validation {
                message: "projection path escapes .belay".into(),
            })?;
    let source_path = relative.to_str().ok_or_else(|| BelayError::Validation {
        message: "projection path is not UTF-8".into(),
    })?;
    let database_path = repository.database_path();
    if !database_path.exists() {
        crate::database::initialize(&database_path)?;
    }
    let mut connection = crate::database::open(&database_path)?;
    let transaction = crate::entry::begin_immediate(&mut connection, &database_path)?;
    let existing: Option<i64> = transaction
        .query_row(
            "SELECT id FROM entries WHERE display_id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| BelayError::sqlite(&database_path, error))?;
    if let Some(internal_id) = existing {
        let indexed = crate::store::load_entry(&transaction, &database_path, internal_id)?;
        require(
            indexed == entry,
            format!("indexed Contract projection {id} differs from durable Markdown"),
        )?;
    } else {
        let internal_id =
            crate::store::insert_entry(&transaction, &database_path, &entry, source_path)?;
        crate::store::replace_links(&transaction, &database_path, internal_id, &entry.links)?;
        let hash = crate::markdown::content_hash(&entry)?;
        crate::store::upsert_sync_state(
            &transaction,
            &database_path,
            internal_id,
            source_path,
            &hash,
            &entry.updated_at,
        )?;
    }
    transaction.commit()
}
fn ensure_exact_file(
    repository: &Repository,
    path: &Path,
    bytes: &[u8],
    kind: &str,
) -> Result<(), BelayError> {
    match crate::store::read_loose_managed_file(repository, path) {
        Ok(existing) if existing.as_bytes() == bytes => Ok(()),
        Ok(_) => conflict(format!(
            "{kind} at {} contains unexpected bytes",
            path.display()
        )),
        Err(BelayError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                let relative = parent.strip_prefix(&repository.belay_dir).map_err(|_| {
                    BelayError::Validation {
                        message: "managed path escapes .belay".into(),
                    }
                })?;
                crate::lifecycle::ensure_directory(repository, relative)?;
            }
            crate::store::write_new_file(repository, path, bytes)
        }
        Err(error) => Err(error),
    }
}
fn reject_identity_collision(repository: &Repository, c: &Contract) -> Result<(), BelayError> {
    let path = state_dir(repository, c).join("original.json");
    if !managed_exists(repository, &path)? {
        return Ok(());
    }
    let raw = crate::store::read_loose_managed_file(repository, &path)?;
    let other: Contract = serde_json::from_str(&raw).map_err(json_error)?;
    if other.contract_id != c.contract_id
        || other.revision != c.revision
        || other.contract_digest != c.contract_digest
    {
        return conflict("same contract id and revision has a different payload");
    }
    Ok(())
}

fn managed_exists(repository: &Repository, path: &Path) -> Result<bool, BelayError> {
    match crate::store::read_loose_managed_file(repository, path) {
        Ok(_) => Ok(true),
        Err(BelayError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
fn inject(actual: Option<ApplyFault>, boundary: ApplyFault) -> Result<(), BelayError> {
    if actual == Some(boundary) {
        return Err(BelayError::Io {
            action: "continue Contract apply after injected interruption",
            path: PathBuf::from("fault-injection"),
            source: std::io::Error::other(format!("interrupted after {boundary:?}")),
        });
    }
    Ok(())
}

fn timestamp(value: &str, field: &str) -> Result<DateTime<Utc>, BelayError> {
    if value.len() != 20 || !value.ends_with('Z') {
        return validation(format!("{field} must be RFC3339 UTC with second precision"));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| BelayError::Validation {
            message: format!("{field} is invalid"),
        })
}
fn valid_contract_id(v: &str) -> bool {
    (3..=64).contains(&v.len())
        && v.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
        && v.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
}
fn valid_item_id(v: &str) -> bool {
    let Some((a, n)) = v.rsplit_once('-') else {
        return false;
    };
    !a.is_empty()
        && a.bytes().all(|b| b.is_ascii_uppercase())
        && n.len() == 3
        && n.bytes().all(|b| b.is_ascii_digit())
}
fn valid_sha(v: &str) -> bool {
    v.strip_prefix("sha256:")
        .is_some_and(|x| is_lower_hex(x, 64))
}
fn is_lower_hex(v: &str, n: usize) -> bool {
    v.len() == n
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_strings(v: &[String], field: &str, required: bool) -> Result<(), BelayError> {
    require(
        !required || !v.is_empty(),
        format!("{field} must not be empty"),
    )?;
    let mut seen = BTreeSet::new();
    for x in v {
        nonempty(x, field)?;
        require(seen.insert(x), format!("{field} contains a duplicate"))?
    }
    Ok(())
}
fn unique_item_ids(v: &[SourcedItem]) -> Result<(), BelayError> {
    let mut s = BTreeSet::new();
    for x in v {
        require(s.insert(&x.id), "duplicate sourced item id")?
    }
    Ok(())
}
fn nonempty(v: &str, f: &str) -> Result<(), BelayError> {
    require(!v.is_empty(), format!("{f} must not be empty"))
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn slug(v: &str) -> String {
    let mut s = v
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    while s.contains("--") {
        s = s.replace("--", "-")
    }
    s = s.trim_matches('-').chars().take(48).collect();
    if s.is_empty() { "contract".into() } else { s }
}
fn safe_component(v: &str) -> Result<String, BelayError> {
    require(valid_contract_id(v), "invalid Contract storage identity")?;
    Ok(v.to_owned())
}
fn require(ok: bool, message: impl Into<String>) -> Result<(), BelayError> {
    if ok { Ok(()) } else { validation(message) }
}
fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}
fn conflict<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Conflict {
        message: message.into(),
    })
}
fn json_error(e: serde_json::Error) -> BelayError {
    BelayError::Validation {
        message: format!("JSON serialization failed: {e}"),
    }
}
