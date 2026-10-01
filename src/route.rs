use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, SecondsFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::coverage;
use crate::entry::{
    EntryLink, EntryStatus, EntryType, LinkRelation, MetadataValue, parse_entry_reference_id,
};
use crate::error::BelayError;
use crate::mutation;
use crate::repository::Repository;
use crate::store::{self, MutationOutcome};

pub const ROUTE_SCHEMA_VERSION: u32 = 1;
const ROUTE_OPERATION_KEY: &str = "route_operation_id";
const ROUTE_RUN_KEY: &str = "route_run_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteManifest {
    pub schema_version: u32,
    pub run_id: String,
    pub primary_seed: String,
    pub included_ids: Vec<String>,
    pub phase: RoutePhase,
    pub input: ArtifactReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<ArtifactReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<ArtifactReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<ArtifactReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<ArtifactReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<ArtifactReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutePhase {
    Started,
    Assessed,
    Proposed,
    Responded,
    Previewed,
    Applying,
    Reconciled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    pub revision: u32,
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteInput {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub primary_seed: String,
    pub included_ids: Vec<String>,
    pub generated_at: String,
    pub entries: Vec<RouteEntry>,
    pub goal_coverage: BTreeMap<String, serde_json::Value>,
    pub evidence_fingerprint: String,
    pub coverage_basis_fingerprint: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub title: String,
    pub status: EntryStatus,
    pub revision: u32,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, MetadataValue>,
    pub links: Vec<EntryLink>,
    pub inbound_links: Vec<EntryLink>,
    pub body: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteAssessment {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub input_fingerprint: String,
    pub outcome: AssessmentOutcome,
    #[serde(default)]
    pub items: Vec<AssessmentItem>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssessmentOutcome {
    Continue,
    Stop,
    InsufficientContext,
    NoSafeRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentItem {
    pub id: String,
    pub classification: AssessmentClassification,
    pub summary: String,
    #[serde(default)]
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssessmentClassification {
    Fact,
    HumanObservation,
    Assumption,
    Hypothesis,
    Unknown,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteProposal {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub input_fingerprint: String,
    pub assessment_hash: String,
    pub outcome: AssessmentOutcome,
    pub summary: String,
    #[serde(default)]
    pub operations: Vec<ProposedOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProposedOperation {
    CreateEntry {
        operation_id: String,
        alias: String,
        #[serde(rename = "type")]
        entry_type: EntryType,
        title: String,
        body: String,
    },
    Link {
        operation_id: String,
        from: String,
        to: String,
        relation: LinkRelation,
    },
    SetStatus {
        operation_id: String,
        target: String,
        status: EntryStatus,
    },
}

impl ProposedOperation {
    fn operation_id(&self) -> &str {
        match self {
            Self::CreateEntry { operation_id, .. }
            | Self::Link { operation_id, .. }
            | Self::SetStatus { operation_id, .. } => operation_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanResponse {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub input_fingerprint: String,
    pub proposal_revision: u32,
    pub proposal_hash: String,
    pub action: HumanAction,
    #[serde(default)]
    pub selected_operation_ids: Vec<String>,
    pub reason: String,
    #[serde(default)]
    pub requested_changes: Vec<String>,
    pub issuer: String,
    pub responded_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HumanAction {
    Accept,
    Revise,
    Reject,
    Defer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationPreview {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub input_fingerprint: String,
    pub proposal_revision: u32,
    pub proposal_hash: String,
    pub response_revision: u32,
    pub response_hash: String,
    pub generated_at: String,
    pub operations: Vec<ProposedOperation>,
    pub entry_preconditions: BTreeMap<String, u32>,
    pub unselected_operation_ids: Vec<String>,
    pub preview_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationResult {
    pub schema_version: u32,
    pub artifact_type: String,
    pub run_id: String,
    pub revision: u32,
    pub preview_hash: String,
    pub input_fingerprint_before: String,
    pub state_fingerprint_after: String,
    pub reconciled_at: String,
    pub operations: Vec<OperationResult>,
    pub unselected_operation_ids: Vec<String>,
    pub aliases: BTreeMap<String, String>,
    pub goal_coverage_after: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationResult {
    pub operation_id: String,
    pub state: OperationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationState {
    Applied,
    Unchanged,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitKind {
    Assessment,
    Proposal,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    Assessment,
    Proposal,
    Response,
}

#[derive(Debug)]
pub struct StartOutcome {
    pub run_id: String,
    pub run_path: PathBuf,
    pub input_path: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug)]
pub struct SubmitOutcome {
    pub artifact_path: PathBuf,
    pub artifact_hash: String,
    pub revision: u32,
}

#[derive(Debug)]
pub struct PreviewOutcome {
    pub preview_path: PathBuf,
    pub preview_hash: String,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingPreview {
    pub run_id: String,
    pub preview_revision: u32,
    pub preview_hash: String,
    pub input_fingerprint: String,
    pub proposal_revision: u32,
    pub proposal_hash: String,
    pub response_revision: u32,
    pub response_hash: String,
    pub operations: Vec<ProposedOperation>,
}

#[derive(Debug)]
pub struct ApplyOutcome {
    pub result: ReconciliationResult,
    pub result_path: PathBuf,
}

impl ApplyOutcome {
    pub fn has_failures(&self) -> bool {
        self.result
            .operations
            .iter()
            .any(|operation| operation.state == OperationState::Failed)
    }
}

pub fn start(
    repository: &Repository,
    primary_seed: &str,
    explicit_includes: &[String],
) -> Result<StartOutcome, BelayError> {
    let seed_reference = parse_entry_reference_id(primary_seed)?;
    if seed_reference.fragment.is_some() {
        return validation("Route primary seed must be an entry ID without a fragment");
    }
    let seed = store::show(repository, &seed_reference.display_id)?;

    let mut ids = BTreeSet::new();
    ids.insert(seed.entry.display_id.clone());
    for link in seed.entry.links.iter().chain(seed.inbound_links.iter()) {
        ids.insert(parse_entry_reference_id(&link.id)?.display_id);
    }
    for include in explicit_includes {
        let reference = parse_entry_reference_id(include)?;
        if reference.fragment.is_some() {
            return validation("Route included IDs must not contain fragments");
        }
        ids.insert(reference.display_id);
    }

    let included_ids = ids
        .iter()
        .filter(|id| *id != &seed.entry.display_id)
        .cloned()
        .collect::<Vec<_>>();
    let entries = load_route_entries(repository, &ids)?;
    let goal_coverage = load_goal_coverage(repository, &entries)?;
    let evidence_fingerprint = evidence_fingerprint(repository)?;
    let coverage_basis_fingerprint = coverage_basis_fingerprint(repository)?;
    let fingerprint = input_fingerprint(
        &seed.entry.display_id,
        &included_ids,
        &entries,
        &goal_coverage,
        &evidence_fingerprint,
        &coverage_basis_fingerprint,
    )?;
    let run_id = allocate_run_id(repository, &fingerprint)?;
    let run_path = run_directory(repository, &run_id)?;
    create_run_directory(repository, &run_path)?;

    let input = RouteInput {
        schema_version: ROUTE_SCHEMA_VERSION,
        artifact_type: "route-input".to_owned(),
        run_id: run_id.clone(),
        revision: 1,
        primary_seed: seed.entry.display_id,
        included_ids: included_ids.clone(),
        generated_at: now(),
        entries,
        goal_coverage,
        evidence_fingerprint,
        coverage_basis_fingerprint,
        fingerprint: fingerprint.clone(),
    };
    let input_file = "input-001.json".to_owned();
    let input_path = run_path.join(&input_file);
    let input_hash = write_json_new(&input_path, &input)?;
    let manifest = RouteManifest {
        schema_version: ROUTE_SCHEMA_VERSION,
        run_id: run_id.clone(),
        primary_seed: input.primary_seed.clone(),
        included_ids,
        phase: RoutePhase::Started,
        input: ArtifactReference {
            revision: 1,
            file: input_file,
            sha256: input_hash,
        },
        assessment: None,
        proposal: None,
        response: None,
        preview: None,
        reconciliation: None,
    };
    write_manifest(&run_path, &manifest)?;

    Ok(StartOutcome {
        run_id,
        run_path,
        input_path,
        fingerprint,
    })
}

pub fn submit(
    repository: &Repository,
    run_id: &str,
    kind: SubmitKind,
    source: &Path,
) -> Result<SubmitOutcome, BelayError> {
    require_regular_file(source)?;
    let bytes =
        fs::read(source).map_err(|error| BelayError::io("read Route artifact", source, error))?;
    let run_path = run_directory(repository, run_id)?;
    let mut manifest = read_manifest(&run_path)?;
    if matches!(
        manifest.phase,
        RoutePhase::Applying | RoutePhase::Reconciled
    ) {
        return validation(
            "an applying or reconciled Route run is immutable; start a new run for further reasoning",
        );
    }
    verify_input_current(repository, &run_path, &manifest)?;

    match kind {
        SubmitKind::Assessment => {
            let artifact: RouteAssessment = parse_json(&bytes, "Route Assessment")?;
            validate_assessment(repository, &run_path, &manifest, &artifact)?;
            let outcome = store_submitted(&run_path, "assessment", artifact.revision, &artifact)?;
            manifest.phase = RoutePhase::Assessed;
            manifest.assessment = Some(outcome.1.clone());
            write_manifest(&run_path, &manifest)?;
            Ok(outcome.0)
        }
        SubmitKind::Proposal => {
            let artifact: RouteProposal = parse_json(&bytes, "Route Proposal")?;
            validate_proposal(repository, &run_path, &manifest, &artifact)?;
            let outcome = store_submitted(&run_path, "proposal", artifact.revision, &artifact)?;
            manifest.phase = RoutePhase::Proposed;
            manifest.proposal = Some(outcome.1.clone());
            write_manifest(&run_path, &manifest)?;
            Ok(outcome.0)
        }
        SubmitKind::Response => {
            let artifact: HumanResponse = parse_json(&bytes, "Human Response")?;
            validate_response(&run_path, &manifest, &artifact)?;
            let outcome = store_submitted(&run_path, "response", artifact.revision, &artifact)?;
            manifest.phase = RoutePhase::Responded;
            manifest.response = Some(outcome.1.clone());
            write_manifest(&run_path, &manifest)?;
            Ok(outcome.0)
        }
    }
}

pub fn preview(repository: &Repository, run_id: &str) -> Result<PreviewOutcome, BelayError> {
    let run_path = run_directory(repository, run_id)?;
    let mut manifest = read_manifest(&run_path)?;
    require_phase(
        &manifest,
        &[RoutePhase::Responded, RoutePhase::Previewed],
        "create a Materialization Preview",
    )?;
    verify_input_current(repository, &run_path, &manifest)?;
    let proposal_ref = manifest
        .proposal
        .as_ref()
        .ok_or_else(|| validation_error("Route run has no Proposal"))?;
    let response_ref = manifest
        .response
        .as_ref()
        .ok_or_else(|| validation_error("Route run has no Human Response"))?;
    let proposal: RouteProposal = read_artifact(&run_path, proposal_ref, "Route Proposal")?;
    let response: HumanResponse = read_artifact(&run_path, response_ref, "Human Response")?;
    if response.proposal_revision != proposal_ref.revision
        || response.proposal_hash != proposal_ref.sha256
        || response.input_fingerprint != proposal.input_fingerprint
    {
        return validation("latest Human Response is not bound to the latest Proposal");
    }
    if response.action != HumanAction::Accept {
        return validation("only an accepted Human Response can produce a Materialization Preview");
    }
    let selected = response
        .selected_operation_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let operations = proposal
        .operations
        .iter()
        .filter(|operation| selected.contains(operation.operation_id()))
        .cloned()
        .collect::<Vec<_>>();
    let input = read_input(&run_path, &manifest)?;
    let entry_preconditions = operation_preconditions(&input, &operations)?;
    let unselected_operation_ids = proposal
        .operations
        .iter()
        .filter(|operation| !selected.contains(operation.operation_id()))
        .map(|operation| operation.operation_id().to_owned())
        .collect::<Vec<_>>();
    let allowed_ids = proposal_input_ids(&proposal, &input)?;
    validate_operations(repository, &operations, &allowed_ids)?;

    let revision = manifest
        .preview
        .as_ref()
        .map_or(1, |artifact| artifact.revision + 1);
    let mut artifact = MaterializationPreview {
        schema_version: ROUTE_SCHEMA_VERSION,
        artifact_type: "materialization-preview".to_owned(),
        run_id: manifest.run_id.clone(),
        revision,
        input_fingerprint: input.fingerprint,
        proposal_revision: proposal.revision,
        proposal_hash: proposal_ref.sha256.clone(),
        response_revision: response.revision,
        response_hash: response_ref.sha256.clone(),
        generated_at: now(),
        operations,
        entry_preconditions,
        unselected_operation_ids,
        preview_hash: String::new(),
    };
    artifact.preview_hash = preview_hash(&artifact)?;
    let file = format!("preview-{:03}.json", artifact.revision);
    let path = run_path.join(&file);
    let sha256 = write_json_new(&path, &artifact)?;
    manifest.phase = RoutePhase::Previewed;
    manifest.preview = Some(ArtifactReference {
        revision: artifact.revision,
        file,
        sha256,
    });
    write_manifest(&run_path, &manifest)?;
    Ok(PreviewOutcome {
        preview_path: path,
        preview_hash: artifact.preview_hash,
        revision: artifact.revision,
    })
}

/// Returns the one Preview an agent may present for conversational approval.
/// It verifies freshness but does not interpret a conversation or authorize a write.
pub fn pending(repository: &Repository, run_id: &str) -> Result<PendingPreview, BelayError> {
    let run_path = run_directory(repository, run_id)?;
    let manifest = read_manifest(&run_path)?;
    require_phase(
        &manifest,
        &[RoutePhase::Previewed],
        "inspect a pending Materialization Preview",
    )?;
    verify_input_current(repository, &run_path, &manifest)?;
    let preview_ref = manifest
        .preview
        .as_ref()
        .ok_or_else(|| validation_error("Route run has no Materialization Preview"))?;
    let preview: MaterializationPreview =
        read_artifact(&run_path, preview_ref, "Materialization Preview")?;
    if !preview_digest_matches(&preview)? {
        return validation("Materialization Preview content does not match its preview hash");
    }
    Ok(PendingPreview {
        run_id: preview.run_id,
        preview_revision: preview.revision,
        preview_hash: preview.preview_hash,
        input_fingerprint: preview.input_fingerprint,
        proposal_revision: preview.proposal_revision,
        proposal_hash: preview.proposal_hash,
        response_revision: preview.response_revision,
        response_hash: preview.response_hash,
        operations: preview.operations,
    })
}

pub fn apply(
    repository: &Repository,
    run_id: &str,
    approved_preview_hash: &str,
) -> Result<ApplyOutcome, BelayError> {
    let run_path = run_directory(repository, run_id)?;
    let mut manifest = read_manifest(&run_path)?;
    require_phase(
        &manifest,
        &[
            RoutePhase::Previewed,
            RoutePhase::Applying,
            RoutePhase::Reconciled,
        ],
        "apply a Materialization Preview",
    )?;
    let preview_ref = manifest
        .preview
        .as_ref()
        .ok_or_else(|| validation_error("Route run has no Materialization Preview"))?;
    let preview: MaterializationPreview =
        read_artifact(&run_path, preview_ref, "Materialization Preview")?;
    if preview.preview_hash != approved_preview_hash {
        return validation("approved preview hash does not match the latest preview");
    }
    if !preview_digest_matches(&preview)? {
        return validation("Materialization Preview content does not match its preview hash");
    }
    if manifest.phase == RoutePhase::Previewed {
        verify_input_current(repository, &run_path, &manifest)?;
        manifest.phase = RoutePhase::Applying;
        write_manifest(&run_path, &manifest)?;
    }

    let input = read_input(&run_path, &manifest)?;
    let previous = manifest
        .reconciliation
        .as_ref()
        .map(|reference| {
            read_artifact::<ReconciliationResult>(&run_path, reference, "Reconciliation Result")
        })
        .transpose()?;
    let mut aliases = if let Some(previous) = previous {
        if previous.preview_hash != preview.preview_hash {
            return validation(
                "latest Reconciliation Result belongs to a different preview; start a new Route run",
            );
        }
        let current = materialization_state_fingerprint(repository, &input, &previous.aliases)?;
        if current != previous.state_fingerprint_after {
            return Err(BelayError::Conflict {
                message: format!(
                    "Belay state changed after the last Route apply: recorded fingerprint {}, current fingerprint {}; start a new Route run",
                    previous.state_fingerprint_after, current
                ),
            });
        }
        previous.aliases
    } else {
        if manifest.phase == RoutePhase::Applying {
            verify_unmutated_input_current(
                repository,
                &manifest.run_id,
                &preview.preview_hash,
                &input,
                &preview.operations,
            )?;
        }
        BTreeMap::new()
    };
    let mut expected_revisions = preview.entry_preconditions.clone();
    let mut results = Vec::new();
    for operation in &preview.operations {
        let result = apply_operation(
            repository,
            &manifest.run_id,
            &preview.preview_hash,
            operation,
            &mut aliases,
            &mut expected_revisions,
        );
        results.push(match result {
            Ok(result) => result,
            Err(error) => OperationResult {
                operation_id: operation.operation_id().to_owned(),
                state: OperationState::Failed,
                target: None,
                message: Some(error.to_string()),
            },
        });
    }
    if let Err(error) = validate_expected_revisions(repository, &expected_revisions) {
        results.push(OperationResult {
            operation_id: "route-postcondition".to_owned(),
            state: OperationState::Failed,
            target: None,
            message: Some(error.to_string()),
        });
    }

    let revision = manifest
        .reconciliation
        .as_ref()
        .map_or(1, |artifact| artifact.revision + 1);
    let state_fingerprint_after = materialization_state_fingerprint(repository, &input, &aliases)?;
    let goal_coverage_after = load_goal_coverage(
        repository,
        &load_route_entries(
            repository,
            &std::iter::once(input.primary_seed.clone())
                .chain(input.included_ids.iter().cloned())
                .collect(),
        )?,
    )?;
    let result = ReconciliationResult {
        schema_version: ROUTE_SCHEMA_VERSION,
        artifact_type: "reconciliation-result".to_owned(),
        run_id: manifest.run_id.clone(),
        revision,
        preview_hash: preview.preview_hash,
        input_fingerprint_before: input.fingerprint,
        state_fingerprint_after,
        reconciled_at: now(),
        operations: results,
        unselected_operation_ids: preview.unselected_operation_ids,
        aliases,
        goal_coverage_after,
    };
    let file = format!("reconciliation-{:03}.json", revision);
    let path = run_path.join(&file);
    let sha256 = write_json_new(&path, &result)?;
    manifest.phase = RoutePhase::Reconciled;
    manifest.reconciliation = Some(ArtifactReference {
        revision,
        file,
        sha256,
    });
    write_manifest(&run_path, &manifest)?;
    Ok(ApplyOutcome {
        result,
        result_path: path,
    })
}

pub fn status(repository: &Repository, run_id: &str) -> Result<RouteManifest, BelayError> {
    read_manifest(&run_directory(repository, run_id)?)
}

pub fn template(
    repository: &Repository,
    run_id: &str,
    kind: TemplateKind,
) -> Result<serde_json::Value, BelayError> {
    let run_path = run_directory(repository, run_id)?;
    let manifest = read_manifest(&run_path)?;
    if matches!(
        manifest.phase,
        RoutePhase::Applying | RoutePhase::Reconciled
    ) {
        return validation(
            "an applying or reconciled Route run is immutable; start a new run for further reasoning",
        );
    }
    verify_input_current(repository, &run_path, &manifest)?;
    let input = read_input(&run_path, &manifest)?;
    match kind {
        TemplateKind::Assessment => Ok(serde_json::json!({
            "schema_version": ROUTE_SCHEMA_VERSION,
            "artifact_type": "route-assessment",
            "run_id": manifest.run_id,
            "revision": manifest.assessment.as_ref().map_or(1, |item| item.revision + 1),
            "input_fingerprint": input.fingerprint,
            "outcome": "continue",
            "items": [],
            "limitations": []
        })),
        TemplateKind::Proposal => {
            let assessment = manifest.assessment.as_ref().ok_or_else(|| {
                validation_error("submit an Assessment before requesting a Proposal template")
            })?;
            Ok(serde_json::json!({
                "schema_version": ROUTE_SCHEMA_VERSION,
                "artifact_type": "route-proposal",
                "run_id": manifest.run_id,
                "revision": manifest.proposal.as_ref().map_or(1, |item| item.revision + 1),
                "input_fingerprint": input.fingerprint,
                "assessment_hash": assessment.sha256,
                "outcome": "continue",
                "summary": "Replace with a concise proposal summary.",
                "operations": []
            }))
        }
        TemplateKind::Response => {
            require_phase(
                &manifest,
                &[
                    RoutePhase::Proposed,
                    RoutePhase::Responded,
                    RoutePhase::Previewed,
                ],
                "create a Human Response template",
            )?;
            let proposal = manifest.proposal.as_ref().ok_or_else(|| {
                validation_error("submit a Proposal before requesting a Human Response template")
            })?;
            Ok(serde_json::json!({
                "schema_version": ROUTE_SCHEMA_VERSION,
                "artifact_type": "human-response",
                "run_id": manifest.run_id,
                "revision": manifest.response.as_ref().map_or(1, |item| item.revision + 1),
                "input_fingerprint": input.fingerprint,
                "proposal_revision": proposal.revision,
                "proposal_hash": proposal.sha256,
                "action": "defer",
                "selected_operation_ids": [],
                "reason": "Replace with the human's explicit response.",
                "requested_changes": [],
                "issuer": "local:user",
                "responded_at": now()
            }))
        }
    }
}

fn apply_operation(
    repository: &Repository,
    run_id: &str,
    preview_hash: &str,
    operation: &ProposedOperation,
    aliases: &mut BTreeMap<String, String>,
    expected_revisions: &mut BTreeMap<String, u32>,
) -> Result<OperationResult, BelayError> {
    let operation_key = format!("{run_id}:{}", operation.operation_id());
    match operation {
        ProposedOperation::CreateEntry {
            operation_id,
            alias,
            entry_type,
            title,
            body,
        } => {
            let mut metadata = BTreeMap::new();
            metadata.insert(
                ROUTE_OPERATION_KEY.to_owned(),
                MetadataValue::String(operation_key),
            );
            metadata.insert(
                ROUTE_RUN_KEY.to_owned(),
                MetadataValue::String(run_id.to_owned()),
            );
            let outcome = store::route_create_with_metadata(
                repository,
                run_id,
                operation_id,
                preview_hash,
                *entry_type,
                title.clone(),
                body.clone(),
                metadata,
            )?;
            let entry = outcome.value;
            aliases.insert(alias.clone(), entry.display_id.clone());
            expected_revisions.insert(entry.display_id.clone(), outcome.post_revision);
            Ok(OperationResult {
                operation_id: operation_id.clone(),
                state: if outcome.replayed {
                    OperationState::Unchanged
                } else {
                    mutation_state(outcome.outcome)
                },
                target: Some(entry.display_id),
                message: outcome
                    .replayed
                    .then(|| "operation replayed from durable receipt".to_owned()),
            })
        }
        ProposedOperation::Link {
            operation_id,
            from,
            to,
            relation,
        } => {
            let from = resolve_target(from, aliases)?;
            let to = resolve_target(to, aliases)?;
            let expected_revision = mutation::revision_precondition(expected_revisions, &from)
                .ok_or_else(|| {
                    validation_error(format!("missing preview revision precondition for {from}"))
                })?;
            let outcome = store::route_link_if_revision(
                repository,
                run_id,
                operation_id,
                preview_hash,
                &from,
                &to,
                *relation,
                expected_revision,
            )?;
            expected_revisions.insert(from.clone(), outcome.post_revision);
            Ok(OperationResult {
                operation_id: operation_id.clone(),
                state: if outcome.replayed {
                    OperationState::Unchanged
                } else {
                    mutation_state(outcome.outcome)
                },
                target: Some(outcome.value),
                message: outcome
                    .replayed
                    .then(|| "operation replayed from durable receipt".to_owned()),
            })
        }
        ProposedOperation::SetStatus {
            operation_id,
            target,
            status,
        } => {
            let target = resolve_target(target, aliases)?;
            let expected_revision = mutation::revision_precondition(expected_revisions, &target)
                .ok_or_else(|| {
                    validation_error(format!(
                        "missing preview revision precondition for {target}"
                    ))
                })?;
            let outcome = store::route_set_status_if_revision(
                repository,
                run_id,
                operation_id,
                preview_hash,
                &target,
                *status,
                expected_revision,
            )?;
            expected_revisions.insert(target.clone(), outcome.post_revision);
            Ok(OperationResult {
                operation_id: operation_id.clone(),
                state: if outcome.replayed {
                    OperationState::Unchanged
                } else {
                    mutation_state(outcome.outcome)
                },
                target: Some(outcome.value),
                message: outcome
                    .replayed
                    .then(|| "operation replayed from durable receipt".to_owned()),
            })
        }
    }
}

fn mutation_state(outcome: MutationOutcome) -> OperationState {
    match outcome {
        MutationOutcome::Changed => OperationState::Applied,
        MutationOutcome::Unchanged => OperationState::Unchanged,
    }
}

fn validate_expected_revisions(
    repository: &Repository,
    expected_revisions: &BTreeMap<String, u32>,
) -> Result<(), BelayError> {
    for (display_id, expected) in expected_revisions {
        let actual = store::show(repository, display_id)?.entry.revision;
        if actual != *expected {
            return Err(BelayError::Conflict {
                message: format!(
                    "entry {display_id} changed after a committed Route operation: expected revision {expected}, actual {actual}"
                ),
            });
        }
    }
    Ok(())
}

fn verify_unmutated_input_current(
    repository: &Repository,
    run_id: &str,
    preview_hash: &str,
    input: &RouteInput,
    operations: &[ProposedOperation],
) -> Result<(), BelayError> {
    if evidence_fingerprint(repository)? != input.evidence_fingerprint {
        return Err(BelayError::Conflict {
            message: "Evidence changed while Route apply was interrupted".to_owned(),
        });
    }
    if coverage_basis_fingerprint(repository)? != input.coverage_basis_fingerprint {
        return Err(BelayError::Conflict {
            message: "Evidence freshness basis changed while Route apply was interrupted"
                .to_owned(),
        });
    }
    let mut mutable_ids = BTreeSet::new();
    let receipts = store::route_receipt_targets(repository, run_id, preview_hash)?;
    let mut aliases = BTreeMap::new();
    for operation in operations {
        if let ProposedOperation::CreateEntry {
            operation_id,
            alias,
            ..
        } = operation
        {
            if let Some((target, _)) = receipts.get(operation_id) {
                aliases.insert(alias.clone(), target.clone());
            }
        }
    }
    let mut expected_entries = input
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for operation in operations {
        let target = match operation {
            ProposedOperation::CreateEntry { .. } => None,
            ProposedOperation::Link { from, .. } => Some(from),
            ProposedOperation::SetStatus { target, .. } => Some(target),
        };
        if let Some(target) = target {
            if !target.starts_with('$') {
                mutable_ids.insert(parse_entry_reference_id(target)?.display_id);
            }
        }
        if let ProposedOperation::Link {
            operation_id,
            from,
            to,
            relation,
        } = operation
        {
            if matches!(
                receipts.get(operation_id),
                Some((_, MutationOutcome::Changed))
            ) && !to.starts_with('$')
            {
                let to_id = parse_entry_reference_id(to)?.display_id;
                let from_id = resolve_target(from, &aliases)?;
                let expected = expected_entries.get_mut(&to_id).ok_or_else(|| {
                    validation_error(format!("Route link target {to_id} is absent from Input"))
                })?;
                let link = EntryLink {
                    relation: *relation,
                    id: from_id,
                    metadata: BTreeMap::new(),
                };
                if !expected.inbound_links.contains(&link) {
                    expected.inbound_links.push(link);
                }
                expected.inbound_links.sort_by(|left, right| {
                    left.relation
                        .to_string()
                        .cmp(&right.relation.to_string())
                        .then_with(|| left.id.cmp(&right.id))
                });
            }
        }
    }
    let ids = input
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let current = load_route_entries(repository, &ids)?
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for expected in expected_entries.values() {
        let actual = current
            .get(&expected.id)
            .ok_or_else(|| BelayError::Conflict {
                message: format!("unmodified Route Input entry {} disappeared", expected.id),
            })?;
        if actual.inbound_links != expected.inbound_links {
            return Err(BelayError::Conflict {
                message: format!(
                    "inbound links for Route Input entry {} changed outside committed Route receipts",
                    expected.id
                ),
            });
        }
        if mutable_ids.contains(&expected.id) {
            continue;
        }
        if json_hash(actual)? != json_hash(expected)? {
            return Err(BelayError::Conflict {
                message: format!(
                    "unmodified Route Input entry {} changed while apply was interrupted",
                    expected.id
                ),
            });
        }
    }
    let current_coverage =
        load_goal_coverage(repository, &current.values().cloned().collect::<Vec<_>>())?;
    for (goal_id, expected) in &input.goal_coverage {
        if mutable_ids.contains(goal_id)
            || expected_entries.get(goal_id).is_some_and(|entry| {
                entry.inbound_links
                    != input
                        .entries
                        .iter()
                        .find(|item| item.id == *goal_id)
                        .map_or(&[][..], |item| item.inbound_links.as_slice())
            })
        {
            continue;
        }
        if current_coverage.get(goal_id) != Some(expected) {
            return Err(BelayError::Conflict {
                message: format!(
                    "Goal Coverage for unmodified Route Input entry {goal_id} changed while apply was interrupted"
                ),
            });
        }
    }
    Ok(())
}

fn resolve_target(target: &str, aliases: &BTreeMap<String, String>) -> Result<String, BelayError> {
    if let Some(alias) = target.strip_prefix('$') {
        return aliases
            .get(alias)
            .cloned()
            .ok_or_else(|| validation_error(format!("unresolved Route alias ${alias}")));
    }
    Ok(parse_entry_reference_id(target)?.canonical_id())
}

fn operation_preconditions(
    input: &RouteInput,
    operations: &[ProposedOperation],
) -> Result<BTreeMap<String, u32>, BelayError> {
    let input_revisions = input
        .entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry.revision))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for operation in operations {
        let target = match operation {
            ProposedOperation::CreateEntry { .. } => None,
            ProposedOperation::Link { from, .. } => Some(from),
            ProposedOperation::SetStatus { target, .. } => Some(target),
        };
        let Some(target) = target else {
            continue;
        };
        if target.starts_with('$') {
            continue;
        }
        let reference = parse_entry_reference_id(target)?;
        if reference.fragment.is_some() {
            return validation("mutable Route operation targets must not contain fragments");
        }
        let revision = input_revisions
            .get(reference.display_id.as_str())
            .copied()
            .ok_or_else(|| {
                validation_error(format!(
                    "missing Route Input revision for {}",
                    reference.display_id
                ))
            })?;
        result.insert(reference.display_id, revision);
    }
    Ok(result)
}

fn validate_assessment(
    repository: &Repository,
    run_path: &Path,
    manifest: &RouteManifest,
    artifact: &RouteAssessment,
) -> Result<(), BelayError> {
    validate_common(
        manifest,
        artifact.schema_version,
        &artifact.artifact_type,
        "route-assessment",
        &artifact.run_id,
        artifact.revision,
        manifest.assessment.as_ref(),
    )?;
    let input = read_input(run_path, manifest)?;
    if artifact.input_fingerprint != input.fingerprint {
        return validation("Route Assessment input fingerprint does not match Route Input");
    }
    let known = input
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for item in &artifact.items {
        validate_identifier("assessment item ID", &item.id)?;
        if !ids.insert(item.id.as_str()) {
            return validation(format!("duplicate Assessment item ID {}", item.id));
        }
        if item.summary.trim().is_empty() {
            return validation(format!("Assessment item {} has an empty summary", item.id));
        }
        if matches!(
            item.classification,
            AssessmentClassification::Fact
                | AssessmentClassification::HumanObservation
                | AssessmentClassification::Conflict
        ) && item.references.is_empty()
        {
            return validation(format!(
                "Assessment item {} requires at least one Belay reference",
                item.id
            ));
        }
        for reference in &item.references {
            let reference = parse_entry_reference_id(reference)?;
            if !known.contains(reference.display_id.as_str()) {
                return validation(format!(
                    "Assessment item {} references {} outside Route Input",
                    item.id, reference.display_id
                ));
            }
            store::validate_entry_reference(repository, &reference.canonical_id())?;
        }
    }
    Ok(())
}

fn validate_proposal(
    repository: &Repository,
    run_path: &Path,
    manifest: &RouteManifest,
    artifact: &RouteProposal,
) -> Result<(), BelayError> {
    validate_common(
        manifest,
        artifact.schema_version,
        &artifact.artifact_type,
        "route-proposal",
        &artifact.run_id,
        artifact.revision,
        manifest.proposal.as_ref(),
    )?;
    let input = read_input(run_path, manifest)?;
    if artifact.input_fingerprint != input.fingerprint {
        return validation("Route Proposal input fingerprint does not match Route Input");
    }
    let assessment = manifest
        .assessment
        .as_ref()
        .ok_or_else(|| validation_error("Route Proposal requires a submitted Assessment"))?;
    if artifact.assessment_hash != assessment.sha256 {
        return validation("Route Proposal assessment hash does not match latest Assessment");
    }
    if artifact.summary.trim().is_empty() {
        return validation("Route Proposal summary must not be empty");
    }
    if artifact.outcome != AssessmentOutcome::Continue && !artifact.operations.is_empty() {
        return validation("stopped Route Proposals must not contain materialization operations");
    }
    let allowed_ids = input
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    validate_operations(repository, &artifact.operations, &allowed_ids)
}

fn validate_operations(
    repository: &Repository,
    operations: &[ProposedOperation],
    allowed_ids: &BTreeSet<String>,
) -> Result<(), BelayError> {
    let mut operation_ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let mut alias_types = BTreeMap::new();
    for operation in operations {
        validate_identifier("operation ID", operation.operation_id())?;
        if !operation_ids.insert(operation.operation_id()) {
            return validation(format!(
                "duplicate Proposal operation ID {}",
                operation.operation_id()
            ));
        }
        if let ProposedOperation::CreateEntry {
            alias,
            entry_type,
            title,
            body,
            ..
        } = operation
        {
            validate_identifier("entry alias", alias)?;
            if !aliases.insert(alias.as_str()) {
                return validation(format!("duplicate Proposal entry alias {alias}"));
            }
            alias_types.insert(alias.as_str(), *entry_type);
            if title.trim().is_empty() {
                return validation(format!(
                    "create-entry operation {} has an empty title",
                    operation.operation_id()
                ));
            }
            if body.contains('\0') {
                return validation(format!(
                    "create-entry operation {} body contains NUL",
                    operation.operation_id()
                ));
            }
        }
    }
    let mut available_aliases = BTreeSet::new();
    for operation in operations {
        match operation {
            ProposedOperation::CreateEntry { alias, .. } => {
                available_aliases.insert(alias.as_str());
            }
            ProposedOperation::Link { from, to, .. } => {
                validate_operation_target(repository, from, &available_aliases, allowed_ids)?;
                validate_operation_target(repository, to, &available_aliases, allowed_ids)?;
                if !from.starts_with('$') && parse_entry_reference_id(from)?.fragment.is_some() {
                    return validation("link source must not contain a fragment");
                }
            }
            ProposedOperation::SetStatus { target, status, .. } => {
                if let Some(alias) = target.strip_prefix('$') {
                    validate_operation_target(repository, target, &available_aliases, allowed_ids)?;
                    let entry_type = alias_types
                        .get(alias)
                        .ok_or_else(|| validation_error(format!("unknown alias ${alias}")))?;
                    if !entry_type.allows_status(*status) {
                        return validation(format!(
                            "status {status} is invalid for aliased {entry_type} entry"
                        ));
                    }
                } else {
                    let target = parse_entry_reference_id(target)?;
                    if target.fragment.is_some() {
                        return validation("status target must not contain a fragment");
                    }
                    if !allowed_ids.contains(&target.display_id) {
                        return validation(format!(
                            "Proposal operation target {} is outside Route Input",
                            target.display_id
                        ));
                    }
                    let shown = store::show(repository, &target.display_id)?;
                    if !shown.entry.entry_type.allows_status(*status) {
                        return validation(format!(
                            "status {status} is invalid for {}",
                            shown.entry.entry_type
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_operation_target(
    repository: &Repository,
    target: &str,
    aliases: &BTreeSet<&str>,
    allowed_ids: &BTreeSet<String>,
) -> Result<(), BelayError> {
    if let Some(alias) = target.strip_prefix('$') {
        if !aliases.contains(alias) {
            return validation(format!("Proposal references unknown alias ${alias}"));
        }
        return Ok(());
    }
    let reference = parse_entry_reference_id(target)?;
    if !allowed_ids.contains(&reference.display_id) {
        return validation(format!(
            "Proposal operation target {} is outside Route Input",
            reference.display_id
        ));
    }
    store::validate_entry_reference(repository, &reference.canonical_id())
}

fn proposal_input_ids(
    proposal: &RouteProposal,
    input: &RouteInput,
) -> Result<BTreeSet<String>, BelayError> {
    if proposal.input_fingerprint != input.fingerprint {
        return validation("Route Proposal input fingerprint does not match Route Input");
    }
    Ok(input.entries.iter().map(|entry| entry.id.clone()).collect())
}

fn validate_response(
    run_path: &Path,
    manifest: &RouteManifest,
    artifact: &HumanResponse,
) -> Result<(), BelayError> {
    require_phase(
        manifest,
        &[
            RoutePhase::Proposed,
            RoutePhase::Responded,
            RoutePhase::Previewed,
        ],
        "submit a Human Response",
    )?;
    validate_common(
        manifest,
        artifact.schema_version,
        &artifact.artifact_type,
        "human-response",
        &artifact.run_id,
        artifact.revision,
        manifest.response.as_ref(),
    )?;
    let input = read_input(run_path, manifest)?;
    if artifact.input_fingerprint != input.fingerprint {
        return validation("Human Response input fingerprint does not match Route Input");
    }
    let proposal_ref = manifest
        .proposal
        .as_ref()
        .ok_or_else(|| validation_error("Human Response requires a submitted Proposal"))?;
    if artifact.proposal_revision != proposal_ref.revision
        || artifact.proposal_hash != proposal_ref.sha256
    {
        return validation("Human Response does not match the latest Proposal revision and hash");
    }
    if artifact.reason.trim().is_empty() || artifact.issuer.trim().is_empty() {
        return validation("Human Response reason and issuer must not be empty");
    }
    DateTime::parse_from_rfc3339(&artifact.responded_at).map_err(|error| {
        validation_error(format!(
            "Human Response responded_at must be RFC 3339: {error}"
        ))
    })?;
    let proposal: RouteProposal = read_artifact(run_path, proposal_ref, "Route Proposal")?;
    let operation_ids = proposal
        .operations
        .iter()
        .map(ProposedOperation::operation_id)
        .collect::<BTreeSet<_>>();
    let selected = artifact
        .selected_operation_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if selected.len() != artifact.selected_operation_ids.len() {
        return validation("Human Response contains duplicate selected operation IDs");
    }
    if let Some(unknown) = selected.difference(&operation_ids).next() {
        return validation(format!(
            "Human Response selects unknown Proposal operation {unknown}"
        ));
    }
    match artifact.action {
        HumanAction::Accept if selected.is_empty() => {
            validation("accepted Human Response must select at least one operation")
        }
        HumanAction::Accept if !artifact.requested_changes.is_empty() => validation(
            "accepted Human Response cannot include requested changes; submit a revised Proposal",
        ),
        HumanAction::Revise if artifact.requested_changes.is_empty() => {
            validation("revised Human Response must describe requested changes")
        }
        HumanAction::Revise | HumanAction::Reject | HumanAction::Defer if !selected.is_empty() => {
            validation("non-accepted Human Response must not select operations")
        }
        _ => Ok(()),
    }
}

fn validate_common(
    manifest: &RouteManifest,
    schema_version: u32,
    artifact_type: &str,
    expected_type: &str,
    run_id: &str,
    revision: u32,
    latest: Option<&ArtifactReference>,
) -> Result<(), BelayError> {
    if schema_version != ROUTE_SCHEMA_VERSION {
        return validation(format!(
            "unsupported Route schema version {schema_version}; expected {ROUTE_SCHEMA_VERSION}"
        ));
    }
    if artifact_type != expected_type {
        return validation(format!(
            "artifact type {artifact_type:?} does not match expected {expected_type:?}"
        ));
    }
    if run_id != manifest.run_id {
        return validation("Route artifact run ID does not match the target run");
    }
    let expected_revision = latest.map_or(1, |artifact| artifact.revision + 1);
    if revision != expected_revision {
        return validation(format!(
            "Route artifact revision {revision} is invalid; expected {expected_revision}"
        ));
    }
    Ok(())
}

fn require_phase(
    manifest: &RouteManifest,
    allowed: &[RoutePhase],
    action: &str,
) -> Result<(), BelayError> {
    if allowed.contains(&manifest.phase) {
        Ok(())
    } else {
        validation(format!(
            "Route run phase {:?} cannot {action}",
            manifest.phase
        ))
    }
}

fn store_submitted<T: Serialize>(
    run_path: &Path,
    prefix: &str,
    revision: u32,
    artifact: &T,
) -> Result<(SubmitOutcome, ArtifactReference), BelayError> {
    let file = format!("{prefix}-{revision:03}.json");
    let path = run_path.join(&file);
    let sha256 = write_json_new(&path, artifact)?;
    let reference = ArtifactReference {
        revision,
        file,
        sha256: sha256.clone(),
    };
    Ok((
        SubmitOutcome {
            artifact_path: path,
            artifact_hash: sha256,
            revision,
        },
        reference,
    ))
}

fn verify_input_current(
    repository: &Repository,
    run_path: &Path,
    manifest: &RouteManifest,
) -> Result<(), BelayError> {
    let input = read_input(run_path, manifest)?;
    let current = current_input_fingerprint(repository, &input)?;
    if current != input.fingerprint {
        return Err(BelayError::Conflict {
            message: format!(
                "Route Input is stale: recorded fingerprint {}, current fingerprint {}; start a new Route run",
                input.fingerprint, current
            ),
        });
    }
    Ok(())
}

fn current_input_fingerprint(
    repository: &Repository,
    input: &RouteInput,
) -> Result<String, BelayError> {
    let ids = std::iter::once(input.primary_seed.clone())
        .chain(input.included_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let entries = load_route_entries(repository, &ids)?;
    let goal_coverage = load_goal_coverage(repository, &entries)?;
    let evidence_fingerprint = evidence_fingerprint(repository)?;
    let coverage_basis_fingerprint = coverage_basis_fingerprint(repository)?;
    input_fingerprint(
        &input.primary_seed,
        &input.included_ids,
        &entries,
        &goal_coverage,
        &evidence_fingerprint,
        &coverage_basis_fingerprint,
    )
}

#[derive(Serialize)]
struct MaterializationStateFingerprint<'a> {
    input_fingerprint: String,
    aliases: &'a BTreeMap<String, String>,
    alias_entries: Vec<RouteEntry>,
}

fn materialization_state_fingerprint(
    repository: &Repository,
    input: &RouteInput,
    aliases: &BTreeMap<String, String>,
) -> Result<String, BelayError> {
    let input_fingerprint = current_input_fingerprint(repository, input)?;
    let alias_ids = aliases.values().cloned().collect::<BTreeSet<_>>();
    let alias_entries = load_route_entries(repository, &alias_ids)?;
    json_hash(&MaterializationStateFingerprint {
        input_fingerprint,
        aliases,
        alias_entries,
    })
}

fn read_input(run_path: &Path, manifest: &RouteManifest) -> Result<RouteInput, BelayError> {
    read_artifact(run_path, &manifest.input, "Route Input")
}

fn read_artifact<T: for<'de> Deserialize<'de>>(
    run_path: &Path,
    reference: &ArtifactReference,
    label: &str,
) -> Result<T, BelayError> {
    let path = safe_artifact_path(run_path, &reference.file)?;
    let bytes =
        fs::read(&path).map_err(|error| BelayError::io("read Route artifact", &path, error))?;
    let actual = sha256_hex(&bytes);
    if actual != reference.sha256 {
        return validation(format!(
            "{label} hash mismatch: manifest {}, actual {}",
            reference.sha256, actual
        ));
    }
    parse_json(&bytes, label)
}

fn load_route_entries(
    repository: &Repository,
    ids: &BTreeSet<String>,
) -> Result<Vec<RouteEntry>, BelayError> {
    ids.iter()
        .map(|id| {
            let shown = store::show(repository, id)?;
            Ok(RouteEntry {
                id: shown.entry.display_id,
                entry_type: shown.entry.entry_type,
                title: shown.entry.title,
                status: shown.entry.status,
                revision: shown.entry.revision,
                updated_at: shown.entry.updated_at,
                tags: shown.entry.tags,
                metadata: shown.entry.metadata,
                links: shown.entry.links,
                inbound_links: shown.inbound_links,
                body: shown.entry.body,
                source_path: shown.source_path,
            })
        })
        .collect()
}

#[derive(Serialize)]
struct InputFingerprint<'a> {
    schema_version: u32,
    primary_seed: &'a str,
    included_ids: &'a [String],
    entries: &'a [RouteEntry],
    goal_coverage: &'a BTreeMap<String, serde_json::Value>,
    evidence_fingerprint: &'a str,
    coverage_basis_fingerprint: &'a str,
}

fn input_fingerprint(
    primary_seed: &str,
    included_ids: &[String],
    entries: &[RouteEntry],
    goal_coverage: &BTreeMap<String, serde_json::Value>,
    evidence_fingerprint: &str,
    coverage_basis_fingerprint: &str,
) -> Result<String, BelayError> {
    json_hash(&InputFingerprint {
        schema_version: ROUTE_SCHEMA_VERSION,
        primary_seed,
        included_ids,
        entries,
        goal_coverage,
        evidence_fingerprint,
        coverage_basis_fingerprint,
    })
}

fn load_goal_coverage(
    repository: &Repository,
    entries: &[RouteEntry],
) -> Result<BTreeMap<String, serde_json::Value>, BelayError> {
    let mut result = BTreeMap::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::Goal)
    {
        let report = coverage::report(repository, Some(&entry.id), true)?;
        result.insert(
            entry.id.clone(),
            serde_json::to_value(report).map_err(serialization_error)?,
        );
    }
    Ok(result)
}

fn evidence_fingerprint(repository: &Repository) -> Result<String, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT evidence.display_id, evidence.kind, evidence.verdict,
                   evidence.commit_sha, evidence.captured_at, evidence.source,
                   evidence.issuer, evidence.summary, evidence.detail_json,
                   evidence_links.target, evidence_links.relation
            FROM evidence
            LEFT JOIN evidence_links ON evidence_links.evidence_id = evidence.id
            ORDER BY evidence.display_id, evidence_links.target, evidence_links.relation
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
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    json_hash(&rows)
}

fn coverage_basis_fingerprint(repository: &Repository) -> Result<String, BelayError> {
    let database_path = repository.database_path();
    let connection = crate::database::open_read_only(&database_path)?;
    let head = crate::evidence::current_head(repository).ok();
    let mut statement = connection
        .prepare(
            "
            SELECT display_id, commit_sha, captured_at
            FROM evidence
            ORDER BY display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .into_iter()
        .map(|(display_id, commit_sha, captured_at)| {
            (
                display_id,
                crate::evidence::freshness(repository, head.as_deref(), &commit_sha, &captured_at)
                    .label(),
            )
        })
        .collect::<Vec<_>>();
    json_hash(&(
        head,
        repository.config.verify.stale_after_commits,
        repository.config.verify.stale_after_days,
        rows,
    ))
}

fn preview_hash(preview: &MaterializationPreview) -> Result<String, BelayError> {
    let mut value = preview.clone();
    value.preview_hash.clear();
    mutation::content_digest(&value)
}

fn preview_digest_matches(preview: &MaterializationPreview) -> Result<bool, BelayError> {
    let mut value = preview.clone();
    value.preview_hash.clear();
    mutation::digest_matches(&value, &preview.preview_hash)
}

fn json_hash<T: Serialize>(value: &T) -> Result<String, BelayError> {
    let bytes = serde_json::to_vec(value).map_err(serialization_error)?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn allocate_run_id(repository: &Repository, fingerprint: &str) -> Result<String, BelayError> {
    let timestamp = Local::now().format("%Y%m%dT%H%M%S");
    let base = format!("ROUTE-{timestamp}-{}", &fingerprint[..8]);
    let root = route_root(repository);
    for sequence in 1..=999 {
        let candidate = if sequence == 1 {
            base.clone()
        } else {
            format!("{base}-{sequence:03}")
        };
        if !root.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    validation("could not allocate a unique Route run ID")
}

fn route_root(repository: &Repository) -> PathBuf {
    repository.belay_dir.join("state/route")
}

fn run_directory(repository: &Repository, run_id: &str) -> Result<PathBuf, BelayError> {
    validate_run_id(run_id)?;
    require_route_root(repository)?;
    Ok(route_root(repository).join(run_id))
}

fn validate_run_id(run_id: &str) -> Result<(), BelayError> {
    if !run_id.starts_with("ROUTE-")
        || run_id.len() > 64
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return validation("invalid Route run ID");
    }
    Ok(())
}

fn create_run_directory(repository: &Repository, path: &Path) -> Result<(), BelayError> {
    let root = route_root(repository);
    reject_symlink(&root)?;
    fs::create_dir(path)
        .map_err(|error| BelayError::io("create Route run directory", path, error))?;
    reject_symlink(path)
}

fn require_route_root(repository: &Repository) -> Result<(), BelayError> {
    reject_symlink(&repository.belay_dir)?;
    let state = repository.belay_dir.join("state");
    reject_symlink(&state)?;
    let root = route_root(repository);
    match fs::symlink_metadata(&root) {
        Ok(_) => reject_symlink(&root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .map_err(|error| BelayError::io("create Route state directory", &root, error))?;
            reject_symlink(&root)
        }
        Err(error) => Err(BelayError::io(
            "inspect Route state directory",
            &root,
            error,
        )),
    }
}

fn read_manifest(run_path: &Path) -> Result<RouteManifest, BelayError> {
    reject_symlink(run_path)?;
    let path = run_path.join("manifest.json");
    require_regular_file(&path)?;
    let bytes =
        fs::read(&path).map_err(|error| BelayError::io("read Route manifest", &path, error))?;
    let manifest: RouteManifest = parse_json(&bytes, "Route manifest")?;
    validate_run_id(&manifest.run_id)?;
    if run_path.file_name().and_then(|name| name.to_str()) != Some(manifest.run_id.as_str()) {
        return validation("Route manifest run ID does not match its directory");
    }
    if manifest.schema_version != ROUTE_SCHEMA_VERSION {
        return validation(format!(
            "unsupported Route schema version {}; expected {}",
            manifest.schema_version, ROUTE_SCHEMA_VERSION
        ));
    }
    Ok(manifest)
}

fn write_manifest(run_path: &Path, manifest: &RouteManifest) -> Result<(), BelayError> {
    let path = run_path.join("manifest.json");
    write_json_replace(&path, manifest)
}

fn safe_artifact_path(run_path: &Path, file: &str) -> Result<PathBuf, BelayError> {
    if file.is_empty() || file.contains('/') || file.contains('\\') || file == "." || file == ".." {
        return validation("Route manifest contains an unsafe artifact path");
    }
    Ok(run_path.join(file))
}

fn write_json_new<T: Serialize>(path: &Path, value: &T) -> Result<String, BelayError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(serialization_error)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| BelayError::io("create Route artifact", path, error))?;
    file.write_all(&bytes)
        .map_err(|error| BelayError::io("write Route artifact", path, error))?;
    file.sync_all()
        .map_err(|error| BelayError::io("sync Route artifact", path, error))?;
    Ok(sha256_hex(&bytes))
}

fn write_json_replace<T: Serialize>(path: &Path, value: &T) -> Result<(), BelayError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(serialization_error)?;
    bytes.push(b'\n');
    let parent = path
        .parent()
        .ok_or_else(|| validation_error("Route manifest path has no parent"))?;
    reject_symlink(parent)?;
    let temporary = parent.join(".manifest.json.tmp");
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|error| BelayError::io("remove stale Route manifest", &temporary, error))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| BelayError::io("create Route manifest", &temporary, error))?;
    file.write_all(&bytes)
        .map_err(|error| BelayError::io("write Route manifest", &temporary, error))?;
    file.sync_all()
        .map_err(|error| BelayError::io("sync Route manifest", &temporary, error))?;
    fs::rename(&temporary, path)
        .map_err(|error| BelayError::io("replace Route manifest", path, error))
}

fn require_regular_file(path: &Path) -> Result<(), BelayError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| BelayError::io("inspect Route artifact", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return validation(format!(
            "Route artifact {} must be a regular file",
            path.display()
        ));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), BelayError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| BelayError::io("inspect Route path", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return validation(format!(
            "Route path {} must be a directory and not a symlink",
            path.display()
        ));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), BelayError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return validation(format!(
            "{label} must contain only ASCII letters, digits, hyphens, or underscores"
        ));
    }
    Ok(())
}

fn parse_json<T: for<'de> Deserialize<'de>>(bytes: &[u8], label: &str) -> Result<T, BelayError> {
    serde_json::from_slice(bytes).map_err(|error| BelayError::Validation {
        message: format!("{label} is invalid JSON: {error}"),
    })
}

fn serialization_error(error: serde_json::Error) -> BelayError {
    BelayError::Validation {
        message: format!("Route JSON serialization failed: {error}"),
    }
}

fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(validation_error(message))
}

fn validation_error(message: impl Into<String>) -> BelayError {
    BelayError::Validation {
        message: message.into(),
    }
}

fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use crate::repository;

    #[test]
    fn run_ids_reject_path_traversal() {
        assert!(validate_run_id("ROUTE-20260729T120000-deadbeef").is_ok());
        assert!(validate_run_id("../ROUTE-bad").is_err());
        assert!(validate_run_id("ROUTE-bad/path").is_err());
    }

    #[test]
    fn preview_hash_ignores_only_the_embedded_hash() {
        let mut preview = MaterializationPreview {
            schema_version: ROUTE_SCHEMA_VERSION,
            artifact_type: "materialization-preview".to_owned(),
            run_id: "ROUTE-20260729T120000-deadbeef".to_owned(),
            revision: 1,
            input_fingerprint: "input".to_owned(),
            proposal_revision: 1,
            proposal_hash: "proposal".to_owned(),
            response_revision: 1,
            response_hash: "response".to_owned(),
            generated_at: "2026-07-29T12:00:00+09:00".to_owned(),
            operations: Vec::new(),
            entry_preconditions: BTreeMap::new(),
            unselected_operation_ids: Vec::new(),
            preview_hash: String::new(),
        };
        let first = preview_hash(&preview).expect("hash preview");
        assert_eq!(
            first,
            "e05ca24d0824712481602e85741ce90dd6d270e137c23af11f65594497b0cca2"
        );
        preview.preview_hash = first.clone();
        assert_eq!(preview_hash(&preview).expect("rehash preview"), first);
        preview.proposal_hash = "changed".to_owned();
        assert_ne!(preview_hash(&preview).expect("hash changed preview"), first);
    }

    fn initialized_repository() -> (tempfile::TempDir, Repository) {
        let temporary = tempdir().expect("create temporary repository");
        fs::create_dir(temporary.path().join(".git")).expect("create repository marker");
        let repository = repository::initialize(temporary.path())
            .expect("initialize repository")
            .repository;
        (temporary, repository)
    }

    fn snapshot(repository: &Repository, ids: &[String]) -> RouteInput {
        let ids = ids.iter().cloned().collect::<BTreeSet<_>>();
        let entries = load_route_entries(repository, &ids).expect("load Route entries");
        let goal_coverage = load_goal_coverage(repository, &entries).expect("load Goal Coverage");
        let evidence_fingerprint = evidence_fingerprint(repository).expect("fingerprint Evidence");
        let coverage_basis_fingerprint =
            coverage_basis_fingerprint(repository).expect("fingerprint Coverage basis");
        RouteInput {
            schema_version: ROUTE_SCHEMA_VERSION,
            artifact_type: "route-input".to_owned(),
            run_id: "ROUTE-20260730T000000-recovery".to_owned(),
            revision: 1,
            primary_seed: ids.iter().next().expect("primary seed").clone(),
            included_ids: ids.iter().skip(1).cloned().collect(),
            generated_at: "2026-07-30T00:00:00+09:00".to_owned(),
            entries,
            goal_coverage,
            evidence_fingerprint,
            coverage_basis_fingerprint,
            fingerprint: "test-fingerprint".to_owned(),
        }
    }

    #[test]
    fn interrupted_apply_reconstructs_changed_and_unchanged_link_receipts() {
        let (_temporary, repository) = initialized_repository();
        let source = store::create(
            &repository,
            EntryType::Decision,
            "Receipt source".to_owned(),
            "Source body.".to_owned(),
        )
        .expect("create source");
        let target = store::create(
            &repository,
            EntryType::Goal,
            "Receipt target".to_owned(),
            "## Success Criteria\n\n- [SC-001] Resume safely.".to_owned(),
        )
        .expect("create target");
        let input = snapshot(
            &repository,
            &[source.display_id.clone(), target.display_id.clone()],
        );
        let operations = vec![
            ProposedOperation::Link {
                operation_id: "op-changed".to_owned(),
                from: source.display_id.clone(),
                to: target.display_id.clone(),
                relation: LinkRelation::Supports,
            },
            ProposedOperation::Link {
                operation_id: "op-unchanged".to_owned(),
                from: source.display_id.clone(),
                to: target.display_id.clone(),
                relation: LinkRelation::Supports,
            },
        ];
        let changed = store::route_link_if_revision(
            &repository,
            &input.run_id,
            "op-changed",
            "preview",
            &source.display_id,
            &target.display_id,
            LinkRelation::Supports,
            source.revision,
        )
        .expect("commit changed link receipt");
        let unchanged = store::route_link_if_revision(
            &repository,
            &input.run_id,
            "op-unchanged",
            "preview",
            &source.display_id,
            &target.display_id,
            LinkRelation::Supports,
            changed.post_revision,
        )
        .expect("commit unchanged link receipt");
        assert_eq!(changed.outcome, MutationOutcome::Changed);
        assert_eq!(unchanged.outcome, MutationOutcome::Unchanged);
        verify_unmutated_input_current(&repository, &input.run_id, "preview", &input, &operations)
            .expect("resume from changed and unchanged receipts");
    }

    #[test]
    fn interrupted_apply_rejects_receipt_external_inbound_changes_for_mutable_targets() {
        let (_temporary, repository) = initialized_repository();
        let goal = store::create(
            &repository,
            EntryType::Goal,
            "Mutable target".to_owned(),
            "## Success Criteria\n\n- [SC-001] Reject stale state.".to_owned(),
        )
        .expect("create goal");
        let external = store::create(
            &repository,
            EntryType::Decision,
            "External source".to_owned(),
            "External body.".to_owned(),
        )
        .expect("create external source");
        let input = snapshot(&repository, std::slice::from_ref(&goal.display_id));
        store::link(
            &repository,
            &external.display_id,
            &goal.display_id,
            LinkRelation::Supports,
        )
        .expect("add external inbound link");
        let operations = vec![ProposedOperation::SetStatus {
            operation_id: "op-status".to_owned(),
            target: goal.display_id.clone(),
            status: EntryStatus::Active,
        }];
        let error = verify_unmutated_input_current(
            &repository,
            &input.run_id,
            "preview",
            &input,
            &operations,
        )
        .expect_err("external inbound link must make resume stale");
        assert!(
            error.to_string().contains("inbound links"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn interrupted_apply_rejects_coverage_basis_drift() {
        let (_temporary, mut repository) = initialized_repository();
        let goal = store::create(
            &repository,
            EntryType::Goal,
            "Coverage target".to_owned(),
            "## Success Criteria\n\n- [SC-001] Preserve basis.".to_owned(),
        )
        .expect("create goal");
        let input = snapshot(&repository, std::slice::from_ref(&goal.display_id));
        repository.config.verify.stale_after_days += 1;
        let operations = vec![ProposedOperation::SetStatus {
            operation_id: "op-status".to_owned(),
            target: goal.display_id,
            status: EntryStatus::Active,
        }];
        let error = verify_unmutated_input_current(
            &repository,
            &input.run_id,
            "preview",
            &input,
            &operations,
        )
        .expect_err("Coverage basis drift must reject resume");
        assert!(error.to_string().contains("freshness basis"));
    }
}
