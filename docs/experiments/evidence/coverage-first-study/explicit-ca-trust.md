# Explicit CA trust option for owned capture

`capture_sources_once` can optionally receive public CA PEM bytes and an
out-of-band SHA-256. The bytes are bounded, hash-checked, restricted to PEM
certificate blocks, and rejected if they contain a private-key marker before
permit verification, lease consumption, transport creation, or dispatch. The
owned HTTPX transport receives an `SSLContext` built only from that bundle;
certificate-chain and hostname verification remain required. Without the
option, the transport continues to use HTTPX's unchanged system CA defaults.

The CA digest is passed to the external capture-permit verifier, is part of the
protected-capture qualification bindings, and is retained in the private
execution-control inventory. The resource collector checks those bindings and
reports the trust mode and digest. A mocked transport cannot claim this trust
path. These bytes are not a credential or a signature: the expected digest and
qualification receipt remain operator-supplied, and this option does not itself
qualify the candidate fetch boundary. Real capture remains subject to the
separate source-boundary qualification and admission process.
