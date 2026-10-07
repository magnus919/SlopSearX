# EXP-048 registration review

**Review result:** One registration blocker remains: define the delivery manifest whose fingerprint is required in reviewer tasks and output.

The six listed EXP-047 source pins all match the current artifacts. EXP-047's pinned execution record and raw A/B outputs confirm both reviewers returned only truncation/inability messages; there are no reusable judgments. The original packet is still six pools/twelve cases, two cases per pool. EXP-048 explicitly retains this sample and contract, with no gate relaxation, relabeling, production claim, or narrowing of the SlopSearX plus experimental GroktoCrawl X objective.

The lossless per-pool file design is a reasonable bounded repair: reviewers must consume all six complete pools before judging all twelve cases; individual byte/word/contract limits, both nested output caps, strict parse/content/end-marker checks, nonretrying incomplete behavior, and the execution-attestation limitation are explicit. The same answerability/preference semantics, references, rationale limits, no-call restrictions, and prospective-only direction screen are retained.

**Blocker:** The review JSON requires `delivery_manifest_sha256`, and the reviewer task is supposed to supply that fingerprint, but the registration does not define a manifest artifact, schema, canonical bytes/serialization, ordered entries, or exactly which file hashes it contains. It says to hash the delivery files/contract/builder/validator/qualification receipt, but does not specify how those hashes are bound into the output field. Before implementation or reviewer invocation, define and freeze a deterministic manifest (including six ordered logical filenames and exact byte hashes, packet/contract hashes, and builder/validator/qualification hashes), then specify its canonical serialization and hash. Otherwise reviewers cannot bind their outputs reproducibly to the exact six-file delivery and qualification.

Reviewed SHA-256:
- Registration: `a0d94caece22a1cf063bca86a6356d4d13c9995564a1c9caef91fa59cdb3c8d4`
- EXP-048 source pins: `844d613039a85248ad4882082cadef08da5a287aaa606f6c32a021f504a4631a`
- EXP-047 packet: `1e69910780a1db2eccfdb10dd9aa2aa41e410de71347e79323b4e4b85db9b6a2`
- EXP-047 contract: `b35fa3a53ab7e8358ebc38e6bf5574dde6d2bdcc215e6fd357a4588966f5d7a5`

No reviewer, provider, search, or browser call was made; no repository files were edited.
