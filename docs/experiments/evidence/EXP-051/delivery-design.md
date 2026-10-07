# EXP-051 enriched visible-support delivery: implementation plan

## Current state and boundary

This is the prospective delivery specification. Record the registration commit before implementing it. No builder or qualification code has been created or run, no candidate delivery packet has been generated, and no reviewer, metadata, search, provider, or model call has occurred. Implementation must wait for the registration and its exact inputs, schemas, limits, and hashes to be committed.

The study is a delivery of the six unchanged EXP-048 complete-pool packets enriched with the already captured EXP-050 metadata. It is not a new acquisition, a replacement-selection run, a quality gate change, or production evidence. It must not read EXP-048 reviewer outputs, grounding notes, unblinding material, labels, or outcomes.

## Inputs and identity join

The six EXP-048 packet files each contain a wrapper with `schema`, `pool`, `cases`, and `delivery_end_marker`. The pool contains `pool_ref`, `purpose`, `facets`, `candidates`, and `selected_ids`; each candidate contains exactly `id`, `title`, `url`, and `snippet`. Each packet has two case rows containing `case_id`, `pool_ref`, `removed_id`, `added_id`, and `after_selected_ids`.

The EXP-050 projection contains 423 rows, each with pool label, original card fields, and metadata. Its opaque original card IDs differ from the IDs in the EXP-048 blind packets. Therefore do not join on ID alone and do not expose or persist an ID translation table. Match an EXP-048 candidate to a projection row by exact `title`, `url`, and `snippet` equality, with a unique one-to-one match. Keep the EXP-048 opaque card ID in the delivered candidate and attach only the matched projection metadata. Fail closed for missing or ambiguous triples, reused matches, duplicate input cards, or any mismatch in the three original visible strings. The parent’s source-only inventory reports unique exact triple matches for all 232 candidate rows; this was not a label or outcome analysis and must be reconfirmed by the post-registration qualification against pinned bytes.

Use pool boundaries and order from EXP-048 unchanged. Verify the pool’s exact original selected IDs and every case reference against that pool’s candidate IDs. Preserve purpose, facets, candidates, selected IDs, and case rows losslessly; the only addition is a separately named metadata object alongside each unchanged candidate card. Do not include the EXP-050 card ID, query mapping, scores, orders, eligibility, prior annotations, response receipt, or raw acquisition data.

The parent-provided inventory is: P01 33 candidates (8 matched, 5 unknown, 20 no_identifier); P02 32 (17, 4, 11); P03 35 (17, 4, 14); P04 44 (30, 0, 14); P05 44 (22, 3, 19); P06 44 (23, 4, 17). Totals are 232 candidates: 117 matched, 20 unknown, and 95 no_identifier. These expected counts are structural continuity checks only, not evidence about review outcomes or metadata value.

## Metadata allowlist and privacy

Use the existing registered EXP-050 projection without modifying it. Copy only the allowlisted metadata fields needed for visible-support review: status and reason; `reported_by`; later-lookup provenance; original-engine provenance (which remains explicitly unknown); requested identifier/scope; reported paper and external identifiers; and the bounded reported fields for title, publication date, year, journal, publication types, authors, and abstract. Preserve each field’s existing reported/unknown status, value, clipping flag, original character count, and list count exactly. Do not normalize, summarize, re-clip, enrich, or infer fields. Distinguish the card’s original visible title from the provider-reported title.

The abstract excerpt may appear only in the private delivery directory for reviewers. The tracked builder and qualification scripts contain no captured abstract text. Qualification receipts and the public manifest contain no abstract values, full projection, raw response, response or abstract digest, or private location. Do not copy the projection’s abstract digest or raw transport receipt into the reviewer packet. Omit fields outside the explicit allowlist.

For `unknown` and `no_identifier`, preserve status and reason, plus explicit unknown provenance and fields as specified by the registration. Never synthesize historical source engine, publication identity, authority, study type, or unseen page content. Metadata source identity means reported identifier match only; the card remains a reading lead, not proof.

## Deterministic private chunking

After registration, make a deterministic builder that reads only the six pinned packet inputs and the full pinned EXP-050 projection. Serialize compact UTF-8 JSON deterministically with a single documented canonicalization rule. Chunks must be at most 24,000 bytes as serialized, including the end marker and all repeated context. Keep every original card indivisible; never truncate text or split a card’s metadata. If a complete card plus required wrapper cannot fit, fail closed.

If a complete pool does not fit one chunk, partition its candidate rows in their original order into the smallest deterministic greedy sequence of chunks that all fit. Every chunk repeats the pool’s unchanged purpose, facets, selected IDs, and unchanged two-case list so it remains interpretable; include a fixed schema, pool ref, one-based chunk index, total chunk count, complete card/metadata rows, and a deterministic end marker. Derive the total count before writing final bytes. Reassemble by chunk order and require exact equality to the original full pool and cases after discarding only the added metadata. Each original candidate must appear exactly once across that pool’s chunks.

Write the final packet set to a newly created private destination. Refuse to overwrite any pre-existing destination or child file, including a partial earlier build. Construct in a fresh private temporary directory and atomically rename only after every chunk, byte bound, reconstruction check, and manifest check passes. Treat the completed destination as immutable; a retry requires a newly registered destination/artifact identity rather than in-place replacement.

The public delivery manifest contains only logical chunk filenames, byte hashes, byte lengths, pool refs, chunk indices/totals, candidate-row counts, case counts, and schema. It contains no private host paths, content, IDs from EXP-050, abstract hashes, or response receipt. Bind the manifest, six source packet hashes, full private projection identity, builder, qualification, and public-safe qualification receipt in a separate publication integrity artifact after registration. Keep the private projection identity private if the registration so specifies.

## Qualification after registration

The deterministic offline qualification should be read-only with respect to source inputs and contain synthetic adversarial fixtures plus the actual source-only structural reconstruction. It should verify:

- Exact input hashes, six pool refs, unchanged purpose/facets/selected IDs/candidate order/cases, expected 232-row status inventory, and unique exact triple joins.
- Missing candidate or metadata row; mismatched title, URL, or snippet; ambiguous duplicate triple; duplicate/reused projection row; duplicate candidate ID; invalid status; missing/extra pool; reordered or missing case; and case references to unknown IDs all fail closed.
- `matched`, `unknown`, and `no_identifier` are preserved as-is. Unknown/missing values remain explicit. Original engine provenance stays unknown and later Semantic Scholar provenance remains distinct.
- Abstract excerpt value, whitespace, Unicode, clipping flag and original character count match the registered projection exactly in private chunks; the public manifest and qualification receipt contain no excerpt values. Original acquisition card fields remain unchanged even when provider title differs.
- Compact serialization and every emitted chunk obey the 24,000-byte maximum; all cards remain indivisible; multi-chunk assembly recovers exact original pool/card/case content and every candidate is present once.
- Wrong markers, chunk order/total mismatch, truncation, byte mutation, hash mismatch, extra/unexpected files, and an already existing different output destination cause refusal. Preserve the first failed run receipt rather than overwriting it.
- No network, provider, search, model, credential, grading, or filesystem write outside the fresh private output and qualification receipt is attempted.

The qualification must not inspect prior reviewer responses, private grading artifacts, labels, or any unblinding map. It demonstrates delivery integrity only; it cannot justify the study’s outcome or a production policy. Fresh reviewers and their exact execution protocol remain governed solely by the committed EXP-051 registration.
