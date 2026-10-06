"""Deterministic boundaries for caller-directed research planning."""

from __future__ import annotations

import re
from typing import Any

PLANNING_METHODS = frozenset({"original", "decomposition", "terminology_expansion", "evidence_followup"})
MAX_EVIDENCE_REFERENCES = 10
_IDENTIFIER = re.compile(
    r"\bCVE-\d{4}-\d{4,}\b|\b10\.\d{4,9}/[^\s<>]+|\b\d{4}\.\d{4,5}(?:v\d+)?\b|@[\w.-]+/[\w.-]+",
    re.IGNORECASE,
)


def validate_planning_metadata(method: Any, evidence_ids: Any, parent: str | None, rationale: str | None) -> list[str]:
    """Reject malformed caller metadata without coercing typed result IDs."""
    if method is not None and (not isinstance(method, str) or method not in PLANNING_METHODS):
        raise ValueError("unknown planning_method")
    if not isinstance(evidence_ids, list) or len(evidence_ids) > MAX_EVIDENCE_REFERENCES:
        raise ValueError("evidence_result_ids must be a list of at most 10 result IDs")
    if any(not isinstance(value, str) or not value or len(value) > 160 for value in evidence_ids):
        raise ValueError("evidence_result_ids must contain nonempty bounded strings")
    if len(set(evidence_ids)) != len(evidence_ids):
        raise ValueError("duplicate evidence_result_ids")
    if evidence_ids and (not parent or not rationale):
        raise ValueError("evidence references require a parent_attempt_id and rationale")
    if method == "evidence_followup" and not evidence_ids:
        raise ValueError("evidence_followup requires admitted result references")
    return list(evidence_ids)


def validate_variant(question: str, variant: str) -> None:
    """Preserve recognizable exact identifiers, not inferred semantic constraints."""
    identifiers = [match.group().rstrip('.,;:!?)"]}') for match in _IDENTIFIER.finditer(question)]
    tokens = {match.group().rstrip('.,;:!?)"]}').casefold() for match in _IDENTIFIER.finditer(variant)}
    if any(identifier.casefold() not in tokens for identifier in identifiers):
        raise ValueError("terminology variants must preserve every CVE, DOI, arXiv or scoped-package identifier")


def query_identity(query: str, engines: list[str]) -> tuple[str, tuple[str, ...]]:
    """Identity for avoiding repeat text searches in the same explicit scope."""
    return " ".join(query.split()).casefold(), tuple(sorted(set(engines)))
