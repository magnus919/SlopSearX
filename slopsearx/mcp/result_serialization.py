"""Canonical MCP result cards and expanded records with progressive disclosure.

Pure projections of result/snapshot data; no policy decisions or engine dispatch.
"""

from __future__ import annotations

from typing import Any

from slopsearx.adapter import SearchResult, media_to_dict
from slopsearx.artifacts import artifact_ref
from slopsearx.payload import (
    PAYLOAD_INLINE_BYTES,
    is_valid_payload,
    payload_max_persist_bytes,
    payload_serialized_size,
    payload_to_dict,
)
from slopsearx.retrieval_url import RETRIEVAL_URL_STATUS_OK, _retrieval_url
from slopsearx.snapshot import SearchSnapshot

RANKING_EXPLANATION = "tier_then_cross_engine_presence"
SNIPPET_LENGTH = 300
NON_VERIFICATION_NOTE = "SlopSearX did not fetch or verify the linked page"
CONTENT_UNAVAILABLE_NOTE = "full content unavailable (adapter returned snippet only)"
RETRIEVAL_HANDOFF_CONTRACT = "slopsearx.retrieval_handoff"
RETRIEVAL_HANDOFF_VERSION = 1


def _source_engines(result: SearchResult) -> list[str]:
    """Sorted, duplicate-free list of contributing engine names.

    Falls back to the primary engine when ``engines`` is empty so the
    cross-engine presence signal is always a non-empty name list.
    """
    return sorted(result.engines) if result.engines else [result.engine]


def _payload_size(payload: dict[str, Any]) -> int | None:
    """Approximate serialized byte size of a payload envelope.

    Returns ``None`` when the payload cannot be JSON-serialized. Callers must
    treat ``None`` as "omit", never as the smallest possible payload — an
    unserializable (e.g. circular-reference) payload must not be inlined on a
    compact card.
    """
    return payload_serialized_size(payload)


def _payload_inline(result: SearchResult, *, requested: bool) -> dict[str, Any] | None:
    """Return the payload to inline on a compact card, or ``None``.

    Compact cards carry a payload only when the caller requested it
    (``include=["payload"]``) or the payload is small enough to inline — and,
    in both cases, only when it is within the persistence bound, so a card
    never shows a payload the record path would return as ``null``. The
    disclosure gates measure and emit the **same canonical form** the
    persistence boundary stores (``payload_to_dict``), so a fresh hit and a
    cache hit can never diverge (a set/bytes-bearing or depth-truncated
    payload is treated identically on both paths) and a card never discloses
    more than the expanded record (progressive disclosure). An
    unserializable payload is always omitted, even when requested, so the MCP
    response itself never fails to serialize.
    """
    if result.payload is None:
        return None
    canonical = payload_to_dict(result.payload)
    if canonical is None:
        return None
    size = _payload_size(canonical)
    if size is None:
        return None
    if requested:
        return canonical if size <= payload_max_persist_bytes() else None
    if size <= PAYLOAD_INLINE_BYTES:
        return canonical if size <= payload_max_persist_bytes() else None
    return None


def _media_triage(result: SearchResult) -> dict[str, Any] | None:
    """Compact, triage-safe media summary for a compact card (issue 188).

    Carries ``media_type`` plus thumbnail/dimensions/duration where present —
    enough to triage a media result without fetching it — but omits the
    media file ``url`` and ``source`` attribution, which belong to the
    expanded record (progressive disclosure). Returns ``None`` for
    non-media results so text cards stay field-identical to before.
    """
    if result.media is None:
        return None
    media = media_to_dict(result.media)
    if not media:
        return None
    return {key: media[key] for key in ("media_type", "thumbnail", "width", "height", "duration") if key in media}


def _retrieval_card(result: SearchResult) -> dict[str, Any]:
    """Compact retrieval-eligibility summary for a triage card.

    Cards carry the eligibility tokens so a card-only consumer can decide
    whether a result is retrievable (and why not) without expanding it. The
    full handoff record — result identity, the verbatim result URL, and
    provenance — lives on the expanded record (progressive disclosure).
    """
    status, reason, scheme, _url = _retrieval_url(result.url)
    return {
        "contract": RETRIEVAL_HANDOFF_CONTRACT,
        "version": RETRIEVAL_HANDOFF_VERSION,
        "eligible": status == RETRIEVAL_URL_STATUS_OK,
        "url_status": status,
        "url_reason": reason,
        "scheme": scheme,
    }


def _retrieval_handoff(result: SearchResult, snapshot: SearchSnapshot, result_id: str) -> dict[str, Any]:
    """Build the stable search→retrieval handoff record (contract v1).

    Self-contained and machine-readable so a downstream retriever
    (GroktoCrawl or another capture layer) can associate a captured page with
    the originating result and snapshot without parsing prose or re-querying
    SlopSearX. Exposes the explicit retrieval boundary: a result is a
    snippet-only lead (``verified`` is always ``False``), and ``url`` /
    ``eligible`` carry the fetch-safety classification. SlopSearX never
    fetches the linked page; this record is the composition contract, not a
    runtime integration.
    """
    status, reason, scheme, handoff_url = _retrieval_url(result.url)
    content = result.content or ""
    return {
        "contract": RETRIEVAL_HANDOFF_CONTRACT,
        "version": RETRIEVAL_HANDOFF_VERSION,
        "result_id": result_id,
        "url": handoff_url,
        "url_status": status,
        "url_reason": reason,
        "scheme": scheme,
        "eligible": status == RETRIEVAL_URL_STATUS_OK,
        "snippet_only": len(content) <= SNIPPET_LENGTH,
        "verified": False,
        "verification_note": NON_VERIFICATION_NOTE,
        "provenance": {
            "snapshot_cursor": snapshot.snapshot_id,
            "query_id": snapshot.query_id,
            "query": snapshot.query,
            "source_engines": _source_engines(result),
        },
    }


def _result_to_dict(
    result: SearchResult,
    *,
    result_id: str | None = None,
    include_payload: bool = False,
) -> dict[str, Any]:
    """Normalize one result into a compact triage card (design §3.1).

    Cards carry triage fields plus a stable server-issued ``result_id``, a
    compact ``retrieval`` eligibility block (contract in
    docs/RETRIEVAL_HANDOFF.md), and — for media results — a compact ``media``
    triage summary. Full ``content``, ``thumbnail``, and ``img_src`` belong
    to the expanded record (progressive disclosure), never the card. The
    complete media record (including the media file URL and source
    attribution) is available via ``slopsearx_read_result``. A domain payload
    is inlined only when requested or small enough (see ``_payload_inline``);
    the full payload is available via ``slopsearx_read_result`` when it is
    within ``PAYLOAD_MAX_PERSIST_BYTES``.
    """
    card = {
        "title": result.title,
        "url": result.url,
        "snippet": (result.content or "")[:SNIPPET_LENGTH],
        "source_engines": _source_engines(result),
        "source_count": len(_source_engines(result)),
        "primary_engine": result.engine,
        "category": result.category,
        "published_at": result.published_date,
        "score": result.score,
        "position": result.position,
        "tier": result.tier,
        "citation": {"label": result.title, "url": result.url},
        "retrieval": _retrieval_card(result),
    }
    media = _media_triage(result)
    if media:
        card["media"] = media
    if result_id is not None:
        card["result_id"] = result_id
        card["artifact"] = artifact_ref("result", result_id)
    inline = _payload_inline(result, requested=include_payload)
    if inline is not None:
        card["payload"] = inline
    return card


def _payload_for_record(payload: dict[str, Any] | None) -> dict[str, Any] | None:
    """Return the payload to reveal on a full record, or ``None``.

    The record reveals the complete payload only when it satisfies the
    self-describing envelope contract and is JSON-serializable. A malformed or
    unserializable payload is omitted rather than risking an invalid record.
    """
    if not is_valid_payload(payload):
        return None
    if payload_serialized_size(payload) is None:
        return None
    return payload


def _result_record(result: SearchResult, snapshot: SearchSnapshot, result_id: str) -> dict[str, Any]:
    """Build the full result record for ``slopsearx_read_result``.

    Reveals strictly more than the card: complete normalized ``content``,
    media fields, every contributing engine, provenance, a
    ``content_available`` flag, an explicit non-verification note, and the
    stable ``retrieval`` handoff record (docs/RETRIEVAL_HANDOFF.md) that a
    downstream retriever uses to link a capture back to this result.
    """
    content = result.content or ""
    source_engines = _source_engines(result)
    content_available = len(content) > SNIPPET_LENGTH
    record = {
        "result_id": result_id,
        "artifact": artifact_ref("result", result_id),
        "title": result.title,
        "url": result.url,
        "content": content,
        "content_available": content_available,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "media": media_to_dict(result.media),
        "payload": _payload_for_record(result.payload),
        "source_engines": source_engines,
        "source_count": len(source_engines),
        "primary_engine": result.engine,
        "category": result.category,
        "published_at": result.published_date,
        "tier": result.tier,
        "position": result.position,
        "score": result.score,
        "citation": {"label": result.title, "url": result.url},
        "provenance": {
            "query": snapshot.query,
            "query_id": snapshot.query_id,
            "rank_explanation": snapshot.ranking_explanation,
            "source_engines": source_engines,
        },
        "snapshot": {
            "cursor": snapshot.snapshot_id,
            "query": snapshot.query,
            "query_id": snapshot.query_id,
            "total": snapshot.total,
        },
        "note": NON_VERIFICATION_NOTE,
        "retrieval": _retrieval_handoff(result, snapshot, result_id),
    }
    if not content_available:
        record["content_unavailable_note"] = CONTENT_UNAVAILABLE_NOTE
    return record
