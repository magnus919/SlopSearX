# Exa Search API

Neural/keyword web search whose result records carry *highlights* — passages
selected from the crawled page. Requires an API key. Added as the evaluation
candidate recorded in issue #272.

- **File:** `engines/exa.py`
- **Type:** API
- **Auth:** `ENGINE_EXA_API_KEY` (required)
- **Categories:** general, news
- **Tier:** 2 (specialised) — see `CONTRIBUTING.md` tier governance
- **Rate limit:** Exa documents 10 QPS on `/search` by default. The
  `rate_limit: 5.0` entry in `_DEFAULT_ENGINES` is a conservative local
  default, and note that nothing in SlopSearX currently reads per-engine
  `rate_limit` — the shared limiter uses one global bucket.
- **Base URL:** `https://api.exa.ai` (`POST /search`)
- **Cost class:** freemium

## Usage

```
?q=<query>&engines=exa
?q=<query>&categories=news
```

## Request shape

The adapter posts a fixed, bounded request body:

| Field | Value | Why |
|---|---|---|
| `numResults` | `max_results`, clamped to 10 | Exa's SDK documents `num_results` as "Default: 10, Max for basic: 10"; its per-request price also includes up to 10 results |
| `type` | `auto` (pinned) | `deep*` tiers run agentic retrieval with model synthesis at much higher cost |
| `contents.highlights` | `{query, maxCharacters: 500}` | The only content requested |
| `category` | `news` when the news category is in scope | Only exact mappings are sent |
| `startPublishedDate` / `endPublishedDate` | RFC 3339 bounds | From `date_from`/`date_to`, or expanded from `time_range` |

`contents.text` and `contents.summary` are **never** requested.

## Response

Returns web results with titles, URLs, publication dates and highlights. The
highlights become the result snippet; nothing else in the record reaches the
normalized result.

## Boundaries

- **No page bodies.** `contents.text` is never requested and a `text` field is
  ignored if returned unasked. Fetching and extracting a page belongs to the
  downstream retriever (`docs/RETRIEVAL_HANDOFF.md` §1, §9).
- **No generated prose.** `contents.summary` is never requested and a
  `summary` field is ignored. Only retrieved passages become a snippet.
- **No enforcement claims.** Publication-date parameters are consumed and
  forwarded, so `supported_filters` declares `date_from`/`date_to`/
  `time_range`. `enforced_filters` is empty: enforcement is only declared once
  the layer has been audited against live responses.

## Error classification

These are SlopSearX's chosen mappings, not a restatement of vendor semantics.
`402` is classified `BLOCKED` rather than `RATE_LIMITED` because it is an
account condition an operator must act on, not a throttle that clears on its
own.

| Upstream | `EngineStatus` |
|---|---|
| 429 | `RATE_LIMITED` |
| 402 | `BLOCKED` — credit/spending-budget exhaustion |
| 401 | `ERROR` (authentication rejected) |
| 403 | `BLOCKED` |
| other 4xx | `ERROR` |
| 5xx | `UNAVAILABLE` |
| timeout | `TIMEOUT` |

## Test coverage and its limits

The adapter's tests replay response fixtures **synthesized from Exa's
published API contract** (its OpenAPI document and the `exa-py` SDK). They are
not captured live responses, and there is no live contract coverage for this
engine — `scripts/upstream_contracts.py` probes only unauthenticated public
endpoints, which a keyed commercial API cannot join without a credential in
CI. So the tests catch parser and boundary regressions in SlopSearX; they
cannot detect upstream drift. A future upstream rename of the top-level
`results` key would surface as a healthy engine returning nothing.

## Notes

- Exa issues recurring monthly credits on signup and bills sustained use
  beyond the allowance. Verify current credit amounts and endpoint charges
  before enabling. Exa's published `/search` price includes up to 10 results
  per request; `ENGINE_EXA_MAX_RESULTS` is clamped to 10 for that reason and
  because the vendor SDK documents 10 as the maximum for basic usage.
- Exa's terms of service restrict copying, storing and redistributing
  material obtained through the service. SlopSearX caches whole responses in
  Valkey and serves them to clients, so review those terms against your
  deployment before enabling this engine.
- Without `ENGINE_EXA_API_KEY` the engine is inert: it appears in the
  capability catalog with `auth_configured: false` and is hard-excluded from
  cost/coverage routing, so enabling it is an explicit operator action.
- Set `ENGINE_EXA_API_KEY` env var or configure in `config.yaml`.
