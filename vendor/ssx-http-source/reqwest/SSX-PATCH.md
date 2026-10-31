# Local T07 source modification

This is locally modified reqwest 0.13.5, not the unchanged registry release. Upstream archive SHA-256: `16a1cfa75cc186dd73d5818e510e042e40927bccc9c236b061cea97e1eb08029`. Original MIT OR Apache-2.0 license and copyright notices remain in the package. `../PROVENANCE.json` records every local file digest, the complete tree digest and the retained complete patch at `../patches/reqwest.patch`. These repository-relative files identify the fork without an external review packet.

The nondefault HTTP/1 send-operation extension, request-local timeout propagation and terminal cancellation ownership are SlopSearX modifications. The fork includes no socket buffer controls, receipt collectors or TLS fixture dependencies. Only ssx-mcp requests the feature; each protected request still explicitly selects its budget.

Before updating, verify a newly reviewed upstream archive, inspect source semantics, rebase the minimal patch, regenerate provenance, and run causal empty-event/trailer/cancellation/write-error/redirect/retry/TLS controls plus all affected-consumer and platform gates. Independent review and maintainer acceptance are required before accepting changed digests. Release SBOMs must include both upstream archive and local patch/tree identities.

UNAPPLIED READ-SLICE PROPOSAL: the nondefault ssx-http1-source-read feature adds RequestBuilder::source_http1_timeouts(write, read), requiring an HTTP/1-only client. It forwards the typed option through accepted redirects and bypasses existing reqwest header/frame read timers only on explicitly opted requests. It preserves total timeout behavior; the future gateway must not install a whole-OAuth cap. This is not connect/TLS/pool policy, repository activation or qualification.

V2 read-slice candidate additionally proposes terminal opted-read cancellation/error owner destruction without shutdown IO, and clears clocks before internal idle probes. All native Hyper/reqwest qualification remains unexecuted.
