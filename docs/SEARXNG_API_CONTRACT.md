# SearXNG API contract checks

`tests/test_searxng_api_contract.py` checks the compatibility boundary without
making uncontrolled requests to search engines. The deterministic reference is
captured from SearXNG `2026.6.29+28d388576` on 2026-09-07 and is stored in
[`tests/fixtures/searxng_api_contract.json`](../tests/fixtures/searxng_api_contract.json).

The suite covers both search routes, GET and form-encoded POST, format
negotiation and disabled formats, `/config`, `/healthz`, validation errors, and
the SearXNG result fields including `parsed_url`. SlopSearX additions such as
YAML, `meta`, `tier`, and routing metadata are explicitly permitted.

## Updating the pinned contract

When upgrading the reference SearXNG version:

1. Audit the official [Search API](https://docs.searxng.org/dev/search_api.html)
   and [administration API](https://docs.searxng.org/admin/api.html).
2. Run the bounded live comparison against the pinned deployment, recording
   only status codes, media types, field names, and sanitized examples.
3. Update the fixture version, capture date, and expectations deliberately.
4. Run `pytest --no-cov -q tests/test_searxng_api_contract.py` and review the
   resulting diff before merging.

The fixture is a contract oracle, not a relevance benchmark. No assertion may
depend on third-party engine availability or live search ranking.
