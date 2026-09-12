# Tavily Search API

Search API built for retrieval-augmented agents. Result records carry an
extracted passage per page; the response can also carry a model-generated
answer, which this adapter never requests. Requires an API key.

- **File:** `engines/tavily.py`
- **Type:** API
- **Auth:** `ENGINE_TAVILY_API_KEY` (required)
- **Categories:** general, news
- **Tier:** 2 (specialised) — see `CONTRIBUTING.md` tier governance
- **Rate limit:** not confirmed against Tavily's documentation. The
  `rate_limit: 5.0` entry in `_DEFAULT_ENGINES` is a conservative local
  default, and note that nothing in SlopSearX currently reads per-engine
  `rate_limit` — the shared limiter uses one global bucket.
- **Base URL:** `https://api.tavily.com` (`POST /search`)
- **Cost class:** freemium

## Usage

```
?q=<query>&engines=tavily
?q=<query>&categories=news
```

## Request shape

The adapter posts a fixed, bounded request body:

| Field | Value | Why |
|---|---|---|
| `max_results` | `max_results`, clamped to 20 | Spend bound — Tavily bills per API credit |
| `search_depth` | `basic` (pinned) | `advanced` costs more credits per request; the newer `fast`/`ultra-fast` depths have no published credit cost |
| `include_answer` | `false` (pinned) | The answer is model-authored; SlopSearX returns leads |
| `include_raw_content` | `false` (pinned) | Page bodies belong to the downstream retriever |
| `include_images` | `false` (pinned) | No image search capability is declared |
| `topic` | `news` / `finance` when in scope | Only exact mappings are sent |
| `time_range` | `day`/`week`/`month`/`year` | Tavily's vocabulary matches SearXNG's exactly |
| `start_date` / `end_date` | `YYYY-MM-DD` | From `date_from`/`date_to` |

## Response

Returns web results with titles, URLs, extracted passages and — for the news
topic — publication dates. The extracted passage becomes the result snippet.

## Boundaries

- **No page bodies.** `include_raw_content` is never requested and a
  `raw_content` field is ignored if returned unasked. Fetching and extracting
  a page belongs to the downstream retriever (`docs/RETRIEVAL_HANDOFF.md`
  §1, §9).
- **No generated prose.** `include_answer` is never requested and the
  top-level `answer` field is ignored, so a synthesized answer can never be
  presented as a retrieved passage. If it is ever wanted, it belongs on the
  `AdapterResponse.answers` channel with a model-authored label, behind an
  operator-visible switch.
- **No enforcement claims.** Relative and absolute publication windows are
  consumed and forwarded, so `supported_filters` declares `date_from`/
  `date_to`/`time_range`. `enforced_filters` is empty: enforcement is only
  declared once the layer has been audited against live responses.

## Error classification

These are SlopSearX's chosen mappings, not a restatement of vendor semantics.
`432`/`433` are classified `BLOCKED` rather than `RATE_LIMITED` — the vendor
SDK groups them with `403`, and they need operator action rather than waiting.

| Upstream | `EngineStatus` |
|---|---|
| 429 | `RATE_LIMITED` |
| 432, 433 | `BLOCKED` — plan/usage-limit exhaustion |
| 401 | `ERROR` (authentication rejected) |
| 403 | `BLOCKED` |
| other 4xx | `ERROR` |
| 5xx | `UNAVAILABLE` |
| timeout | `TIMEOUT` |

## Test coverage and its limits

The adapter's tests replay response fixtures **synthesized from Tavily's
published API contract** (the `tavily-python` and `tavily-js` SDKs, including
the vendor's own example response body). They are not captured live
responses, and there is no live contract coverage for this engine —
`scripts/upstream_contracts.py` probes only unauthenticated public endpoints,
which a keyed commercial API cannot join without a credential in CI. So the
tests catch parser and boundary regressions in SlopSearX; they cannot detect
upstream drift.

The `published_date` wire field name is taken from `tavily-js`, which maps
`publishedDate: result.published_date`; it does not appear in the
`tavily-python` example body, so confirm it against a live news-topic response
before relying on date-aware behaviour for this engine.

## Notes

- Tavily issues a monthly free credit allowance and bills beyond it. Credit
  cost varies by search depth and by the extras this adapter pins off;
  verify current pricing before enabling.
- Without `ENGINE_TAVILY_API_KEY` the engine is inert: it appears in the
  capability catalog with `auth_configured: false` and is hard-excluded from
  cost/coverage routing, so enabling it is an explicit operator action.
- Set `ENGINE_TAVILY_API_KEY` env var or configure in `config.yaml`.
- Tavily's terms of service were **not** reviewed for this adapter. SlopSearX
  caches whole responses in Valkey and serves them to clients; review Tavily's
  terms on caching, storage and redistribution against your deployment before
  enabling this engine, as you would for any commercial source.
