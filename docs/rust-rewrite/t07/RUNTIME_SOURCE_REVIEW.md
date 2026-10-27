# T07 native runtime source review V2 — uncompiled source

This is a successor to immutable /private/tmp/t07-runtime-source-review-v1, based on fetched origin/main9f1a3fa196c6dcdb966a1fe47b117ad81ebf8010. It is reviewable source, not a passed gate or completed issue902. No Cargo build/test/resolution, dependency download/activation, Python oracle/backend, provider call or external OAuth activation occurred. Cargo.lock is unchanged. Pinned Rust1.99 rustfmt edition2024 parsed/formatted touched leaves; this is not type checking. One SDK formatter edition2021 attempt failed on existing let-chains, then edition2024 succeeded. Graphify AST update is run separately; all semantic tests below remain unexecuted.

## Implemented source contract

The actual native executable composes settings, one owned concrete source-profile HTTP pool, static or OAuth authentication, MCP initialization/discovery, raw schemas and stdio. It constructs no backend engines or Valkey. Startup failure, EOF, stdio failure and Unix signals run explicit callback/file/browser/HTTP/SDK cleanup. Browser child handles remain owned through kill/wait; SDK waiting tasks remain owned through cancellation/join. Response reads select cancellation and preserve source R300, separate C/P/W and source OAuth authorization300. Timeout/profile acceptance remains a separate transport obligation.

FastMCP invocation uses only the original shallow generated annotations: preserve unvalidated optional defaults, first recognized union type, lax primitive coercion, deep raw nested data, unknown-key/required checks and predispatch failures. Dynamic descriptors preserve full inputSchema including unknown extensions, but retain original outer projection and visibility. Source Python Unicode16 title/identifier/cleandoc behavior is implemented from retained source/tables. Invalid tool arguments and unknown tools are CallToolResult.isError, not RPC invalid_params. Remote -32021 remains protocol error; other tool failures become tool results. Resource/prompt MCP errors remain protocol errors; unknown resource carries -32602 and uri data; missing required prompt argument becomes -32603 before dispatch; invalid outer argument types are -32602. Extra prompt arguments are filtered as original.

Tool logical extraction preserves remote-isError precedence, nonnull structured values including empty objects, joined stripped text, legacy JSON or result wrapper. Null gives empty content; strings raw text; scalar/list JSON text; structuredContent only objects. Legacy JSON normalization preserves large integer width (source4300-digit limit), duplicate-last values/insertion order, deep raw structures and nonfinite-to-null wire behavior. Resource text joining ignores blobs; prompts flatten text to one user message. Fixed three resources, one template and five prompts come from actual gateway.py declarations, not its broader opening docstring. No authoritative executed gateway catalog capture exists: this is explicitly SOURCE DERIVED, not an executed oracle.

OAuth uses existing official SDK HTTP injection and lazy original challenge behavior. Minted token_endpoint_auth_method none/basic/post governs secret transmission; Basic follows source percent encoding. Source scope priority, resource/protocol inclusion, issuer exact/root-slash binding, nonportable DCR identifiers, cached source receipt/expiry semantics and non200 refresh fallback are implemented. Store uses original client_info/tokens schema and preserves unknown root fields; network/malformed-store errors do not silently become successful authorization. Metadata/registration/token/refresh are explicitly routed through the same source adapter.

SDK source-policy boundary is separate, UNAPPLIED: /private/tmp/t07-sdk-source-policy-v2. Its predecessor retains23 sanitized diagnostic sites, auth5a158137f05fe766bbc79be29b7130d5041ba74bdb539c3200ad923b1fecf99b. Additive SourceWithinOrigin is selected before first authorization HTTP only by explicit native manager opt-in. Existing default Follow/Stop semantics remain; default SDK HTTP adapter refuses this new profile. Discovery delegates one five-hop same-origin/default HTTPS-upgrade method-preserving policy; registration/code/refresh select it explicitly. MCP transport's separate twenty-hop rule is preserved. Full predecessor patch plus additive patch series and regenerated file identities are provided for review, with no repository activation.

T07 additive URL constructor preserves source userinfo/fragments; existing T06 guard remains unchanged. Wire URLs omit fragments/userinfo, static configured URL Basic takes precedence over default Bearer, OAuth configured authentication takes precedence over URL Basic. Percent decoding uses UTF8 replacement, without an ASCII-only restriction. Original resource canonicalization keeps userinfo/query and textual default-port distinctions. Human authorization prompt/browser intentionally receives the original raw authorization URL, as the original flow does; ordinary diagnostics, errors, telemetry and debug fields must not receive it. Loopback prompt fixtures are synthetic only.

## Accepted differences and evidence limits

Parent accepted native serverInfo.version0.1.0, preserving exact name/instructions/capabilities. Parent accepted version-specific Pydantic diagnostic TEXT differences only, retaining field meaning, error class/channel and predispatch behavior. No other source-domain waiver is claimed. Restoring source URL and interactive URL behavior is preservation, not a new security difference. All claims above describe authored source and source comparison; qualification may expose corrections.

The actual T07 xtask binds exact source digests plus49 component test selectors,3 seam tests and8 actual-process loopback tests. It performs locked/offline actual binary compilation, compiled inventory equality and zero ignored/failing/missing checks. Process fixtures use actual stdio, actual loopback HTTP including OAuth307 metadata/registration/token redirects, PKCE/callback, secure persistence, refresh fallback, static Basic precedence, two instances, startup redaction, EOF cancellation and SIGTERM join. Source/test bindings are explicit source-derived expectations; compiling/running is held for root review. The source-only SDK recording test additionally checks explicit profile selection and unchanged default policies.

## Remaining release obligations

Review this entire source and separate additive SDK policy/patch/provenance before activation. Review exact four-crate575-file HTTP composition and Cargo feature/lock delta; the new runtime deliberately references concrete fork APIs and cannot compile against stock crates. Preserve and complete independent R1/R2 Q1/Q2/Q3, P/C/W/R/mount/platform/trust/proxy/IDNA/current-consumer obligations, not substitute an aggregate timeout or arbitrary50MiB criterion. Run bounded native qualification, fix compiler/runtime failures, complete inherited T01/T05/T06/F08/shared stdio regressions, workspace quality/portal CI, supported target install evidence, substantive independent review, artifact/ledger/ADR updates and ready PR/exact-head CI before delivery. Native/source-policy tests are not a waiver of missing original executed catalog capture.

Portal impact: this client source does not change SearchService or browser contracts. Packaging changes still require portal impact review and existing required portal CI. No Python behavior changed.


## Current registration request fixture

The source OAuth lane sends `application_type: "native"` in its dynamic client
registration request. MCP 2.3.0 `mcp/shared/auth.py:113–130` gives the request
model this default, and `mcp/client/auth/utils.py:292` includes it through
`model_dump(mode="json", exclude_none=True)`. The project constructs that
request model in `slopsearx/mcp/oauth_client.py:219` without overriding the
default. The retained primary model has SHA-256
`899afdfa4f368e89d6672141431b6a32f839f9e38a7f0b3a6e6654cda25200e2`.

This request contract differs from the client-information response model,
whose `application_type` default is `None` (`auth.py:133–159`). The frozen
client URL and token-model response fixtures retain their `null` values.
No intentional wire difference is introduced by correcting the process
fixture: its registration handler now requires the exact string `native`,
while retaining the `token_endpoint_auth_method == "none"` assertion and
all other OAuth, storage, cancellation and shutdown checks.

The accepted source caller already preserves this field, and
`source_oauth_real_loopback_discovery_registration_code_refresh_and_store`
and the genuine SDK source-operation control assert it. The first public
213-case attempt on `4e051784` passed 117 gateway library controls and four
streaming seams, then stopped with seven process passes and five failures
at the obsolete absence assertion. That failed attempt remains failed.
The strict fixture successor and its current binding require focused
execution and a new complete public T07 result; neither is claimed here.
