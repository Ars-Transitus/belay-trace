//! Mechanical checks shared by preview-based mutations.
//!
//! This module intentionally stops short of defining a receipt engine. Route
//! receipts currently depend on Route's database schema, operation recovery,
//! and postcondition reconstruction. A common receipt identity should only be
//! introduced when another real apply path has the same persistence contract.

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::BelayError;

/// Hash the exact JSON bytes used to bind a preview to later approval.
pub(crate) fn content_digest<T: Serialize>(value: &T) -> Result<String, BelayError> {
    let bytes = serde_json::to_vec(value).map_err(|error| BelayError::Validation {
        message: format!("could not serialize mutation content: {error}"),
    })?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Compare a recomputed content digest with the digest embedded in a preview.
pub(crate) fn digest_matches<T: Serialize>(
    value: &T,
    embedded_digest: &str,
) -> Result<bool, BelayError> {
    Ok(content_digest(value)? == embedded_digest)
}

/// Read the revision bound to a concrete mutation target.
pub(crate) fn revision_precondition(
    preconditions: &BTreeMap<String, u32>,
    target: &str,
) -> Option<u32> {
    preconditions.get(target).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct ExamplePreview<'a> {
        operation_id: &'a str,
        target: &'a str,
        revision: u32,
    }

    #[test]
    fn content_digest_rejects_replay_with_changed_mutation_content() {
        let approved = ExamplePreview {
            operation_id: "status-1",
            target: "GOAL-1",
            revision: 3,
        };
        let digest = content_digest(&approved).expect("digest approved preview");
        assert!(digest_matches(&approved, &digest).expect("verify approved preview"));

        let tampered = ExamplePreview {
            operation_id: "status-1",
            target: "GOAL-1",
            revision: 4,
        };
        assert!(!digest_matches(&tampered, &digest).expect("reject tampered preview"));
    }

    #[test]
    fn revision_precondition_does_not_fall_back_to_another_target() {
        let preconditions = BTreeMap::from([("GOAL-1".to_owned(), 3), ("PLAN-1".to_owned(), 7)]);

        assert_eq!(revision_precondition(&preconditions, "GOAL-1"), Some(3));
        assert_eq!(revision_precondition(&preconditions, "GOAL-2"), None);
    }
}
