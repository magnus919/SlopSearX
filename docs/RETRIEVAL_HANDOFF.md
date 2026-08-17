# SlopSearX Search-to-Retrieval Handoff Contract

Status: **implemented / living reference** (issue 189). This document is the
authoritative definition of the boundary between SlopSearX (search-only) and a
downstream page retriever such as GroktoCrawl. It defines the machine-readable
`retrieval` handoff record that links a search result to downstream capture, the
URL eligibility vocabulary, and the failure/warning semantics for URLs that must
not be handed off.

Contract: `slopsearx.retrieval_handoff`, version `1`.

---

## 1. SlopSearX is a standalone search service

SlopSearX is a **standalone meta search engine**. It:

- runs its own pipeline — scope resolution, engine fan-out, deduplication,
  ranking, caching, snapshots — over its own engine adapters;
- exposes a **SearXNG-compatible HTTP API shape** (`/search?q=...&format=json`,
  `/search?q=...&format=yaml`, `/search?q=...&categories=...`, `/health`,
  `/metrics`, `/config`) as one output among several (the MCP surface is a
  second, agent-native surface);
- **does not wrap, proxy, or delegate requests to SearXNG**. The compatibility
  is a response-shape contract for drop-in consumers, not a runtime dependency;
- **does not fetch, extract, hash, or verify linked pages**. Every search result
  is a lead: title, URL, snippet, metadata, and provenance. Nothing in
  SlopSearX follows a result URL, and nothing in SlopSearX treats a snippet as
  verified evidence.

That boundary is deliberate. When a downstream reader needs the actual page —
for extraction, hashing, verification, or any other capture — it performs the
retrieval itself, using the handoff record below to associate the capture with
the originating search result and snapshot.

## 2. The search-only boundary

| Capability | SlopSearX | Downstream retriever (e.g. GroktoCrawl) |
|---|---|---|
| Discover candidate pages | Yes | No |
| Return snippet / title / URL / metadata / provenance | Yes | No |
| Rank and deduplicate by cross-engine presence | Yes | No |
| Capture stable result snapshots (opaque cursors) | Yes | No |
| Fetch the linked page | **No** | Yes |
| Extract / parse the page body | **No** | Yes |
| Hash the page content | **No** | Yes |
| Resolve redirects / canonicalize the live URL | **No** | Yes |
| Verify the page or its claims | **No** | Yes (and even then, per its own policy) |

SlopSearX's `score` is a cross-engine presence signal
(`tier_then_cross_engine_presence`), never relevance confidence or a
verification verdict. A snippet or a structured payload field is exactly what
the adapter reported — not an independent assessment.

## 3. The handoff record

The `retrieval` block is the stable, self-contained handoff record. It is
exposed in two disclosure levels:

- **Card** (every search tool and `slopsearx_read_results`): a compact
  eligibility summary — enough for a card-only consumer to decide whether to
  retrieve, and why not when it cannot.
- **Record** (`slopsearx_read_result`): the full handoff record — result
  identity, the verbatim result URL, provenance, and disclosure.

The record is **machine-readable by construction**: a downstream retriever
associates a capture with the originating result and snapshot through
`retrieval.result_id`, `retrieval.provenance.snapshot_cursor`, and
`retrieval.provenance.query_id` — never by parsing prose.

### 3.1 Card form

Present on every compact result card:

| Field | Type | Meaning |
|---|---|---|
| `contract` | `str` | `"slopsearx.retrieval_handoff"` — stable contract name. |
| `version` | `int` | Contract version (`1`). |
| `eligible` | `bool` | Whether the URL may be handed to a downstream HTTP retriever. |
| `url_status` | `str` | Closed eligibility token (see §4). |
| `url_reason` | `str \| null` | Stable human-readable reason; `null` when `url_status == "ok"`. |
| `scheme` | `str \| null` | Lowercased URL scheme when one parses (e.g. `https`). |

### 3.2 Record form (the full handoff record)

Present on every expanded result record, in addition to the card fields:

| Field | Type | Meaning |
|---|---|---|
| `contract` | `str` | `"slopsearx.retrieval_handoff"`. |
| `version` | `int` | `1`. |
| `result_id` | `str` | Server-issued `"<cursor>:<index>"` — the result identity to record on a capture. |
| `url` | `str \| null` | The raw result URL handed off **verbatim** — never canonicalized or rewritten — and the value to fetch; **non-null only when `url_status == "ok"`** (§5). |
| `url_status` | `str` | Closed eligibility token (see §4). |
| `url_reason` | `str \| null` | Stable reason; `null` when `url_status == "ok"`. |
| `scheme` | `str \| null` | Lowercased scheme. |
| `eligible` | `bool` | `true` iff `url_status == "ok"`. |
| `snippet_only` | `bool` | `true` when the record's full content is at most the snippet bound (300 chars) — i.e. the adapter returned a snippet only, so there is no cached body to rely on. |
| `verified` | `bool` | Always `false`. SlopSearX never fetches or verifies the linked page. |
| `verification_note` | `str` | `"SlopSearX did not fetch or verify the linked page"`. |
| `provenance` | `object` | `{snapshot_cursor, query_id, query, source_engines}` — snapshot/query provenance for capture association. |

`provenance` fields:

| Field | Type | Meaning |
|---|---|---|
| `snapshot_cursor` | `str` | The opaque snapshot handle (`meta.cursor`) the result came from. |
| `query_id` | `str` | The query execution id (`meta.query_id`). |
| `query` | `str` | The query text that produced the snapshot. |
| `source_engines` | `list[str]` | Sorted, duplicate-free contributing engine names. |

### 3.3 Example

```json
{
  "contract": "slopsearx.retrieval_handoff",
  "version": 1,
  "result_id": "snap-abc123def456:2",
  "url": "https://nvd.nist.gov/vuln/detail/CVE-2024-1234",
  "url_status": "ok",
  "url_reason": null,
  "scheme": "https",
  "eligible": true,
  "snippet_only": true,
  "verified": false,
  "verification_note": "SlopSearX did not fetch or verify the linked page",
  "provenance": {
    "snapshot_cursor": "snap-abc123def456",
    "query_id": "ssx-3f9a1c2b",
    "query": "CVE-2024-1234 impact analysis",
    "source_engines": ["nvd"]
  }
}
```

## 4. URL eligibility vocabulary

`url_status` is a **closed vocabulary**. SlopSearX classifies the result URL
advisory (it performs no fetch), so a downstream retriever can decide
eligibility and fetch safety deterministically.

| `url_status` | Meaning | `eligible` |
|---|---|---|
| `ok` | Absolute `http`/`https` URL with a host; `url` carries the captured value. | `true` |
| `missing` | The result has no URL string. | `false` |
| `non_http` | Parses to a non-HTTP(S) scheme (e.g. `mailto:`, `tel:`). | `false` |
| `unsafe_scheme` | A scheme an HTTP retriever must not fetch (`file:`, `data:`, `javascript:`, `vbscript:`, `gopher:`, `ftp:`). | `false` |
| `ambiguous` | Canonicalization-ambiguous: no scheme, no host, or unparseable (e.g. a relative reference, a bare `https://`, a malformed bracketed host, a backslash, whitespace, or control character in the authority, an invalid/out-of-range port, a percent-encoded or non-ASCII host, a literal IP host that is not globally routable — including decimal/hex/octal/abbreviated numeric encodings — or userinfo credentials in the authority). | `false` |

Only `ok` is eligible. Every other token is a machine-readable failure/warning
reason; `url_reason` carries the corresponding prose.

## 5. Never hand off an unsafe target

For `missing`, `non_http`, `unsafe_scheme`, and `ambiguous` URLs the handoff
record sets `url` to `null` and `eligible` to `false`. An ineligible URL is
**never** exposed as a fetch target, so a downstream retriever cannot blindly
retrieve an unsafe or ambiguous value (this is the mechanism that keeps
SlopSearX from becoming an SSRF-capable proxy: there is no fetch anywhere in
SlopSearX, and the handoff refuses to mint a target for schemes such as
`file:`/`data:`/`javascript:` or for URLs that cannot be unambiguously
canonicalized).

The raw result URL remains available in the common envelope (`url` on the
card/record) for audit and provenance; only the handoff target is withheld.

## 6. Associating a capture with its search result

A downstream retriever (GroktoCrawl is a supported composition option, not a
hidden runtime integration — see §7) records the handoff fields verbatim on
each capture. Association is then a key lookup, not prose parsing:

```
capture.handoff_ref.result_id       == retrieval.result_id
capture.handoff_ref.snapshot_cursor == retrieval.provenance.snapshot_cursor
capture.handoff_ref.query_id        == retrieval.provenance.query_id
capture.handoff_ref.url             == retrieval.url
```

Because `result_id` is snapshot-absolute (`<cursor>:<index>`) and the snapshot
is immutable, the originating search can be re-read later with
`slopsearx_read_result(result_id)` or
`slopsearx_read_results(cursor, ...)` as long as the snapshot TTL has not
expired.

## 7. GroktoCrawl composition

GroktoCrawl can be composed with SlopSearX as the **downstream reader**: it
consumes search results from SlopSearX, retrieves eligible `retrieval.url`
targets itself, and links each capture back via `retrieval.result_id` /
`retrieval.provenance.snapshot_cursor` / `retrieval.provenance.query_id`.

**The `retrieval` handoff block is emitted only on the MCP surface.** The
SearXNG-compatible HTTP JSON/YAML shape (`/search?q=...&format=json|yaml`)
carries no `retrieval` fields — it stays SearXNG-shaped so drop-in HTTP
consumers keep working unchanged (see §1). A consumer that needs the handoff
record must read results over MCP (`slopsearx_search` /
`slopsearx_read_results` / `slopsearx_read_result`). A consumer that only
integrates over the HTTP shape gets no handoff record: it can associate a
capture with the envelope fields alone, or read the corresponding result over
MCP.

This document names GroktoCrawl as a **composition option**. It is not an
undocumented runtime integration: SlopSearX ships no code that calls
GroktoCrawl, and GroktoCrawl is not required for SlopSearX to function. Each
side honors this handoff contract; neither depends on the other's internals.

## 8. Failure and warning semantics

- **Missing URL** — `url_status: "missing"`, `eligible: false`. The result is
  still a valid lead (title/snippet/provenance) but there is nothing to fetch.
- **Non-HTTP URL** — `url_status: "non_http"`. The scheme is reported so a
  retriever can route it elsewhere or skip it; it is never handed off over HTTP.
- **Unsafe scheme** — `url_status: "unsafe_scheme"`. The handoff refuses to
  mint a fetch target; a retriever must skip it.
- **Canonicalization-ambiguous URL** — `url_status: "ambiguous"`. SlopSearX
  never guesses a target; the retriever must treat it as ineligible. This
  includes URLs whose authority cannot be canonicalized unambiguously:
  - a backslash, whitespace, or control character in the authority —
    Python's `urlparse` does not normalize backslashes, so
    `https://internal.example\@public.com/` would otherwise parse with host
    `public.com` while a WHATWG client connects to `internal.example`, and a
    literal ASCII space (`http:// example.com/`) would otherwise certify an
    unfetchable authority as `ok`;
  - a port that is not a parseable integer or is outside the sane TCP range
    (`http://host:abc/`, `http://host:99999/`);
  - a percent-encoded or non-ASCII host — Python's `urlparse` neither
    percent-decodes nor IDNA-maps the host, so `http://%31%32%37.0.0.1/`
    would otherwise be certified `ok` while a WHATWG client decodes it to
    `127.0.0.1` (metadata-IP SSRF);
  - a literal IP host that is not globally routable, in any numeric encoding
    — `http://127.0.0.1/`, `http://[::1]/`, `http://10.0.0.1/`,
    `http://169.254.169.254/`, and the decimal/hex/octal/abbreviated forms
    `http://2130706433/`, `http://0x7f000001/`, `http://0177.0.0.1/`, and
    `http://127.1/` (all loopback) or `http://2852039166/` /
    `http://0xa9fea9fe/` (the metadata IP). The check is literal-only: it
    performs no DNS, so hostnames still classify normally;
  - userinfo credentials in the authority (`http://user:pass@example.com/`),
    which would otherwise persist credentials and trigger downstream
    Basic-auth transmission.
  The search pipeline itself also survives such URLs: a result whose URL cannot be parsed
  is deduplicated by its raw value and classified `ambiguous` at the handoff
  boundary instead of failing the whole search.
- **`verified` is always `false`** — no handoff record ever implies the page
  was fetched, parsed, or verified by SlopSearX.

A retriever that receives an ineligible handoff should record the
`url_status`/`url_reason` on the capture for audit, exactly as it records the
result identity.

## 9. Out of scope (hard boundary)

- Page fetch, extraction, hashing, HTTP metadata capture, and SSRF controls
  inside SlopSearX.
- Delegating requests from SlopSearX to SearXNG.
- Treating search snippets as verified evidence.

Changes that cross this boundary belong to the retriever, not to SlopSearX.

## 10. Related documents and fixtures

- `docs/MCP_CONTRACT.md` — field-level mapping of `retrieval` on cards/records.
- `docs/MCP_SERVER.md` — MCP tool surface that emits the handoff record.
- `tests/fixtures/retrieval_handoff/search_handoff_capture.json` — search →
  handoff → downstream capture provenance example.
- `tests/fixtures/retrieval_handoff/ineligible_url_unsafe.json` — an unsafe URL
  classified by the contract.
- `tests/test_retrieval_handoff.py` — contract tests asserting the vocabulary,
  provenance survival across cards/expansion/snapshot reads, and fixture
  consistency.
