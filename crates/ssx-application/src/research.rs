//! Research identity and settlement policy; execution belongs to W02.
use sha2::{Digest, Sha256};
use ssx_domain::{
    DomainError, SearchResponse, WallClockTimestamp,
    research::{LeadId, ResearchJob},
};

/// Source lead-v1 identity: SHA256 of the shared normalized URL's UTF-8 bytes.
/// Metadata and source engine never affect identity. Empty and malformed URLs
/// retain the accepted normalizer's exact fallback semantics.
pub fn lead_id(url: &str) -> LeadId {
    let digest = Sha256::digest(crate::normalise_url(url).as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    LeadId::new(format!("lead-v1-{hex}")).expect("SHA256 produces a canonical lead identity")
}

/// Finalize source response evidence using actual result URLs. The durable
/// domain transition charges duplicate admissions and caps results; only new
/// identities deduplicate. Call within the authoritative store mutation after
/// setting the query's outcome/cursor/provenance. No response means no admission.
/// Carry the attempt identity captured before dispatch into this callback; an
/// identity mismatch aborts the authoritative mutation without persisting fields.
pub fn finish_attempt(
    job: &mut ResearchJob,
    index: usize,
    expected_attempt: &ssx_domain::AttemptId,
    response: Option<&SearchResponse>,
    now: WallClockTimestamp,
) -> Result<(), DomainError> {
    let leads = response.map(|response| {
        response
            .results
            .iter()
            .map(|r| lead_id(&r.url))
            .collect::<Vec<_>>()
    });
    job.finish_attempt(index, expected_attempt, leads.as_deref(), now)
}
