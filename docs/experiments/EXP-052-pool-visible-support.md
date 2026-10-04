# EXP-052: one-pool visible-support references

## Purpose and boundary

This is a prospective successor registration for descriptive independent review of visible discovery and facet support. It addresses EXP-051's incomplete two-reviewer outcome by making one complete pool the review unit. It preserves the exact six complete pools, their purposes and facets, selected sets, and all twelve existing replacement cases. It does not change candidate selection, acquire sources, or repair or reinterpret EXP-051 outcomes.

The sole source is the immutable existing EXP-051 delivery with manifest SHA-256 `6b367674480b4245e2511733144a9e477571c92499a8bedb0e143a87d67a47ea`. No source packet or metadata is regenerated, normalized, or reclipped. No source-card content, labels, or prior reviewer results are copied into this registration. The complete review semantics and compact schema are in [the EXP-052 review contract](evidence/EXP-052/review-contract.md).

## Review design

Create twelve fresh independent `gpt-6-luna` invocations: one pool per task and two independent reviewers per pool. Review P01 through P06 in that order, completing each reviewer pair before starting the next pair as slots permit. At most two reviewers run concurrently. Each task receives one pool only and must read every immutable chunk of that pool completely. Each chunk gets its own tool read, in order, with a maximum 16,000-token output allowance per read; chunk reads and files are never combined. Exact terminal markers are mandatory.

Each reviewer assesses exactly the unique union of cards selected across that pool's current set and two after-sets; all facets and both cases are included. This preserves whole-pool context while bounding one final response to one pool. The response uses fixed aliases for the existing EXP-051 enums and field paths, with an exact expansion map in the contract. Facet coverage and case relation are derived deterministically from card-level judgments using the existing EXP-051 rules. Rationale is capped at 160 characters, pointer lists at two, and the full JSON response at 10,000 UTF-8 bytes.

An incomplete or invalid chunk read, malformed response, capacity failure, or contract violation is retained as returned and makes the complete twelve-record primary outcome incomplete. There are no retries, repairs, substitutions, or replacement reviewers. A primary outcome passes only if all twelve independent records are complete and valid. Disagreement is retained without forcing agreement.

## Capacity evidence and limits

A deterministic preflight must construct the maximum-size response envelope separately for each frozen pool, using the exact number of selected-union cards, ordered facets, two cases, maximum allowed pointer counts, longest enum values, and 160-character rationales. It must verify that every envelope fits the 10,000-byte response cap and that every pool chunk can be completely read under the per-call 16,000-token output allowance. Failed or inconclusive preflight stops execution before the first reference.

The existing `capacity-observation.json` records one synthetic Luna invocation whose complete 12-card, 48-facet-row, 2-case response was extracted unchanged and verified byte-identical to its 13,447-byte input. This confirms one successful full-size synthetic response and exact copying in that observation; it does not establish a hard provider completion-token limit or guarantee review success. Its 13,447-byte output exceeds EXP-052's 10,000-byte cap, so it does not qualify this compact schema's cap. Qualification must calculate the exact worst-case compact response envelope for each real pool and show it is at or below 10,000 bytes; the existing observation supplies no guarantee that a model will always satisfy that bound. No known provider output ceiling or safety margin may be claimed. If deterministic envelope generation, separate complete reads, or any pre-reference qualification fails, stop before references. The byte cap is a validator rule, not a claim that provider transport guarantees byte-exact reproduction.

## Qualification and freeze requirements

Before invoking any reviewer, freeze the exact registration, this contract, task text, validator, and immutable manifest identity. Publish qualification evidence showing:

- exact linkage to the six existing pools, twelve cases, ordered facets, selected unions, and pinned manifest, without source changes;
- deterministic maximum response envelope and UTF-8 byte count for every pool, all at or below 10,000 bytes;
- synthetic adversarial parser/validator checks for duplicate keys, missing/extra/reordered IDs and facets, wrong markers, bad pointers, invalid enums, oversized output, and invalid attestations;
- round-trip equivalence for every enum alias and field-pointer alias against the original EXP-051 meanings and paths;
- consistency checks for selected-set support, uncertainty propagation, reading-lead distinctions, and the full derived-relation precedence;
- complete per-chunk read/marker validation using separate reads, each within the 16,000-token tool-output allowance;
- an explicit record of the limits that remain unknown, including the provider's hard completion-token cap and byte-exact copying guarantee.

Qualification must make no model, provider, search, network, or source-content calls. Synthetic data must not encode source judgments or labels. Review execution begins only after qualification and the exact task/manifest bindings are frozen. Preserve failed and successful response bytes without repair.

## Interpretation and delivery boundary

Even twelve valid independent references provide descriptive evidence about judgments on these frozen visible packets only. They do not establish truth, source quality, selector superiority, ranking uplift, generalization, or production readiness. The separately defined production acceptance scope remains the shared SlopSearX service and GroktoCrawl X, with its existing quality gates and implementation checks. EXP-052 alone does not authorize production adoption.
