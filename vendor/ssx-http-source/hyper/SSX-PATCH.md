# Local T07 source modification

This is locally modified hyper 1.12.0, not the unchanged registry release. Upstream archive SHA-256: `2c3e324da4c95177d6291d4c8730197c0d1822f8a9766814a4a44fa5ab797c9c`. Original MIT license and copyright notices remain in the package. `../PROVENANCE.json` records every local file digest, the complete tree digest and the retained complete patch at `../patches/hyper.patch`. These repository-relative files identify the fork without an external review packet.

This proposal retains the reviewed nondefault HTTP/1 send-operation extension, request-local timeout propagation and terminal cancellation ownership. It adds the nondefault ssx-http1-source-read feature: a typed, explicitly selected per-plaintext-read option and per-frame body demand. The latter is a candidate only, not activated or qualified in the repository. No socket buffer controls, receipt collectors or TLS fixture dependencies are added. Unopted requests retain their existing paths.

Before updating, verify a newly reviewed upstream archive, inspect source semantics, rebase the minimal patch, regenerate provenance, and run causal empty-event/trailer/cancellation/write-error/redirect/retry/TLS controls plus all affected-consumer and platform gates. Independent review and maintainer acceptance are required before accepting changed digests. Release SBOMs must include both upstream archive and local patch/tree identities.

V2 read-slice candidate additionally proposes terminal opted-read cancellation/error owner destruction without shutdown IO, and clears clocks before internal idle probes. All native Hyper/reqwest qualification remains unexecuted.

V3 read-slice candidate adds a same-pass post-write profiled receive poll inside the existing bounded loop, after cancellation checks, to guarantee the first Headers read even without an extra transport wake. No default path or send method is changed. It is unexecuted.
