# Recorded engine replay fidelity check

A private archive of public HTTP responses was replayed through the current
GitHub, arXiv, OpenAlex, and Wikipedia adapters using the strict offline
transport. The replay consumed all 31 archived HTTP scenarios with no unused
scenarios. The adapter layer made 29 searches: 13 GitHub, 4 arXiv, 6 OpenAlex,
and 6 Wikipedia. Each archived exchange had status 200, and each replayed
adapter returned `ok`.

All 29 replayed result sets exactly matched the corresponding archived
canonical pool result sets, totaling 463 parsed results. The replay transport
also verified every archived response-body digest against its local capture
metadata. This is parser and response-body fidelity evidence for this fixed
archive only; it is not a claim about provider freshness, rankings, future live
behavior, or release qualification. It earns no retrieval-quality credit.

The archive did not preserve complete request headers or original query
metadata, so requests were reconstructed from the frozen acquisition plan and
current adapter request builders. This check therefore does not establish
byte-for-byte original-request identity. Only aggregate counts and outcomes
are recorded here; the captured response bodies, query inputs, and capture
identifiers remain private.
