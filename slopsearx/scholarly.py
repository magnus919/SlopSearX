"""Bounded, offline publication identity grouping before source fusion.

Only source-reported identifiers, recognized identifier URLs and explicit version
relations establish identity. Titles, citation links and indexing dates never do.
"""

from __future__ import annotations

import copy
import re
from dataclasses import replace
from typing import Any
from urllib.parse import urlparse

from slopsearx.adapter import SearchResult
from slopsearx.merger import _normalise_url
from slopsearx.payload import payload_for_persistence, payload_serialized_size
from slopsearx.publication_metadata import normalize_doi, validate_paper_fields

POLICY_VERSION = "scholarly-work-v1"
MAX_MEMBERS = 64
MAX_GROUP_BYTES = 64_000
MAX_RECORD_BYTES = 128_000
VERSION_RELATIONS = frozenset(
    {"isversionof", "hasversion", "isnewversionof", "ispreviousversionof", "ispreprintof", "haspreprint"}
)
ARXIV = re.compile(r"(?P<base>\d{4}\.\d{4,5}|[a-z-]+(?:\.[A-Z]{2})?/\d{7})(?:v(?P<version>[1-9]\d*))?", re.IGNORECASE)


def _data(result: SearchResult) -> dict[str, Any]:
    payload = result.payload or {}
    value = payload.get("data")
    return (
        value
        if payload.get("domain") == "science" and payload.get("type") == "publication" and isinstance(value, dict)
        else {}
    )


def _url_ids(url: str) -> dict[str, str]:
    try:
        parsed = urlparse(url)
        host = (parsed.hostname or "").casefold()
    except ValueError:
        return {}
    path = parsed.path.strip("/")
    if host in {"doi.org", "dx.doi.org"}:
        doi = normalize_doi(path)
        return {"doi": doi} if doi else {}
    if host in {"nature.com", "www.nature.com"} and re.fullmatch(
        r"articles/s\d{5}-\d{3}-\d{4,5}-[a-z0-9]", path, re.IGNORECASE
    ):
        return {"doi": "10.1038/" + path.split("/")[1].casefold()}
    if host == "pubmed.ncbi.nlm.nih.gov" and path.isdigit():
        return {"pmid": path}
    if host in {"pmc.ncbi.nlm.nih.gov", "www.ncbi.nlm.nih.gov"}:
        match = re.fullmatch(r"(?:pmc/)?articles/(PMC\d+)", path, re.IGNORECASE)
        if match:
            return {"pmcid": match[1].upper()}
    if host in {"arxiv.org", "www.arxiv.org", "export.arxiv.org"}:
        identifier = re.sub(r"^(?:abs|pdf)/", "", path).removesuffix(".pdf")
        if ARXIV.fullmatch(identifier):
            return {"arxiv": identifier.casefold()}
    return {}


def publication_ids(result: SearchResult) -> dict[str, str]:
    ids = _url_ids(result.url)
    data = _data(result)
    doi = normalize_doi(data.get("doi"))
    if doi:
        if "doi" in ids and ids["doi"] != doi:
            return {}  # Conflicting source identity is ambiguous.
        ids["doi"] = doi
    for key in ("pmid", "pmcid"):
        raw = data.get(key)
        if isinstance(raw, str) and re.fullmatch(r"\d+" if key == "pmid" else r"PMC\d+", raw, re.IGNORECASE):
            if key in ids and ids[key] != raw.upper():
                return {}
            ids[key] = raw.upper()
    raw_arxiv = data.get("arxiv_id") or (data.get("publication_id") if result.engine == "arxiv" else None)
    if isinstance(raw_arxiv, str) and ARXIV.fullmatch(raw_arxiv):
        if "arxiv" in ids and ids["arxiv"] != raw_arxiv.casefold():
            return {}
        ids["arxiv"] = raw_arxiv.casefold()
    return ids


def paper_fields(result: SearchResult) -> dict[str, Any]:
    data = dict(_data(result))
    doi = publication_ids(result).get("doi")
    if doi:
        data["doi"] = doi
    else:
        data.pop("doi", None)
    return validate_paper_fields(data)


def _version_edges(result: SearchResult, ids: dict[str, str]) -> list[tuple[str, str]]:
    edges: list[tuple[str, str]] = []
    arxiv = ids.get("arxiv")
    if arxiv:
        match = ARXIV.fullmatch(arxiv)
        if match:
            edges.append(("arxiv-base:" + match["base"], "arxiv"))
    relations = _data(result).get("relations")
    if not isinstance(relations, list):
        return edges
    for relation in relations[:32]:
        if not isinstance(relation, dict):
            continue
        kind = relation.get("relation_type")
        target = normalize_doi(relation.get("doi"))
        own = ids.get("doi")
        if isinstance(kind, str) and kind.casefold() in VERSION_RELATIONS and target and own:
            edges.append(("doi:" + target, kind.casefold()))
    return edges


def group_publications(feeds: dict[str, list[SearchResult]], query: str = "") -> dict[str, list[SearchResult]]:
    """Use a bounded identity graph; keep all admitted members or decline a merge."""
    entries = [(engine, pos, result) for engine, rows in feeds.items() for pos, result in enumerate(rows, 1)]
    identifiers = [publication_ids(result) for _, _, result in entries]
    parent = list(range(len(entries)))
    members = [[i] for i in range(len(entries))]

    def root(i: int) -> int:
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    def join(left: int, right: int, *, version: bool = False) -> None:
        left, right = root(left), root(right)
        if left == right or len(members[left]) + len(members[right]) > MAX_MEMBERS:
            return
        if not version:
            for namespace in ("doi", "pmid", "pmcid", "arxiv"):
                left_ids = {identifiers[i][namespace] for i in members[left] if namespace in identifiers[i]}
                right_ids = {identifiers[i][namespace] for i in members[right] if namespace in identifiers[i]}
                if left_ids and right_ids and left_ids != right_ids:
                    return
        # Decline a merge whose complete source payload cannot be retained.
        if any(
            entries[i][2].payload is not None
            and payload_for_persistence(entries[i][2].payload, max_bytes=MAX_GROUP_BYTES) is None
            for i in members[left] + members[right]
        ):
            return
        combined = [member_record(i) for i in members[left] + members[right]]
        size = payload_serialized_size({"members": combined})
        if size is None or size > MAX_GROUP_BYTES:
            return
        parent[right] = left
        members[left].extend(members[right])

    def member_record(i: int) -> dict[str, Any]:
        engine, pos, result = entries[i]
        return {
            "engine": engine,
            "original_position": pos,
            "url": result.url,
            "title": result.title,
            "content": result.content,
            "published_date": result.published_date,
            "identifiers": identifiers[i],
            "payload": payload_for_persistence(result.payload, max_bytes=MAX_GROUP_BYTES),
        }

    seen: dict[str, int] = {}
    for i, (_, _, result) in enumerate(entries):
        keys = ["url:" + _normalise_url(result.url)] + [key + ":" + value for key, value in identifiers[i].items()]
        for key in keys:
            if key in seen:
                join(i, seen[key])
            else:
                seen[key] = i
    newer: set[int] = set()
    older: set[int] = set()
    for i, (_, _, result) in enumerate(entries):
        for token, kind in _version_edges(result, identifiers[i]):
            if token.startswith("arxiv-base:"):
                if token in seen:
                    join(i, seen[token], version=True)
                else:
                    seen[token] = i
            elif token in seen:
                target = seen[token]
                join(i, target, version=True)
                # Only explicit chronology selects a newer version. Preprint and
                # generic version relationships establish grouping, not recency.
                if kind == "isnewversionof":
                    newer.add(i)
                    older.add(target)
                elif kind == "ispreviousversionof":
                    newer.add(target)
                    older.add(i)
    output: dict[str, list[SearchResult]] = {engine: [] for engine in feeds}
    grouped: dict[int, list[int]] = {}
    for i in range(len(entries)):
        grouped.setdefault(root(i), []).append(i)
    representatives: dict[int, SearchResult] = {}
    for group_key, indexes in grouped.items():
        candidate = indexes[0]
        explicit_newer = [i for i in indexes if i in newer and i not in older]
        if len(explicit_newer) == 1:
            candidate = explicit_newer[0]
        arxiv_versions = [
            (int(m["version"]), i)
            for i in indexes
            if (m := ARXIV.fullmatch(identifiers[i].get("arxiv", ""))) and m["version"]
        ]
        bases = {ARXIV.fullmatch(identifiers[i]["arxiv"])["base"] for _, i in arxiv_versions}  # type: ignore[index]
        ordered_arxiv = bool(
            arxiv_versions and len(bases) == 1 and len(arxiv_versions) == len(indexes) and not explicit_newer
        )
        if ordered_arxiv:
            candidate = max(arxiv_versions, key=lambda item: (item[0], -item[1]))[1]
        requested = [
            i
            for i in indexes
            if identifiers[i].get("arxiv")
            and re.search(r"(?<![\w.])" + re.escape(identifiers[i]["arxiv"]) + r"(?![\w.])", query, re.IGNORECASE)
            and "v" in identifiers[i]["arxiv"]
        ]
        requested_revision = len({identifiers[i]["arxiv"] for i in requested}) == 1
        if requested_revision:
            candidate = requested[0]
        selected = entries[candidate][2]
        fields = paper_fields(selected)
        for i in indexes:
            # Supplement metadata only from the same manifestation, not another
            # DOI/version's author list or bibliographic date.
            if identifiers[i] and identifiers[i] == identifiers[candidate]:
                for name, value in paper_fields(entries[i][2]).items():
                    fields.setdefault(name, value)
        warnings = sorted(
            {
                warning
                for i in indexes
                for warning in _data(entries[i][2]).get("publication_types", [])
                if isinstance(warning, str)
                and warning
                in {
                    "Retracted Publication",
                    "Retraction of Publication",
                    "Published Erratum",
                    "Corrected and Republished Article",
                    "Withdrawn Publication",
                }
            }
        )
        if warnings:
            fields["comments"] = "; ".join(
                filter(
                    None, [fields.get("comments", ""), *("Source member notice: " + warning for warning in warnings)]
                )
            )
        group = None
        if any(identifiers[i] or _data(entries[i][2]) for i in indexes):
            group = {
                "schema_version": 1,
                "paper": fields,
                "representative_engine": entries[candidate][0],
                "members": [member_record(i) for i in indexes],
                "selection": "requested_revision"
                if requested_revision
                else "explicit_revision"
                if ordered_arxiv or len(explicit_newer) == 1
                else "stable_source_order",
                "warnings": warnings,
            }
        representatives[group_key] = replace(selected, engines=set(selected.engines), work_group=group)
    engine_work_seen: dict[str, set[int]] = {engine: set() for engine in feeds}
    for i, (engine, _, original) in enumerate(entries):
        group_key = root(i)
        result = representatives[group_key]
        if result.work_group:
            if group_key in engine_work_seen[engine]:
                continue
            engine_work_seen[engine].add(group_key)
        output[engine].append(
            replace(
                result, engine=engine, engines={engine}, tier=original.tier, work_group=copy.deepcopy(result.work_group)
            )
        )
    return output
