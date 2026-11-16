# A05: DuckDuckGo and Google HTML adapters

Issue #925. Source baseline `01876639fb8d87123f12293c02821981cff3f8b9`;
accepted prerequisite A01 PR #969 merge
`025ab2f2d50febcb573ba62579eecf7e2d07ef3a`, handoff
[#917 comment](https://github.com/magnus919/SlopSearX/issues/917#issuecomment-6212053625).

The native implementation lives in `crates/ssx-infra/src/adapters/duckduckgo.rs`
and `google.rs`. `register_html` installs executable F06 registry factories;
it does not change catalog `ported` flags. Callers own `Arc<AdapterHttp>` and
`SearchAdmission`, including F10 policy wiring. Each search owns a fresh F08
cookie jar. F08 connection pools and proxy rotation remain shared primitives.
G01 owns aggregate registration, catalog and cross-surface qualification.

## Evidence and reproduction

`corpus.json` retains the original 115 offline source observations byte-for-byte.
`corpus-review.json` appends 43 actual-source observations from independent review,
for 158 mandatory replay cases. Both retain: raw response bytes,
status and headers, exact resolved config, source exception type/message,
complete HTTPX requests, and the complete normalized AdapterResponse and
SearchResult dataclasses. Only measured latency is omitted. No output fields
are projected away: media width/height/duration nulls, extended response lists,
synthetic status, payload and work-group absence remain asserted.

`coverage.json` maps 50 original named-engine and inherited pooling regressions
to native cases or reviewed transport ownership. Source tests under
`test_adapters.py`, `test_google_adapter.py`, `test_duckduckgo_resilience.py`,
`test_media_contract.py`, `test_adapter.py` and `test_http_pooling.py` are included. F08 retains
transport-wide concurrency, environment proxy, TLS and pool limit coverage;
A05 adds engine-specific request order and cookie/fallback/cancellation tests.
The gate verifies exact native inventory, no ignored/filtered tests, and every
corpus ID emitted after successful full-output/request comparison.

```sh
cargo xtask check A05 --output target/rust-evidence/A05.json
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --no-run
```

Actual loopback tests observe every engine request. Warm cookie/fallback tests
explicitly issue and assert a separate `/warm` request before the six search
requests, warming shared sockets without carrying its cookie jar into search.
The original cold-bootstrap integration control used paused Tokio time with a
System HTTP owner. Its source and historical 6 ms timer-wheel witness are retained
in `source-amendments/3f1b9cca-cold-bootstrap.rs.frozen`; they are not current
owner-clock evidence. The explicit relocation in
`source-amendments/cold-bootstrap-owner-clock-v1.json` preserves all thirteen
logical controls as twelve integration tests and one private helper unit, alongside
the seven unchanged helper controls. The cold successor uses injected pending I/O
and the same private Manual HTTP owner for bootstrap deadline construction and
expiry. It requires no expiry at 4 ms and expiry/drop at exactly 5 ms, holds that
owner at 5 ms while search/parser continuation completes, and preserves success,
fallback and exact GET/POST order across five requests and two fresh searches.
Tokio/parser clocks remain System and the production 5 ms clamp is unchanged.
Its no-cookie requests do not establish disposal of a late Set-Cookie response:
the pending bootstrap never produces one. Successful-cookie propagation and
isolation remain required in the existing helper controls. Cancellation
aborts a real loopback bootstrap and verifies no subsequent search/fallback.
The loopback proxy test routes requests for `provider.invalid` through local
listeners, verifies absolute method/target/query and proves failed-route
cooldown by observing the third search remain on the healthy route. No provider
DNS, live requests, browser automation, proxy probes or deployment are used.

The temporary exporter imports original backend code solely as an offline
migration oracle. It runs with CPython 3.14.7, HTTPX 0.28.1, lxml 6.1.1 and
cssselect 1.4.0. Native tests and gates never invoke it. G08 retires its executable
Python; retained portable JSON is historical evidence. Independent recapture:

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a05/oracle/export.py /private/tmp/a05-independent.json /private/tmp/a05-independent-review.json
```

## Preserved source details

- DDG bootstrap literally computes `min(bootstrap_timeout_ms, 5.0) / 1000`;
  the default is **5 ms**. This surprising behavior is preserved.
- DDG text redirect handling keeps percent encoding and trailing query fields;
  lite/image handling decodes `uddg` with Python parse_qs empty-value behavior.
- A primary rate limit/timeout is terminal. Blocks, parse failures and unusable
  pages can fall back once for text; images never use lite. Fallback rate limit,
  timeout, blocked/empty and failed parsing retain distinct source messages.
- Google valid empty element markup is OK. Empty documents, whitespace,
  declarations, processing instructions and closed/unclosed comment-only bodies
  raise the source `Document is empty` failure. The pinned html5ever tokenizer
  distinguishes these from valid text/entities/elements. Element-scoped CSS
  selectors include their context node, preserving lxml results, snippets and
  request/fallback counts. Both adapters retain position gaps and selector order.
- `max_results` uses lossless JSON to preserve missing default versus null,
  boolean, float, negative, zero, string, list and object semantics. Comparison
  errors occur only while visiting candidate nodes; null on a zero-candidate
  page remains a valid empty result. Google sends Python-formatted `num`.
- Charset decoding preserves Python ASCII and Latin-1 behavior instead of the
  WHATWG alias mapping to Windows-1252. UTF-32 BOM and explicit little/big-endian
  decoding preserve source titles/results. Raw bytes remain in both captures.
- Python quote_plus encoding preserves exact `*`/`~`, spaces, literal percent
  and Unicode query/form bytes instead of WHATWG form encoding.
- User Agent, browser navigation/form headers, form body bytes, language and
  query parameters are asserted, including same-origin/site/cross-site hosts.
  Paging/language/safe-search/time-range inputs are ignored exactly as source;
  caller presentation max_results never replaces engine config max_results.

## Intentional differences and limits

Two exact differences are retained in the source fixtures and asserted as
specific native expectations in the replay test:

1. `google_network`: F08's typed `Network` error omits HTTPX ConnectError's
   arbitrary exception text (`source oracle network`). Status remains error,
   no retry occurs, and source's absence of proxy failure feedback is preserved.
   No exception text from providers is copied into this diagnostic; URL/status
   diagnostics that F08 does retain are compared exactly for HTTP failures.
2. `ddg_cookie_continuity`: a host-only cookie set by `duckduckgo.com` is not sent
   to `html.duckduckgo.com`. Python HTTPX sent it. The accepted F08 jar follows
   [RFC6265 section 5.4](https://www.rfc-editor.org/rfc/rfc6265#section-5.4).
   Parent explicitly approved retaining that restriction. A separate Domain
   cookie does cross subdomains; same-host continuity, unrelated-host isolation
   and fresh-search isolation are separately tested. Possible live DDG impact
   remains unverified pending authorized provider qualification.

`corpus-preflight.json` retains the first exporter run, which failed to provide
fallback responses for zero/negative result limits. It is diagnostic evidence,
not an acceptance oracle. See `first-failures.md` for corrections. No baseline
expected output was edited to fit native code.

Portal impact: this adds native engine implementations behind accepted shared
contracts. No Python portal route/HTML contract changes. F06 eligibility and
filter metadata are unchanged. G01 still must qualify complete service/portal
composition. Offline success does not establish current provider availability,
relevance, CAPTCHA bypass, production integration or performance improvement.

Implementation SHA, independent review, exact-head CI and final acceptance are
owned by the parent delivery task; this document does not certify completion.

Independent review requested four source corrections; all have appended whole-
search source captures and exact native assertions. `correction-review.md`
records the findings, fixes, unchanged original corpus identity and dependency
edges. Original source expectations and the two approved exceptions are intact.
