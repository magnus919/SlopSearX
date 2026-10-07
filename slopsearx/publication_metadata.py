"""Allowlisted SearXNG Paper metadata, independent of ranking/runtime."""

from __future__ import annotations

import re
from typing import Any

from slopsearx.adapter import SearchResult

STRING_FIELDS = ("doi", "journal", "publisher", "editor", "pages", "number", "comments", "type", "pdf_url", "html_url")
LIST_FIELDS = ("authors", "issn", "isbn", "tags")
DOI = re.compile(r"10\.\d{4,9}/[^\s<>]+", re.IGNORECASE)


def normalize_doi(value: Any) -> str | None:
    if not isinstance(value, str) or len(value) > 512:
        return None
    text = re.sub(r"^(?:https?://(?:dx\.)?doi\.org/|doi:\s*)", "", value.strip(), flags=re.IGNORECASE)
    return text.casefold() if DOI.fullmatch(text) else None


def validate_paper_fields(data: dict[str, Any]) -> dict[str, Any]:
    """Project only official Paper fields, bounded and never synthesized by an LLM."""
    fields: dict[str, Any] = {}
    doi = normalize_doi(data.get("doi"))
    if doi:
        fields["doi"] = doi
    for key in STRING_FIELDS:
        value = data.get(key)
        if key != "doi" and isinstance(value, str) and value and len(value.encode("utf-8", errors="replace")) <= 2048:
            fields[key] = value
    for key in LIST_FIELDS:
        value = data.get(key)
        if (
            isinstance(value, list)
            and len(value) <= 64
            and all(isinstance(item, str) and len(item) <= 256 for item in value)
        ):
            fields[key] = list(value)
    volume = data.get("volume")
    if type(volume) in (str, int) and len(str(volume)) <= 256:
        fields["volume"] = volume
    return fields


def projected_paper(result: SearchResult) -> dict[str, Any]:
    """Revalidate the wire allowlist even for rehydrated cache records."""
    raw = (result.work_group or {}).get("paper")
    return validate_paper_fields(raw) if isinstance(raw, dict) else {}
