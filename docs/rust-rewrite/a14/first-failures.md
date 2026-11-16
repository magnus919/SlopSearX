# A14 first native failures

These are the first native regression failures from this batch, retained as
observed. They were not Python oracle outputs. The source was inspected after
the failures, and only the local test configuration or malformed-input model
was corrected.

## Configured base path omitted in loopback setup

Initial failure, captured from `cargo test -p ssx-infra jobs::tests --lib`:

```text
thread 'adapters::jobs::tests::greenhouse_builds_content_query_and_job_payload_with_absent_id_preserved' panicked:
assertion failed: request.starts_with("GET /v1/boards/acme-inc/jobs?content=true HTTP/1.1")

thread 'adapters::jobs::tests::lever_sends_mode_json_and_normalizes_categories_epoch_and_limit' panicked:
"GET /acme?mode=json HTTP/1.1\r\nuser-agent: SlopSearX/0.1.0 (meta search engine; agent-native)\r\naccept-encoding: gzip, deflate\r\naccept: */*\r\nhost: 127.0.0.1:52817\r\n\r\n"
```

The native fixtures configured `base_url` as the loopback origin only. Source
defaults include endpoint prefixes: Greenhouse uses
`https://boards-api.greenhouse.io/v1/boards` at `engines/greenhouse.py:57`,
then appends `/{company_slug}/jobs` and `content=true` at lines 61–69. Lever
uses `https://api.lever.co/v0/postings` at `engines/lever.py:56`, then appends
the company slug and `mode=json` at lines 60–68. I corrected fixture configs to
include `/v1/boards` and `/v0/postings`; adapter path construction stayed the
same. The rerun captured the expected request paths and passed.

## Incorrect expected count for a valid empty Ashby response

Initial failure:

```text
thread 'adapters::jobs::tests::malformed_payloads_and_http_failures_are_classified' panicked:
assertion `left == right` failed
  left: 0
 right: 1
```

The loop incorrectly paired a valid empty
`{"data":{"jobBoard":{"jobPostings":[]}}}` response with an expected count
of one. Ashby source slices the empty `jobPostings` collection at
`engines/ashby.py:97–99`, which correctly yields zero results. I corrected the
native fixture expectation to zero. A separate top-level array remains a
malformed response because source calls `.get` on the parsed root object at
lines 97–98; native parsing now classifies it as an error.

The first run ended with 4 passing and 3 failing tests. The corrected run passed
all 8 native cases; the output is recorded in `coverage.json`.
