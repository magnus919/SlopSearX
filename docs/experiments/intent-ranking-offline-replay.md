# Offline intent-ranking replay harness

**Diagnostic only.** `scripts/intent_ranking_replay.py` replays externally pinned request and raw-response artifacts through the offline coverage prototype. It has no provider, credentials, runtime admission, clock, or execution path.

The caller supplies the expected manifest SHA-256, stage UUID, source revision, and ordered operation inventory. The manifest has an exact schema and is checked against its SHA before parsing. Each operation contains only its query, purpose, facet definitions, candidate projection, incumbent permutation, request-body digest, and response-receipt digest. The harness recompiles every request through `intent_ranking_coverage`, checks every request digest and binding, and verifies the complete sealed response inventory before invoking any response parser or selector.

Malformed response bodies retain the prototype's whole-incumbent fallback as a diagnostic result. Any manifest, request, inventory, or receipt mismatch rejects the entire replay without returning partial rankings. An optional caller-sealed result digest can pin the deterministic output.

The `source_revision` input is only a binding label supplied and checked by this harness. It does not establish that the current source tree, dependencies, or runner match that revision. Any registering runner must independently verify and qualify those source bytes and external pins. This harness creates no authority from a manifest or archive.

Focused synthetic tests: `pytest tests/test_intent_ranking_replay.py`.
