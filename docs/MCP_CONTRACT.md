# SlopSearX MCP Contract — Field-Level Mapping

Status: **implemented / living reference**. This document is the authoritative,
field-level mapping between the SlopSearX internal search model and its MCP
representation. It is the acceptance evidence for the field-level MCP contract
mapping milestone and the living reference that contract tests (`tests/`) assert
against.

Every underlying field of `SearchResult`, `SearchResponse`, the scope/outcome
structures, the engine capability catalog, and the research coverage model is
mapped to exactly one of:

- **its MCP representation** (the JSON key(s) it becomes on the wire), or
- **an explicit omission rationale** (why it is deliberately not surfaced), or
- **an explicit unsupported state** (a field the MCP layer carries but states
  the underlying service does not enforce or produce).

Where a name differs between the internal model and the wire, this document
records both. The schema pins in `validation-contract.md` are authoritative for
names and vocabularies; this mapping is consistent with them.

The source of truth for every mapping below is the implemented code, primarily:

- `slopsearx/adapter.py` — `SearchResult`, `EngineAdapter`, capability vocabularies
- `slopsearx/service.py` — `SearchRequest`, `ScopeDecision`, `EngineOutcome`,
  `SearchResponse`, serialization boundary
- `slopsearx/capabilities.py` — `EngineCapability`, `IntentProfile`, `MCPPolicy`
- `slopsearx/snapshot.py` — `SearchSnapshot` (pagination/expansion)
- `slopsearx/research.py` — research coverage model
- `slopsearx/mcp/tools.py` — the MCP translation boundary (`_result_to_dict`,
  `_result_record`, `_envelope`, `_enforce_policy`, capabilities/status tools)

---

## 1. Result card vs. result record (progressive disclosure)

The MCP layer exposes one underlying result at two disclosure levels (design §4.4):

- **Card** (`slopsearx_search`, `slopsearx_search_targeted`,
  `slopsearx_search_jobs`, `slopsearx_search_security`,
  `slopsearx_search_science`, `slopsearx_read_results`): compact triage fields.
  Media and full content are **never** on a card.
- **Record** (`slopsearx_read_result`): the full evidence for one result —
  complete `content`, media fields, provenance, and the non-verification note.

`SearchResult` is the single shared source for both. The sections below list
each field once and note where it appears (card, record, or neither).

---

## 2. `SearchResult` field mapping

Source: `slopsearx/adapter.py:78-91`. The internal normalized result dataclass.

| Internal field | Type | MCP card | MCP record | Notes / rationale |
|---|---|---|---|---|
| `url` | `str` | `url` | `url` | Direct passthrough. Also echoed as `citation.url`. |
| `title` | `str` | `title` | `title` | Direct passthrough. Also echoed as `citation.label`. |
| `content` | `str` | `snippet` (truncated to 300 chars) | `content` (full) | Card truncation is the progressive-disclosure boundary (`SNIPPET_LENGTH = 300`, `tools.py`). The full string is **only** on the record. |
| `engine` | `str` | `primary_engine` | `primary_engine` | The adapter that produced the result is surfaced as the "primary" engine. |
| `engines` | `set[str]` | `source_engines` (sorted list) + `source_count` | `source_engines` (sorted list) + `source_count` | Canonicalized to a **sorted, duplicate-free list** at the serialization boundary (`search_result_to_dict`, `service.py`); rehydration is exact. When the set is empty, the list falls back to `[primary_engine]` so the cross-engine presence signal is never empty (`_source_engines`, `tools.py`). `source_count == len(source_engines)`. |
| `score` | `float` | `score` | `score` | Numeric ranking weight. |
| `position` | `int` | `position` | `position` | Rank position (0-based, matches snapshot-absolute indexing). |
| `category` | `str` | `category` | `category` | SearXNG-compatible category tag. |
| `published_date` | `Optional[str]` | `published_at` | `published_at` | ISO-8601 string or `None`; round-trips as exact value (`VAL-CORRECT-005`). |
| `thumbnail` | `Optional[str]` | *(omitted)* | `thumbnail` | **Media is record-only** (design §4.4 / decision 4; `VAL-EXPAND-006`). Omitted from cards by deliberate choice. |
| `img_src` | `Optional[str]` | *(omitted)* | `img_src` | **Media is record-only**. Omitted from cards by deliberate choice. |
| `tier` | `int` (1 or 2) | `tier` | `tier` | PresenceRanker tier; 1 = broad, 2 = specialized. `meta.ranking` states `tier_then_cross_engine_presence`. |
| `payload` | `Optional[dict]` | `payload` (conditionally) | `payload` (full) | Optional versioned domain payload (see §14). Cards inline it only when `include=["payload"]` was requested or the serialized payload is small enough (`PAYLOAD_INLINE_BYTES = 512`); otherwise it is omitted from cards. Records always carry the complete payload (or `null` when absent). |

Additional record-only fields synthesized at the MCP boundary (not on the
internal model, but derived from it):

| MCP record field | Derivation |
|---|---|
| `result_id` | Server-issued `"<cursor>:<index>"` where `index` is the 0-based absolute position in the full captured snapshot (`VAL-EXPAND-001`, schema pin). |
| `content_available` | `bool`; `true` iff `len(content) > SNIPPET_LENGTH` (i.e. the record reveals strictly more than the card). |
| `content_unavailable_note` | Present only when `content_available == false`: `"full content unavailable (adapter returned snippet only)"` — an explicit, honest marker rather than silently presenting the snippet as complete. |
| `citation` | `{label: title, url: url}` — stable across card/record/research-snapshot hops (`VAL-CROSS-013`). |
| `provenance` | `{query, query_id, rank_explanation, source_engines}` — how the result entered the set. |
| `snapshot` | `{cursor, query, query_id, total}` — citation/snapshot context. |
| `note` | `"SlopSearX did not fetch or verify the linked page"` — mandatory non-verification disclosure (`VAL-EXPAND-010`). |

---

## 3. `SearchResponse` field mapping (the search envelope)

Source: `slopsearx/service.py:128-147`. Built by `_envelope` in `tools.py`.

| Internal field | Type | MCP representation | Notes / rationale |
|---|---|---|---|
| `query` | `str` | `query` (envelope) | Echoed verbatim. |
| `results` | `list[SearchResult]` | `results` (card list) | Each result becomes a card. `max_results` is a presentation bound applied to the **returned page only**; the full ranked set is captured in the snapshot (`meta.total`). |
| `scope` | `ScopeDecision` | `scope` (object) | See §5 below. |
| `engine_outcomes` | `list[EngineOutcome]` | `engine_outcomes` (list) | See §6 below. List-typed per schema pin. |
| `suggestions` | `list[str]` | `meta.suggestions` | Gated by the `include` request: surfaced only when `include` contains `suggestions`, else an empty list — never silently dropped when requested (`VAL-SEARCH-006`). |
| `answers` | `list[dict]` | `answers` (envelope) | Typed surface — never discarded when the service produced them; empty list when absent (`VAL-SEARCH-007`). |
| `corrections` | `list[str]` | `corrections` (envelope) | Same envelope-recovery rule as `answers`. |
| `infoboxes` | `list[dict]` | `infoboxes` (envelope) | Same envelope-recovery rule as `answers`. |
| `query_id` | `str` | `meta.query_id` (also on `all_engines_failed` errors) | Non-empty on success. |
| `cached` | `bool` | `meta.cached` | `true` exactly when served from cache (`VAL-SEARCH-010`). |
| `response_time_ms` | `int` | `meta.response_time_ms` | |
| `partial` | `bool` | `meta.partial` | `true` when any selected engine did not return an ok status — a partial result is never presented as complete (`VAL-SEARCH-009`). |
| `all_unresponsive` | `bool` | → `error` envelope `all_engines_failed` | When `true`, the MCP layer returns a structured error (not an empty success) that still exposes `scope` and per-engine `engine_outcomes` so "no coverage" is distinguishable from "query produced nothing" (`VAL-SEARCH-015`). |
| `empty_engines` | `list[list[str]]` | `empty_engines` (list of `{engine, reason}`) | Zero-result engines are reported separately from `engine_outcomes`; an engine is never reported as both failed and empty. |
| `cached_error` | `bool` | `meta.cached_error` | Honest cache error flag. |

### 3.1 The `meta` block

`meta` carries the cross-cutting envelope metadata (source: `_envelope`):

| MCP key | Meaning |
|---|---|
| `meta.query_id`, `meta.cached`, `meta.cached_error`, `meta.response_time_ms`, `meta.partial` | Direct mirrors of the `SearchResponse` fields above. |
| `meta.ranking` | Fixed string `tier_then_cross_engine_presence` (`RANKING_EXPLANATION`), documenting how results were ordered. |
| `meta.cursor` | Snapshot handle; non-empty on every successful search when the snapshot store is available; `null`/absent only when the store is unavailable (with a pagination warning). |
| `meta.suggestions` | Gated by `include` as described above. |
| `meta.total` | Aggregate count of the full captured (unsliced) result set. |
| `meta.has_more` | Whether further pages exist beyond this response (`total > len(presented results)`). |

---

## 4. `SearchRequest` (request) parameter mapping

Source: `slopsearx/service.py:77-94`. These are the inputs that select what the
envelope surfaces; they are not part of the response model but define the
contract between tool inputs and the `SearchResponse`.

| SearchRequest field | MCP tool input(s) | Notes |
|---|---|---|
| `query` | `query` on every search tool (and `question` on `start_research`) | Validated (empty / over-long) before dispatch. |
| `categories` | `categories` | OR semantics; overridden by `engines`. |
| `engines` | `engines` / `sources` / `engines` (specialist) | Explicit engine list wins over categories and intent. Passed through the shared policy gate. |
| `language` | `language` (default `en`) | Never consumed by any adapter → `enforcement.language.status == "unsupported"` (`VAL-FILTER-003`). |
| `time_range` | `time_range` | Never consumed by any adapter → `enforcement.time_range.status == "unsupported"` (`VAL-FILTER-002`). |
| `safesearch` | `safesearch` (`off`/`moderate`/`strict`) | `moderate` → `unsupported`; `strict` → **rejected** (fails closed) because no adapter enforces it (`VAL-FILTER-004/005`). |
| `page` | `page` on `read_results` | 1-based pagination over the snapshot. |
| `max_results` | `max_results` | Bounded by policy; a presentation bound only — never truncates the captured snapshot. |
| `include` | `include` | Selects surfaced fields (e.g. `suggestions`, `engine_status`); view derivation is per-request and independent of cache population order. |
| `freshness` | `freshness` | `prefer_cache` / `prefer_fresh` / `no_preference`; not part of cache identity (cache stores the canonical full response). |
| `client_identifier` | *(derived)* | From `state.tenant`; not a user-facing input. |

---

## 5. `ScopeDecision` / `EngineExclusion` mapping

Source: `slopsearx/service.py:97-114`. The executed scope, surfaced under
envelope `scope` and by the preview tool.

| Internal field | Type | MCP representation | Notes |
|---|---|---|---|
| `selected_engines` | `list[str]` | `scope.selected_engines` | The engines actually dispatched. Sensitive engines are absent without the grant. |
| `resolved_categories` | `list[str]` | `scope.resolved_categories` | Categories that drove routing. |
| `routing_rule` | `str` | `scope.routing_reason` | Human-readable selection reason (schema pin). Preview tool also emits `routing_rule` as a backward-compatible alias. |
| `matched_topic` | `Optional[str]` | `scope` (execution) / `matched_topic` (preview only) | Topic matched by `auto` intent. |
| `warnings` | `list[str]` | merged into envelope `warnings` | Scope-level warnings. |
| `excluded_engines` | `list[EngineExclusion]` | `scope.excluded_engines` (list of `{engine, reason}`) | Engines considered but excluded (e.g. sensitive without grant, inactive). Preview exposes the same shape. |

`EngineExclusion` (`service.py:97-102`): `engine` → `excluded_engines[].engine`,
`reason` → `excluded_engines[].reason`.

---

## 6. `EngineOutcome` mapping

Source: `slopsearx/service.py:117-125`. One entry per attempted engine, exposed
as envelope `engine_outcomes` (a **list**, per schema pin) and reused in
research coverage.

| Internal field | MCP representation (`engine_outcomes[]`) | Research coverage |
|---|---|---|
| `engine` | `engine` | `engine` |
| `status` | `status` | `status` (and feeds bucket classification) |
| `result_count` | `result_count` | `result_count` |
| `latency_ms` | `latency_ms` | — |
| `message` | `message` | — |
| *(derived)* | — | `failure_class` (research only; see §8) |

`status` is exactly one of `EngineStatus` tokens: `ok`, `rate_limited`,
`blocked`, `error`, `timeout`, or `unavailable`. Engines that returned zero
results are reported via `empty_engines`, not as a non-ok outcome status
(`VAL-SEARCH-009`).

---

## 7. Engine capability catalog mapping

Sources: `slopsearx/adapter.py` (declarative adapter metadata + vocabularies),
`slopsearx/capabilities.py:75-103` (`EngineCapability`), surfaced by
`slopsearx_list_capabilities` and the `slopsearx://capabilities*` resources.

The catalog is **derived from the live registry**, never from prose. Every
capability field from the internal model maps to a same-name MCP entry.

### 7.1 Engine identity & state

| Internal field | MCP key | Notes |
|---|---|---|
| `name` | `name` | Registry engine name. |
| `display_name` | `display_name` | |
| `engine_type` | `type` | `api` / `scrape` / `structured`. |
| `categories` | `categories` | Full category list. |
| `subcategories` (property) | `subcategories` | Namespace-prefixed categories (e.g. `github:code`). |
| `enabled` | `enabled` | `include_disabled` controls whether disabled engines appear. |
| `auth_class` | `auth.class` | `none` / `optional` / `required` / `unknown` — a class, never a secret (`VAL-CAP-003`). |
| `auth_configured` | `auth.configured` | Boolean. Omitted entirely when `include_auth_requirements=false`. |
| `scope_hints` | `scope_hints` | Routing hints. |
| `caveats` | `caveats` | Operator-facing caveats. |

### 7.2 Feature matrix (design §4.6)

These derive from `EngineAdapter` declarative class attributes
(`adapter.py:113-129`), normalized with registry-derived defaults so every entry
is complete.

| Internal field | MCP key | Vocabulary / semantics |
|---|---|---|
| `sensitive` | `sensitive` | Boolean; `true` means reaching the engine requires `MCP_TARGETED_SENSITIVE_ALLOWED`. Fail-closed by default. |
| `supported_filters` | `supported_filters` | Object keyed by `language`, `time_range`, `safesearch`, `pagination`, each a boolean. No adapter declares support today → all `false`, matching the `unsupported` enforcement status. |
| `supported_result_types` | `supported_result_types` | List drawn from `text` / `answers` / `corrections` / `infoboxes` / `media` / `structured` (`SUPPORTED_RESULT_TYPES`). |
| `failure_classes` | `failure_classes` | List drawn from the stable token set `ok`, `rate_limited`, `blocked`, `error`, `timeout`, `auth_required`, `unavailable`. |
| `cost_class` | `cost_class` | Coarse operator-configured hint; empty string is emitted as `null` (explicit unknown — no fabricated estimates). |
| `last_known_status` | `last_known_status` | `ok` / `rate_limited` / `blocked` / `error` / `timeout` / `unknown`. Observed passively through search outcomes; defaults to `unknown`. |
| `last_known_status_at` | `last_known_status_at` | ISO freshness marker, or `None` when status is unknown. |

### 7.3 Capability vocabularies (shared by catalog and enforcement)

Defined in `adapter.py` and used verbatim:

- `SUPPORTED_FILTER_KEYS = (language, time_range, safesearch, pagination)`
- `SUPPORTED_RESULT_TYPES = (text, answers, corrections, infoboxes, media, structured)`
- `FAILURE_CLASS_TOKENS = (ok, rate_limited, blocked, error, timeout, auth_required, unavailable)`

The filter-enforcement report (`enforcement`) is consistent with the catalog:
an adapter that does not declare a filter yields `unsupported`; a strict
`safesearch` yields `rejected` (`_core_filter_enforcement`, `tools.py`). This
is the explicit-unsupported state the feature requires: the MCP layer carries
the filter parameter but honestly reports that no adapter consumes it.

---

## 8. Research coverage mapping

Sources: `slopsearx/research.py` (`EngineCoverage`, `CoverageSummary`,
`ResearchQuery`, `ResearchJob`), surfaced by `_job_summary` in `tools.py`.

### 8.1 Per-engine coverage entry (`engine_coverage[]`)

| Internal field | MCP key | Notes |
|---|---|---|
| `engine` | `engine` | |
| `bucket` | `bucket` | Disjoint class: `successful` / `empty` / `failed` / `unavailable` / `not-selected` (`VAL-RESEARCH-005`). |
| `status` | `status` | `EngineStatus` token where applicable. |
| `result_count` | `result_count` | |
| `failure_class` | `failure_class` | Stable token (`ok`/`rate_limited`/`blocked`/`error`/`timeout`/`unavailable`/`auth_required`); `auth_required` derived from credential state, not `AdapterResponse.status` (`VAL-RESEARCH-006`). |

### 8.2 Coverage summary (`coverage`, job- and query-level)

`CoverageSummary` fields map 1:1 to same-name integer counts:
`attempted`, `successful`, `empty`, `failed`, `unavailable`, `not_selected`.
The invariant `attempted == successful + empty + failed + unavailable` holds
(`VAL-RESEARCH-005`).

### 8.3 Job handle (`slopsearx_start_research` / `get_job`)

| Internal field | MCP key |
|---|---|
| `job_id` | `job_id` |
| `state` | `state` |
| `question` | `question` |
| `strategy` | `strategy` |
| `progress` (`completed`, `total`) | `progress.{completed,total}` |
| `deadline` | `deadline` (ISO) |
| `idempotency_key` | `idempotency_key` |
| `queries[]` | `queries` — each: `index`, `query`, `intent`, `engines`, `state`, `result_count`, `query_id`, `cursor`, `error`, `attempts[]`, `engine_coverage[]`, `coverage` |
| `warnings` | `warnings` |

---

## 9. Structured filter-enforcement report (`enforcement`)

Source: `_core_filter_enforcement` / `_enforcement_entry` in `tools.py`. Schema
pin: top-level key `enforcement`, an object keyed by filter name, each value
`{requested, status, reason, enforced_by}` where `status ∈ {enforced,
partially_enforced, unsupported, rejected}`.

| Filter | Reported status today | Rationale |
|---|---|---|
| `language` | `unsupported` | No adapter consumes it. |
| `time_range` | `unsupported` | No adapter consumes it. |
| `safesearch` (moderate) | `unsupported` | No adapter enforces it. |
| `safesearch` (strict) | `rejected` | Fail-closed: no engine can guarantee strict filtering. |
| `location`, `employment_type` (jobs) | `unsupported` | Not consumed by current ATS adapters. |
| `date_from`, `date_to` (science) | `unsupported` | Not consumed; pointer to `time_range` in the reason. |

`enforced_by` lists the engines that enforce the filter (empty for
`unsupported`). This report is the machine-readable replacement for prose-only
filter warnings (`VAL-FILTER-001`).

---

## 10. Error code vocabulary

The MCP layer returns structured `{error: {code, message, ...}}` envelopes. The
closed vocabulary (schema pin) is: `invalid_input`, `invalid_scope`,
`invalid_result_id`, `invalid_cursor`, `expired_handle`, `tool_disabled`,
`all_engines_failed`, `invalid_job_id`, `no_retryable_work`,
`store_unavailable`, plus `job_budget_exceeded`, `deadline_exceeded`,
`rate_limited`, and `safesearch_unenforced`. These are the error-side of the
contract; every one is emitted from `_error` in `tools.py`.

---

## 11. Cache / snapshot serialization boundary (round-trip guarantees)

Because the envelope surfaces `SearchResult`/`SearchResponse` evidence, the
serialization boundary must round-trip exactly. Documented guarantees
(verified by `VAL-CORRECT-*`):

- `SearchResult.engines` is canonicalized to a **sorted list** at the boundary
  (`search_result_to_dict`); rehydration (`search_result_from_dict`) is robust
  to lists and legacy stringified-set values — never iterating string
  characters.
- `SearchResult.payload` is canonicalized through
  `slopsearx.payload.payload_to_dict` (sets/tuples → lists, JSON-safe
  primitives) at the boundary and rehydrated through
  `slopsearx.payload.payload_from_dict`; missing/malformed values yield
  `None`, so a broken payload never crashes the read path.
- The cache stores the **canonical full response**; the MCP read boundary
  derives the requested `include`/`max_results` view so a cached response never
  disagrees with the current request.
- Optional/nullable fields (`thumbnail`, `img_src`, `published_date`, `category`,
  `score`, `tier`, `position`, `payload`) and empty collections round-trip as
  exact values.
- Snapshot payloads preserve engine provenance across `SnapshotStore.create` →
  read → rehydrate.

---

## 12. Fields with explicit omission rationale (summary)

| Internal field | MCP status | Rationale |
|---|---|---|
| `SearchResult.content` (full) on cards | Omitted (truncated to `snippet`) | Progressive disclosure; full content on records only. |
| `SearchResult.thumbnail`, `img_src` on cards | Omitted | Media is record-only by design (decision 4). |
| `SearchResponse.scope.matched_topic` on execution | Present in preview only | Execution doesn't need the topic echo; preview uses it for plan-vs-execute comparability. |
| `EngineCapability.auth` when `include_auth_requirements=false` | Omitted | Explicit request to drop auth info. |
| `EngineCapability.cost_class` when empty | `null` | Explicit unknown — no fabricated cost/latency estimates. |
| Adapter secrets / API keys anywhere | Never surfaced | Redaction is a hard invariant (`VAL-CAP-003`, `VAL-DIAG-011`). |

---

## 13. Consistency with the validation contract's schema pins

Every name in this mapping matches the schema pins in `validation-contract.md`:

- `enforcement` object with `{requested, status, reason, enforced_by}` and the
  closed status enum — §9.
- `scope.selected_engines` + `scope.routing_reason`; preview `routing_reason` /
  `matched_topic` / `excluded_engines` / `warnings` — §5.
- List-typed `engine_outcomes` with `{engine, status, result_count, latency_ms,
  message}`; zero-result engines via `empty_engines` — §3, §6.
- 0-based absolute `result_id` `<cursor>:<index>` — §2.
- `MCP_TARGETED_SENSITIVE_ALLOWED` as the uniform sensitive grant — §7.2, §9.
- Error-code vocabulary — §10.
- Research failure classes / coverage buckets — §8.

If a field name in this document ever diverges from the implemented source or
the contract pins, the contract pins and source are authoritative and this
document must be corrected.

---

## 14. Structured domain payloads

Source: `slopsearx/payload.py` (envelope contract and initial typed schemas),
attached by adapters via `SearchResult.payload` and mapped at the MCP boundary
in `tools.py` (`_payload_inline`, `_result_to_dict`, `_result_record`).

### 14.1 Envelope

A payload is an optional, self-describing object:

| Key | Type | Meaning |
|---|---|---|
| `domain` | `str` | Stable family: `security`, `science`, `packages`, `jobs`, `media`, `financial`, `biomedical`. |
| `type` | `str` | Narrower type within the family (e.g. `vulnerability`, `publication`, `package`, `job`, `media_item`, `economic_series`, `drug_label`). |
| `schema_version` | `int` | Version of the envelope schema (currently `1`). |
| `data` | `dict` | Domain-typed fields actually reported by the adapter. |
| `provenance` | `dict` | `engine` plus `adapter_fields`, `normalized_fields`, `inferred_fields` lists distinguishing field origin. |

`data` never invents fields the adapter did not return: `build_payload` drops
`None` values, so an absent source field stays absent rather than becoming a
fabricated `null`/`false`/empty value.

### 14.2 Disclosure (progressive)

- **Card** (`slopsearx_search` and friends, `slopsearx_read_results`): `payload`
  is present only when the caller requested `include=["payload"]` or the
  serialized payload is ≤ `PAYLOAD_INLINE_BYTES` (512). Otherwise the key is
  omitted.
- **Record** (`slopsearx_read_result`): `payload` is always present with the
  complete payload, or `null` when the result has none.

### 14.3 Source-derived evidence

Payload `data` is exactly what the adapter reported. SlopSearX does not fetch
or verify the linked page, does not fill in missing fields, and draws no
domain-specific conclusions from payload fields — a reported CVSS score is the
source's score, not an independent assessment.
