# Upstream contract validation

Normal `pytest` runs replay minimal, captured npm, OpenAlex, and crates.io
responses through the actual adapters without network access. Fixtures record
the capture time, public endpoint, fixed query, and sanitization policy. They
retain the first three public catalog records and only selected parser fields;
request/response headers, publisher identities, credentials, and raw errors
are not retained. These fixtures catch parser regressions, not subsequent
upstream drift. Never refresh fixtures automatically to accept a failing test.

For a bounded live check, from a development checkout run:

```sh
python -m scripts.upstream_contracts --output /tmp/upstream-report.json
```

The probe uses actual adapter request construction and parsing. It allows only
three fixed HTTPS hosts/paths and queries, one request per source, no retries or
redirects, no environment proxies or credentials, a 10-second deadline per
source, and a 1 MiB uncompressed response limit. It requests three results each.
Run no more frequently than needed; the included schedule is weekly.

The JSON report distinguishes `passed`, `contract_failure` (successful HTTP
with malformed/empty/unusable results, invalid latency, or a safety bound), and
`upstream_unavailable` (network failure, timeout, or non-success HTTP). Exit codes
are respectively 0, 1, and 2, with contract failure taking precedence. A failed
live job is a triage signal, not proof of a SlopSearX defect. Check the report
before changing adapters. Blocked and rate-limited responses cannot establish
ranking quality. Positive latency means the adapter recorded timing; it is not
a benchmark. Usable URLs mean syntactically usable links, not that each target
was fetched. No independent relevance judgment or ranking comparison is made.

The **Upstream contracts (opt-in)** GitHub workflow supports manual dispatch.
Its weekly scheduled job runs only when repository variable
`LIVE_UPSTREAM_CONTRACTS=true`; this variable is not enabled by this change.
Live checks are separate from required PR CI and upload the machine report even
on failure. No secrets are required. Standard tests remain deterministic.

To prepare new evidence for review, add `--capture-dir /tmp/upstream-captures`.
Only successful probes write allowlisted payloads. Inspect that directory before
copying fixtures into `tests/fixtures/upstream_contracts`; review source schema
changes and ensure no unnecessary personal fields were retained. Fixture
sanitization intentionally omits abstracts, popularity metadata, and other fields,
so ordinary unit tests remain necessary for those adapter branches.
