# Offline coverage engine replay

`scripts/coverage_engine_replay.py` provides a small, fail-closed HTTPX replay
transport for the four search adapters used by the frozen coverage collector:
GitHub, arXiv, OpenAlex, and Wikipedia. Tests inject its `MockTransport`
through the adapters' existing transport hook, so the actual adapter parsers
receive the replayed HTTP responses. A replay never creates an ordinary
network transport or falls back to live access. An unmatched request raises a
replay mismatch, which the adapter reports through its normal error result.

Each canonical bundle has a schema version and an externally checked SHA-256.
Every HTTP body has its own SHA-256; requests are matched exactly on method,
HTTPS origin/path, and normalized query, with optional selected-header checks.
Scenarios are consumed once, in order. Timeout, connection error, incomplete
response, HTTP status, and malformed-body outcomes are explicit scenarios.
Credential-like query/header names (including common API key, token, secret,
and authorization spellings) are rejected, and bundle request/response headers
must use narrow benign-header allowlists. This deterministic name check is not
a general secret scanner: arbitrary response-body bytes are not inspected.
Anyone preparing a bundle must sanitize its bodies before pinning it. The
replay receipt lists consumed and unused scenario IDs and always sets
`quality_credit` to false.

Bundles distinguish `recorded-replay` from `synthetic-scenario`. A recorded
bundle must name a capture ID, capture time, and source digest; replaying it
tests parser compatibility with those archived bytes only. Synthetic examples
exercise response shapes and failure paths but are not observations of a
provider. The included tests author synthetic examples and make no claims about
provider behavior, relevance, ranking, or coverage.

This is a manually refreshed digital model of a narrow HTTP boundary, not an
automatically synchronized digital twin. The manual synchronization is the
explicit capture-and-pin step that supplies recorded bundles. Parser and
scenario tests verify transport handling and implementation behavior; they do
not validate freshness, provider fidelity, retrieval quality, or the result of
a future live run. Actual live validation remains the release oracle for
provider-facing claims. Simulated responses cannot qualify a release or earn
ranking-quality credit.
