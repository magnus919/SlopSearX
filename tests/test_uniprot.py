"""Deterministic relevance and request-shape tests for UniProt."""

from __future__ import annotations

import httpx

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines


def _protein(
    accession: str,
    gene: str,
    organism: str,
    name: str,
) -> dict:
    return {
        "primaryAccession": accession,
        "uniProtkbId": f"{accession}_TEST",
        "proteinDescription": {"recommendedName": {"fullName": {"value": name}}},
        "genes": [{"geneName": {"value": gene}}],
        "organism": {"scientificName": organism},
    }


def _fixture_results() -> list[dict]:
    # Deliberately put the related record first.  The human and mouse records
    # are both exact gene matches; their relative order must remain upstream
    # order because no species was requested.
    return [
        _protein("Q99999", "VIMBP", "Homo sapiens", "Vimentin-binding protein"),
        _protein("P20152", "Vim", "Mus musculus", "Vimentin"),
        _protein("P08670", "VIM", "Homo sapiens", "Vimentin"),
    ]


async def test_simple_gene_query_boosts_exact_genes_without_species_filter():
    adapter = discover_engines({"uniprot": {"enabled": True}})["uniprot"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": _fixture_results()})

    adapter.set_http_transport(httpx.MockTransport(handler))
    response = await adapter.search("VIM")

    assert response.status == EngineStatus.OK
    assert requests[0].url.params["query"] == "(VIM) OR (gene:VIM)"
    assert requests[0].url.params["size"] == "30"
    assert requests[0].url.params["format"] == "json"
    assert [result.url.rsplit("/", 2)[-2] for result in response.results] == ["P20152", "P08670", "Q99999"]
    assert [result.position for result in response.results] == [1, 2, 3]
    assert [result.score for result in response.results] == [2.0, 2.0, 1.0]
    assert "Mus musculus" in response.results[0].content
    assert "Homo sapiens" in response.results[1].content


async def test_accession_query_boosts_exact_identifier_and_keeps_related_records():
    adapter = discover_engines({"uniprot": {"enabled": True}})["uniprot"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": _fixture_results()})

    adapter.set_http_transport(httpx.MockTransport(handler))
    response = await adapter.search("P08670")

    assert response.status == EngineStatus.OK
    assert requests[0].url.params["query"] == "(P08670) OR (accession:P08670)"
    assert response.results[0].url.endswith("/P08670/entry")
    assert response.results[0].score == 3.0
    assert {result.url.rsplit("/", 2)[-2] for result in response.results} == {"P08670", "P20152", "Q99999"}


async def test_exact_gene_beyond_presentation_limit_is_promoted_from_bounded_window():
    adapter = discover_engines({"uniprot": {"enabled": True, "max_results": 2}})["uniprot"]
    requests: list[httpx.Request] = []
    records = [
        _protein("Q99999", "VIMBP", "Homo sapiens", "Vimentin-binding protein"),
        _protein("Q99998", "VIML", "Mus musculus", "Vimentin-like protein"),
        _protein("P08670", "VIM", "Homo sapiens", "Vimentin"),
    ]

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": records})

    adapter.set_http_transport(httpx.MockTransport(handler))
    response = await adapter.search("VIM")

    assert response.status == EngineStatus.OK
    assert requests[0].url.params["size"] == "6"
    assert [result.url.rsplit("/", 2)[-2] for result in response.results] == ["P08670", "Q99999"]
    assert len(response.results) == 2


async def test_explicit_gene_query_is_preserved_and_matches_gene_fields_case_insensitively():
    adapter = discover_engines({"uniprot": {"enabled": True}})["uniprot"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": _fixture_results()})

    adapter.set_http_transport(httpx.MockTransport(handler))
    response = await adapter.search("gene:VIM")

    assert response.status == EngineStatus.OK
    assert requests[0].url.params["query"] == "gene:VIM"
    assert [result.score for result in response.results] == [2.0, 2.0, 1.0]
