# A01 registry-search pilot

Issue [#917](https://github.com/magnus919/SlopSearX/issues/917) ports the npm
and crates.io source adapters through the accepted F08 HTTP session and the F06
registry. The executable `ssx_application::SearchEngine` port is re-exported as
`ssx_registry::Engine`; the older application `Engine::probe` remains only a
foundation readiness probe. The registry depends on the application contract,
while application remains independent of registry, config and infrastructure.

`register_pilots` takes caller-owned `Arc<AdapterHttp>` and
`Arc<dyn SearchAdmission>` handles. The composition root retains the HTTP
client for shutdown and the shared F10 rate limiter for cooldown, reactivation
and shutdown. It uses the public `f10_monotonic_seconds()` clock when calling
`RateLimiter` cooldown APIs, so stored deadlines share the bridge's process
epoch. The explicit `AllowAllAdmission` mode is for configurations with
no limiter and deterministic tests. `F10Admission` wraps the accepted shared
`RateLimiter`; a denied/cooldown/deactivated decision returns `rate_limited`
before HTTP dispatch. Store failure remains `unavailable` under F10 fail-closed
mode and allows dispatch under fail-open mode. This is a bridge to the accepted
quota policy, not a second quota implementation.

The adapters use per-search F08 sessions, default TLS/proxy/environment
behavior, caller-owned transport lifetime, and their configured provider
timeout. Dropping the returned search future cancels its active F08 operation;
the loopback regression verifies the peer observes socket closure. The current
search port does not receive an absolute remaining-deadline/cancellation
context. `SearchRequest.interactive_timeout_ms` is the original request policy,
not a remaining budget after prior work, so F14 orchestration must pass a
remaining budget or extend the pure application context before claiming an
end-to-end deadline bound. Session cookies are local to each search; F06
`max_results` sets both provider request size and the local slice, and request
page values remain ignored as in the source adapter.

## Source-to-native regression map

| Python behavior | Native evidence |
| --- | --- |
| npm `text`/`size`, crates `q`/`per_page`; configured URL query is replaced by HTTPX params | `registry_factories_execute_search_via_f08_loopback_transport` parses captured loopback query pairs |
| Configured `max_results` bounds provider request and local slice; `page` is ignored | Same loopback case passes a conflicting request max/page and asserts provider parameters and two results |
| npm payload envelope includes only truthy source fields and ordered provenance; crates has no payload | Loopback assertions compare npm data/provenance fields and require no crates payload |
| Python stringification of null name/version; numeric-string and bool popularity coercion | `npm_source_edge_cases_match_captured_python_parser_observations`; observations are in `cases.json` |
| Python `str()` for nested list/dict package names and publisher usernames; exact comma grouping for raw integers larger than `u64` | `npm_nested_python_container_stringification_matches_captured_python_observation` and `crates_download_rendering_matches_offline_python_observations`; both preserve lossless JSON number text |
| Python truthiness for publisher usernames, empty descriptions, and popularity containers; `float()` whitespace, digit separators, and Unicode decimal strings | `npm_truthiness_matches_captured_python_observations` and `npm_popularity_strings_match_captured_python_float_observations`; falsey publisher values are omitted and falsey popularity is zero |
| crates.io recent-download score and comma-grouped total downloads | `registry_factories_execute_search_via_f08_loopback_transport` asserts both from the captured fixture |
| crates.io integers retain exact raw digits while floats normalize through Python-compatible `f64` parsing/formatting, including trailing-zero, rounding, underflow, negative-zero, scientific notation, boolean, null, string, and empty-description cases | `crates_download_rendering_matches_offline_python_observations` executes captured observations plus raw HTTP body lexemes |
| Missing `objects`/`crates` means empty results; explicit null and wrong outer types are errors | `missing_result_collections_are_empty_but_null_and_wrong_outer_types_fail` |
| Both providers cover valid empty arrays, malformed JSON and row shapes, 429, 404, 503, request timeout, invalid base URL, and dropped-call cancellation | `cases.json` maps each provider case to native loopback tests |
| Public npm/crates requests carry no authorization; request filters and media fields remain unforwarded and result media stays empty | `registry_factories_execute_search_via_f08_loopback_transport` checks request headers, query pairs, and media |
| Rate denial happens before HTTP dispatch; F10 store failures preserve fail-open/fail-closed classifications | `f10_rate_denial_from_registry_adapter_dispatches_no_provider_request`, `f10_store_failure_preserves_fail_open_and_fail_closed_classification` |
| F10 cooldown remains fenced across adapter-bridge handles sharing one limiter | `f10_admission_handles_share_the_limiter_clock_and_cooldown_state` |
| Composition-set cooldown uses the same public clock and prevents dispatch | `composition_cooldown_uses_the_public_shared_f10_clock_and_blocks_dispatch` |
| Dropped calls cancel without retrying a request | `dropping_registry_search_cancels_the_outstanding_f08_socket` |

Tests use captured source payload fixtures or synthetic loopback responses only.
The native lane performs no provider calls or deployment. Its qualification
scope is the npm and crates.io offline source contract covered by the mapped
fixtures and regressions; it is not live-provider certification or parity
qualification for any remaining engines. Source observations were collected
offline from the existing Python adapters using MockTransport; the native gate
pins the source files and executes the Rust regressions.
