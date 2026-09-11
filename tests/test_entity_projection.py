"""Explicit-ID oracles, conservation, real adapters, cache and tenant boundaries."""

from __future__ import annotations

import copy
import json

import httpx
import pytest

from slopsearx.adapter import SearchResult, discover_engines
from slopsearx.mcp import lineage_tools
from slopsearx.mcp import tools as t
from slopsearx.mcp.entity_projection import entity_groups, entity_projection
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.payload import build_payload
from slopsearx.service import ScopeDecision
from slopsearx.snapshot import SearchSnapshot, SnapshotStore
from tests.test_adapters import MockHTTP
from tests.test_mcp_tools import _build_state


def result(name="CVE-2024-12345", *, engine="nvd", version=None, **facts):
    domain, kind, data = "security", "vulnerability", {"cve_id": name, **facts}
    if engine in {"npm", "pypi", "crates"}:
        domain, kind, data = "packages", "package", {"name": name, "version": version, **facts}
    return SearchResult(
        url="https://example.com/lead",
        title="same title",
        content="lead",
        engine=engine,
        engines={engine},
        payload=build_payload(domain, kind, data, engine=engine),
    )


def snapshot(results):
    return SearchSnapshot("snap-test", "q", "q1", results, ScopeDecision(), len(results), "default")


def test_grouping_conserves_original_indices_conflicts_and_does_not_mutate():
    rows = [
        result(description="first"),
        result("requests", engine="pypi", version="1"),
        SearchResult(url="https://unknown", title="same title", content="lead", engine="wikipedia"),
        result("cve-2024-12345", description="second"),
    ]
    before = copy.deepcopy(rows)
    groups = entity_groups(snapshot(rows))
    assert len(groups) == 3
    assert groups[0]["result_ids"] == ["snap-test:0", "snap-test:3"]
    assert groups[0]["conflicting_fields"] == ["description"]
    assert groups[0]["identifier"] == {"cve_id": "CVE-2024-12345"}
    assert groups[2]["entity_id"] is None
    assert rows == before
    assert entity_groups(snapshot(list(reversed(rows))))[0]["entity_id"] == groups[0]["entity_id"]


def test_release_coordinates_are_delimiter_safe_and_namespace_qualified():
    rows = [
        result("Some_Pkg", engine="pypi", version="1"),
        result("some.pkg", engine="pypi", version="1"),
        result("some-pkg", engine="npm", version="1"),
        result("some-pkg", engine="pypi", version="2"),
        result("some-pkg", engine="pypi"),
        result("@scope/pkg", engine="npm", version="1"),
        result("@Scope/pkg", engine="npm", version="1"),
    ]
    groups = entity_groups(snapshot(rows))
    assert len(groups) == 6
    assert groups[0]["result_ids"] == ["snap-test:0", "snap-test:1"]
    assert len({g["entity_id"] for g in groups}) == 6
    assert groups[3]["identifier"]["version"] is None
    assert all(g["entity_id"].startswith("entity-v1-") for g in groups)


@pytest.mark.parametrize("value", [None, 1, [], {}, "", "CVE-2024-1", "CVE-2024-12345 extra", "x" * 513])
def test_invalid_identifiers_remain_distinct(value):
    groups = entity_groups(snapshot([result(value), result(value)]))
    assert len(groups) == 2
    assert all(g["entity_id"] is None for g in groups)


@pytest.mark.parametrize(
    "mutation",
    [
        lambda p: p.update(schema_version=2),
        lambda p: p.update(schema_version=True),
        lambda p: p.update(domain="jobs"),
        lambda p: p.update(provenance=None),
        lambda p: p["provenance"].update(engine="unknown"),
        lambda p: p["provenance"].update(adapter_fields=[]),
        lambda p: p["provenance"].update(inferred_fields=["cve_id"]),
        lambda p: p["provenance"].update(inferred_fields=None),
        lambda p: p.update(data=[]),
    ],
)
def test_untrusted_or_unknown_payload_is_not_identity(mutation):
    row = result()
    mutation(row.payload)
    assert entity_groups(snapshot([row]))[0]["entity_id"] is None


def test_packages_ambiguity_and_provenance():
    rows = [
        result("pkg", engine="npm", ecosystem="pypi"),
        result("pkg", engine="crates"),
        result("pkg", engine="npm", version=["1"]),
        result("@a/b/c", engine="npm"),
    ]
    assert all(g["entity_id"] is None for g in entity_groups(snapshot(rows)))
    row = result()
    row.engine, row.engines = "cve", {"cve", "nvd"}
    assert entity_groups(snapshot([row]))[0]["namespace"] == "cve"


def test_equal_and_one_sided_facts_do_not_conflict():
    groups = entity_groups(snapshot([result(description="x"), result(description="x", references=["url"])]))
    assert groups[0]["conflicting_fields"] == []


def test_conflict_fields_are_specific_to_identity_namespace():
    first, second = result(), result()
    first.payload["data"]["name"] = "first"
    second.payload["data"]["name"] = "second"
    assert entity_groups(snapshot([first, second]))[0]["conflicting_fields"] == ["name"]
    packages = [
        result("pkg", engine="npm", cve_id="CVE-2024-12345"),
        result("pkg", engine="npm", cve_id="CVE-2024-99999"),
    ]
    assert entity_groups(snapshot(packages))[0]["conflicting_fields"] == ["cve_id"]


def test_large_snapshot_linear_conservation():
    rows = [result() for _ in range(500)] + [result(None) for _ in range(500)]
    groups = entity_groups(snapshot(rows))
    assert len(groups) == 501
    ids = [i for g in groups for i in g["result_ids"]]
    assert len(ids) == len(set(ids)) == 1000


def v2_result(domain, kind, data, *, engine="source"):
    return SearchResult(
        url="https://example.com/lead",
        title="lead",
        content="lead",
        engine=engine,
        engines={engine},
        payload=build_payload(domain, kind, data, engine=engine),
    )


def test_v2_scholarly_identifiers_normalize_without_merging_namespaces():
    row = v2_result(
        "science",
        "publication",
        {
            "doi": "https://doi.org/10.1000/CAFÉ",
            "pmid": "12345678",
            "pmcid": "pmc87654321",
            "openalex_id": "https://openalex.org/w2626778328",
        },
    )
    groups, relationships = entity_projection(snapshot([row]), version=2)

    assert [(group["namespace"], group["identifier"]) for group in groups] == [
        ("doi", {"doi": "10.1000/café"}),
        ("pmid", {"pmid": "12345678"}),
        ("pmcid", {"pmcid": "PMC87654321"}),
        ("openalex", {"openalex_id": "W2626778328"}),
    ]
    assert len({group["entity_id"] for group in groups}) == 4
    assert {edge["relation"] for edge in relationships} == {"source_reported_alias"}
    assert all(edge["result_ids"] == ["snap-test:0"] for edge in relationships)


def test_v2_repository_case_and_git_url_are_canonical_but_redirect_paths_are_not_followed():
    equivalent = [
        v2_result("code", "repository", {"repository": "Owner/Repo"}, engine="github"),
        v2_result("code", "repository", {"repository": "https://github.com/owner/repo.git"}, engine="github"),
    ]
    groups, _ = entity_projection(snapshot(equivalent), version=2)
    assert len(groups) == 1
    assert groups[0]["identifier"] == {"repository": "owner/repo"}
    assert groups[0]["result_ids"] == ["snap-test:0", "snap-test:1"]

    redirected = v2_result(
        "code", "repository", {"repository": "https://github.com/old/repo/redirect"}, engine="github"
    )
    assert entity_projection(snapshot([redirected]), version=2)[0][0]["entity_id"] is None

    conflicting = v2_result(
        "code",
        "repository",
        {"repository": "owner/one", "repository_url": "https://github.com/owner/two"},
        engine="github",
    )
    unresolved = entity_projection(snapshot([conflicting]), version=2)[0][0]
    assert unresolved["entity_id"] is None
    assert unresolved["reason"] == "conflicting_source_identifiers"


def test_v2_candidate_repository_and_advisory_edges_do_not_merge_entities():
    package = v2_result(
        "packages",
        "package",
        {"name": "pkg", "version": "1", "repository_url": "https://github.com/Org/Repo"},
        engine="pypi",
    )
    advisory = v2_result(
        "security",
        "vulnerability",
        {"cve_id": "CVE-2026-12345", "ghsa_id": "GHSA-2345-cfgh-jmpq"},
        engine="nvd",
    )
    groups, relationships = entity_projection(snapshot([package, advisory]), version=2)

    assert {group["namespace"] for group in groups} == {"pypi", "github_repository", "cve", "ghsa"}
    assert {edge["relation"] for edge in relationships} == {"candidate_repository", "source_reported_advisory"}
    assert all(edge["from_entity_id"] != edge["to_entity_id"] for edge in relationships)


@pytest.mark.parametrize(
    "data",
    [
        {"doi": "same title"},
        {"pmid": "0"},
        {"pmcid": "PMC12 extra"},
        {"openalex_id": "https://evil.example/W2626778328"},
        {"repository": "https://github.com/owner/repo/issues"},
        {"ghsa_id": "GHSA-aaaa-bbbb-cccc"},
        {"doi": "10.1000/" + "x" * 513},
    ],
)
def test_v2_malformed_or_oversized_identifiers_stay_unresolved(data):
    domain, kind = ("science", "publication")
    if "repository" in data:
        domain, kind = "code", "repository"
    if "ghsa_id" in data:
        domain, kind = "security", "advisory"
    group = entity_projection(snapshot([v2_result(domain, kind, data)]), version=2)[0][0]
    assert group["entity_id"] is None
    assert group["reason"] == "invalid_inferred_or_conflicting_identifier"


def test_v2_inferred_identifier_is_unresolved_and_ids_ignore_cursor_rank_and_order():
    inferred = v2_result("science", "publication", {"doi": "10.1000/example"})
    inferred.payload["provenance"]["inferred_fields"] = ["doi"]
    assert entity_projection(snapshot([inferred]), version=2)[0][0]["entity_id"] is None

    first = v2_result("science", "publication", {"doi": "doi:10.1000/Example"})
    second = copy.deepcopy(first)
    second.position, second.score = 99, 0.01
    left = snapshot([first])
    right = SearchSnapshot("another", "q", "other", [second], ScopeDecision(), 1, "default")
    assert (
        entity_projection(left, version=2)[0][0]["entity_id"] == entity_projection(right, version=2)[0][0]["entity_id"]
    )


async def test_entity_tool_negotiates_v2_and_rejects_unknown_version(state):
    cursor = await state.snapshots.create(
        "q",
        "q1",
        [v2_result("science", "publication", {"doi": "10.1000/example", "pmid": "123"})],
        ScopeDecision(),
    )
    v1 = await t.slopsearx_read_entities(cursor)
    v2 = await t.slopsearx_read_entities(cursor, version=2)
    assert v1["version"] == 1 and "relationships" not in v1
    assert v2["version"] == 2 and v2["relationships"][0]["relation"] == "source_reported_alias"
    graph = await lineage_tools.slopsearx_get_artifact_lineage(v2["entities"][0]["artifact"], max_depth=0)
    assert graph["nodes"][0]["status"] == "live"
    assert (await t.slopsearx_read_entities(cursor, version=3))["error"]["field"] == "version"
    assert (await t.slopsearx_read_entities(cursor, version=True))["error"]["field"] == "version"


@pytest.fixture
def state():
    value = _build_state()
    set_state(value)
    yield value
    set_state(None)


async def test_snapshot_pages_and_member_reads_preserve_flat_view(state):
    rows = [result(), result("pkg", engine="npm"), result("cve-2024-12345"), result(None)]
    cursor = await state.snapshots.create("q", "q1", rows, ScopeDecision())
    before = await t.slopsearx_read_results(cursor)
    page = await t.slopsearx_read_entities(cursor, max_results=1)
    assert page["meta"]["total_entities"] == 3
    assert page["meta"]["total_results"] == 4
    assert page["meta"]["unresolved_entities"] == 1
    assert page["meta"]["has_more"]
    assert page["entities"][0]["result_ids"] == [f"{cursor}:0", f"{cursor}:2"]
    for result_id in page["entities"][0]["result_ids"]:
        expanded = await t.slopsearx_read_result(result_id)
        assert expanded["result_id"] == result_id
    second = await t.slopsearx_read_entities(cursor, page=2, max_results=1)
    assert second["entities"][0]["namespace"] == "npm"
    assert (await t.slopsearx_read_entities(cursor, page=99))["entities"] == []
    assert (await t.slopsearx_read_results(cursor)) == before
    empty = await state.snapshots.create("q", "q2", [], ScopeDecision())
    assert (await t.slopsearx_read_entities(empty))["meta"]["total_entities"] == 0


async def test_tenant_lifecycle_errors(state, monkeypatch):
    cursor = await state.snapshots.create("q", "q1", [result()], ScopeDecision())
    with tenant_scope("other"):
        assert (await t.slopsearx_read_entities(cursor))["error"]["code"] == "invalid_cursor"
    for value, page, code in [("", 1, "invalid_input"), (cursor, 0, "invalid_input"), ("missing", 1, "invalid_cursor")]:
        assert (await t.slopsearx_read_entities(value, page))["error"]["code"] == code
    monkeypatch.setattr("slopsearx.snapshot.time.time", lambda: 1e12)
    assert (await t.slopsearx_read_entities(cursor))["error"]["code"] == "expired_handle"
    state.snapshots = SnapshotStore(None)
    assert (await t.slopsearx_read_entities(cursor))["error"]["code"] == "store_unavailable"


@pytest.mark.parametrize(
    "engine,query,upstream,namespace",
    [
        ("npm", "pkg", {"objects": [{"package": {"name": "pkg", "version": "1"}}]}, "npm"),
        ("pypi", "pkg", {"info": {"name": "pkg", "version": "1", "summary": "example"}}, "pypi"),
        ("nvd", "CVE-2024-12345", {"vulnerabilities": [{"cve": {"id": "CVE-2024-12345"}}]}, "cve"),
        (
            "cve",
            "CVE-2024-12345",
            {"containers": {"cna": {"descriptions": [{"lang": "en", "value": "example"}]}}},
            "cve",
        ),
    ],
)
async def test_real_adapter_cache_snapshot_group_roundtrip(engine, query, upstream, namespace):
    state = _build_state([engine])
    adapter = discover_engines({engine: {"enabled": True}})[engine]
    state.ctx.active_engines[engine] = adapter
    set_state(state)
    calls = []

    def handler(request):
        calls.append(str(request.url))
        return httpx.Response(200, json=upstream)

    try:
        async with MockHTTP(handler):
            first = await t.slopsearx_search(query, engines=[engine])
            assert first["results"], first
            grouped = await t.slopsearx_read_entities(first["meta"]["cursor"])
            assert grouped["entities"][0]["namespace"] == namespace
            # Make the cache take a real JSON serialization round trip.
            state.ctx.cache._data = json.loads(json.dumps(state.ctx.cache._data))
            cached = await t.slopsearx_search(query, engines=[engine])
            assert len(calls) == 1
            again = await t.slopsearx_read_entities(cached["meta"]["cursor"])
            assert again["entities"][0]["entity_id"] == grouped["entities"][0]["entity_id"]
            assert [r["url"] for r in first["results"]] == [r["url"] for r in cached["results"]]
    finally:
        set_state(None)
