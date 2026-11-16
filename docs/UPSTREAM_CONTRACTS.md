# Upstream contract validation

The retained Python tests replay minimal, captured npm, OpenAlex, and crates.io
responses through the historical adapters without network access. The native
Rust command also replays these fixtures through the actual Rust adapters. Fixtures record
the capture time, public endpoint, fixed query, and sanitization policy. They
retain the first three public catalog records and only selected parser fields;
request/response headers, publisher identities, credentials, and raw errors
are not retained. These fixtures catch parser regressions, not subsequent
upstream drift. Never refresh fixtures automatically to accept a failing test.

For deterministic offline replay of the native adapters, run:

```sh
cargo xtask upstream-contracts --output /path/to/upstream-report.json
```

This default mode uses only the captured fixtures; it makes zero network
requests. For an explicitly authorized live check, add `--live`:

```sh
cargo xtask upstream-contracts --live --output /path/to/upstream-report.json
```

The native probe uses actual Rust adapter request construction and parsing. It
allows only three fixed HTTPS hosts, paths, and query sets; one request per
source; no retries or redirects; no environment proxies or credentials; a
10-second async deadline per source; identity encoding; and a 1 MiB response
limit. It requests three results each. The deadline cancels asynchronous send
and body reads; the fixed 1 MiB input bound limits synchronous parsing work.
OpenAlex retains its native Rust adapter user agent, so identical headers are
not claimed. Live probes should run only when needed; the included schedule is
weekly and remains disabled unless its repository variable is explicitly set.

The JSON report distinguishes `passed`, `contract_failure` (successful HTTP
with malformed/empty/unusable results, invalid latency, or a safety bound), and
`upstream_unavailable` (network failure, timeout, or non-success HTTP). Exit codes
are respectively 0, 1, and 2, with contract failure taking precedence. A failed
live job is a triage signal, not proof of a SlopSearX defect. Check the report
before changing adapters. Blocked and rate-limited responses cannot establish
ranking quality. Positive latency means the adapter recorded timing; it is not
a benchmark. Usable URLs mean syntactically usable links, not that each target
was fetched. No independent relevance judgment or ranking comparison is made.

The **Upstream contracts (opt-in)** GitHub workflow's manual `run_live` input
defaults to false. The weekly scheduled job runs only when repository variable
`LIVE_UPSTREAM_CONTRACTS=true`; this change does not enable that variable. Live
checks are separate from required PR CI and upload the machine report even on
failure. No secrets are required. Pull requests and the default local command
remain deterministic and offline.

To prepare new evidence for review, add `--capture-dir PATH` to the native command.
Only successful probes write allowlisted payloads. Live captures are marked
with `mode=live` and a `captured_at` timestamp. Offline fixture replays are marked
with `mode=offline_fixture_replay` and `generated_at`; they also preserve the
fixture path and its original source capture time. Offline replay output is not
new provider evidence. Use `--live` only when an explicitly authorized live
check is needed, and inspect the capture directory before copying new live
captures into `tests/fixtures/upstream_contracts`. Review source schema changes
and ensure no unnecessary personal fields were retained. Fixture sanitization
intentionally omits abstracts and npm popularity metadata; crates download
counts are retained because its parser uses them. Ordinary unit tests remain
necessary for adapter branches whose fields were omitted. The frozen Python
script/test and historical source inventories remain byte-identical migration
evidence until the separately reviewed G08 source-retirement work.

See [`docs/rust-rewrite/o02/UPSTREAM_CONTRACTS_TRANSITION.json`](rust-rewrite/o02/UPSTREAM_CONTRACTS_TRANSITION.json)
for the exact source hashes, parity limits, validation evidence, and remaining
O02 scope for this transition.

The Internet Archive adapter's domain path first queries the Wayback CDX API. If
that request fails or times out, it makes at most one bounded request to the
official Availability API for the requested domain. A valid `closest` record
produces exactly one result (for example, a `whitehouse.gov` snapshot), with an
HTTPS-normalized `web.archive.org` URL. It is explicitly described as one
closest snapshot, not as a CDX history. Missing snapshots remain an honest
empty result; malformed or untrusted returned URLs, blocked responses, and
timeouts remain classified failures. The CDX and fallback calls share one
aggregate timeout, with a reserved budget for the fallback. See the Internet
Archive's [official Wayback developer resources](https://archivesupport.zendesk.com/hc/en-us/articles/360001495812-Developer-Resources)
for the upstream API reference.
