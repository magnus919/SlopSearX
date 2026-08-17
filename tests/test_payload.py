"""Tests for versioned, JSON-safe domain payloads on normalized results.

Covers the payload envelope contract, provenance, cache/snapshot
serialization round-trips, progressive disclosure on MCP cards/records, and
representative real-adapter fixtures for each initial domain family. Absent
source fields must stay absent — never fabricated as null/false/empty.
"""

from __future__ import annotations

from typing import Any

import httpx

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.payload import (
    DOMAIN_BIOMEDICAL,
    DOMAIN_FINANCIAL,
    DOMAIN_JOBS,
    DOMAIN_MEDIA,
    DOMAIN_PACKAGES,
    DOMAIN_SCIENCE,
    DOMAIN_SECURITY,
    build_payload,
    is_valid_payload,
    payload_from_dict,
    payload_to_dict,
)
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import (
    AppContext,
    SearchService,
    search_response_from_payload,
    search_response_to_payload,
    search_result_from_dict,
    search_result_to_dict,
)
from slopsearx.snapshot import SnapshotStore
from tests.test_adapters import MockHTTP

# ---------------------------------------------------------------------------
# Contract tests
# ---------------------------------------------------------------------------


class TestPayloadContract:
    def test_build_payload_is_self_describing(self) -> None:
        payload = build_payload(
            DOMAIN_SECURITY,
            "vulnerability",
            {"cve_id": "CVE-2024-12345", "cvss": {"score": 9.8}},
            engine="nvd",
        )

        assert is_valid_payload(payload)
        assert payload["domain"] == "security"
        assert payload["type"] == "vulnerability"
        assert payload["schema_version"] == 1
        assert payload["data"] == {"cve_id": "CVE-2024-12345", "cvss": {"score": 9.8}}

    def test_build_payload_drops_none_fields(self) -> None:
        """Absent source fields stay absent rather than materializing as null."""
        payload = build_payload(
            DOMAIN_SECURITY,
            "vulnerability",
            {"cve_id": "CVE-2024-1", "cvss": None, "cwe_ids": None, "references": None},
            engine="nvd",
        )

        assert payload["data"] == {"cve_id": "CVE-2024-1"}
        assert "cvss" not in payload["data"]
        assert "cwe_ids" not in payload["data"]
        assert "references" not in payload["data"]
        assert payload["provenance"]["adapter_fields"] == ["cve_id"]

    def test_build_payload_preserves_falsey_values(self) -> None:
        """Falsey-but-present values (0, "", []) are retained, not dropped."""
        payload = build_payload(
            DOMAIN_MEDIA,
            "media_item",
            {"vote_average": 0.0, "overview": "", "release_date": None},
            engine="tmdb",
        )

        assert payload["data"] == {"vote_average": 0.0, "overview": ""}
        assert "release_date" not in payload["data"]

    def test_provenance_distinguishes_field_kinds(self) -> None:
        payload = build_payload(
            DOMAIN_SCIENCE,
            "publication",
            {"publication_id": "2401.00001", "abstract": "An abstract."},
            engine="arxiv",
            normalized_fields=("url", "title", "content"),
            inferred_fields=("tier", "score"),
        )

        provenance = payload["provenance"]
        assert provenance["engine"] == "arxiv"
        assert provenance["adapter_fields"] == ["publication_id", "abstract"]
        assert provenance["normalized_fields"] == ["url", "title", "content"]
        assert provenance["inferred_fields"] == ["tier", "score"]

    def test_payload_to_from_dict_round_trip_types_and_provenance(self) -> None:
        payload = build_payload(
            DOMAIN_FINANCIAL,
            "economic_series",
            {
                "series_id": "GDP",
                "popularity_rank": 3,
                "score": 0.95,
                "active": True,
                "tags": ["macro", "quarterly"],
                "nested": {"key": "value", "count": 2},
            },
            engine="fred",
            inferred_fields=("score",),
        )

        serialized = payload_to_dict(payload)
        assert serialized is not None
        assert serialized == payload

        rehydrated = payload_from_dict(serialized)
        assert rehydrated == payload
        assert isinstance(rehydrated["data"]["popularity_rank"], int)
        assert isinstance(rehydrated["data"]["score"], float)
        assert isinstance(rehydrated["data"]["active"], bool)
        assert rehydrated["provenance"]["inferred_fields"] == ["score"]

    def test_payload_json_safe_canonicalizes_sets_and_tuples(self) -> None:
        payload = build_payload(
            DOMAIN_PACKAGES,
            "package",
            {"name": "x", "tags": {"a", "b"}, "tuple": (1, 2)},
            engine="pypi",
        )

        serialized = payload_to_dict(payload)
        assert serialized is not None
        assert serialized["data"]["tags"] == ["a", "b"]
        assert serialized["data"]["tuple"] == [1, 2]

    def test_payload_from_dict_rejects_non_dict(self) -> None:
        assert payload_from_dict(None) is None
        assert payload_from_dict("not-a-payload") is None
        assert payload_from_dict(123) is None

    def test_is_valid_payload_rejects_incomplete(self) -> None:
        assert is_valid_payload(None) is False
        assert is_valid_payload({"domain": "security"}) is False
        assert is_valid_payload({"domain": "security", "type": "vulnerability"}) is False
        assert is_valid_payload({"domain": "security", "type": "vulnerability", "schema_version": 1}) is False


# ---------------------------------------------------------------------------
# Cache / snapshot serialization round-trips
# ---------------------------------------------------------------------------


class _FakeStore:
    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        if not self.is_connected:
            return None
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        if self.is_connected:
            self._data[key] = value


def _result_with_payload() -> SearchResult:
    return SearchResult(
        url="https://nvd.nist.gov/vuln/detail/CVE-2024-12345",
        title="CVE-2024-12345",
        content="buffer overflow",
        engine="nvd",
        engines={"nvd", "cve"},
        score=2.0,
        position=1,
        category="security",
        published_date="2024-03-15",
        tier=2,
        payload=build_payload(
            DOMAIN_SECURITY,
            "vulnerability",
            {
                "cve_id": "CVE-2024-12345",
                "cvss": {"score": 9.8, "severity": "CRITICAL", "vector": "CVSS:3.1/..."},
                "cwe_ids": ["CWE-120"],
                "references": ["https://example.com/advisory"],
            },
            engine="nvd",
        ),
    )


class TestPayloadSerialization:
    def test_result_dict_round_trips_payload_exactly(self) -> None:
        original = _result_with_payload()
        payload = search_result_to_dict(original)

        assert isinstance(payload["payload"], dict)
        assert payload["payload"]["domain"] == "security"

        rebuilt = search_result_from_dict(payload)
        assert rebuilt.payload == original.payload
        assert rebuilt.engines == {"nvd", "cve"}

    def test_response_round_trips_payload_exactly(self) -> None:
        from slopsearx.service import EngineOutcome, ScopeDecision, SearchResponse

        response = SearchResponse(
            query="cve",
            results=[_result_with_payload()],
            scope=ScopeDecision(selected_engines=["nvd"]),
            engine_outcomes=[EngineOutcome(engine="nvd", status="ok", result_count=1, latency_ms=1.0, message=None)],
        )

        payload = search_response_to_payload(response)
        rebuilt = search_response_from_payload(payload)

        assert rebuilt.results[0].payload == response.results[0].payload
        assert rebuilt.results[0].payload is not None
        assert rebuilt.results[0].payload["provenance"]["engine"] == "nvd"

    def test_result_to_dict_does_not_deep_copy_via_asdict(self, monkeypatch) -> None:
        """The cache-write path must not deep-copy results via dataclasses.asdict."""
        import slopsearx.service as service_module

        def _fail(*_args: Any, **_kwargs: Any) -> Any:
            raise AssertionError("dataclasses.asdict must not be used on the cache-write path")

        monkeypatch.setattr(service_module.dataclasses, "asdict", _fail)
        payload = search_result_to_dict(_result_with_payload())
        assert payload["payload"]["domain"] == "security"
        assert payload["engines"] == ["cve", "nvd"]

    def test_response_to_payload_does_not_deep_copy_via_asdict(self, monkeypatch) -> None:
        """The cache-write path must not deep-copy the response via dataclasses.asdict."""
        import slopsearx.service as service_module
        from slopsearx.service import EngineOutcome, ScopeDecision, SearchResponse

        def _fail(*_args: Any, **_kwargs: Any) -> Any:
            raise AssertionError("dataclasses.asdict must not be used on the cache-write path")

        monkeypatch.setattr(service_module.dataclasses, "asdict", _fail)
        response = SearchResponse(
            query="cve",
            results=[_result_with_payload()],
            scope=ScopeDecision(selected_engines=["nvd"]),
            engine_outcomes=[EngineOutcome(engine="nvd", status="ok", result_count=1, latency_ms=1.0, message=None)],
        )
        payload = search_response_to_payload(response)
        assert payload["results"][0]["payload"]["domain"] == "security"
        assert payload["cached"] is False

    def test_result_without_payload_remains_valid(self) -> None:
        result = SearchResult(
            url="https://example.com",
            title="plain",
            content="no payload",
            engine="brave",
            engines={"brave"},
        )
        payload = search_result_to_dict(result)
        assert payload["payload"] is None

        rebuilt = search_result_from_dict(payload)
        assert rebuilt.payload is None

    async def test_snapshot_round_trips_payload(self) -> None:
        from slopsearx.service import ScopeDecision

        store = _FakeStore()
        snapshots = SnapshotStore(store, tenant="t1", ttl_seconds=120)
        snapshot_id = await snapshots.create(
            "cve", "ssx-1", [_result_with_payload()], ScopeDecision(selected_engines=["nvd"])
        )

        snapshot = await snapshots.get(snapshot_id)
        assert snapshot is not None
        assert snapshot.results[0].payload == _result_with_payload().payload


# ---------------------------------------------------------------------------
# MCP progressive disclosure
# ---------------------------------------------------------------------------


class _PayloadEngine(EngineAdapter):
    """Mock engine returning one result with a fixed domain payload."""

    def __init__(self, name: str, payload: dict[str, Any] | None, content: str = "snippet") -> None:
        super().__init__()
        self.name = name
        self._payload = payload
        self._content = content
        self.categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        return AdapterResponse(
            results=[
                SearchResult(
                    url="https://example.com/result",
                    title="Payload result",
                    content=self._content,
                    engine=self.name,
                    engines={self.name},
                    score=1.0,
                    position=1,
                    payload=self._payload,
                )
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


def _mcp_state(engine: EngineAdapter) -> McpState:
    policy: MCPPolicy = load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines={engine.name: engine},
        router=None,
        cache=_FakeStore(),
        tier1_engines={engine.name},
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
    store = _FakeStore()
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )


class TestPayloadDisclosure:
    async def test_small_payload_inlined_on_card(self) -> None:
        payload = build_payload(DOMAIN_PACKAGES, "package", {"name": "requests", "version": "2.31.0"}, engine="brave")
        state = _mcp_state(_PayloadEngine("brave", payload))
        set_state(state)
        try:
            result = await t.slopsearx_search("requests", engines=["brave"])
            assert result["results"][0]["payload"] == payload
        finally:
            set_state(None)

    async def test_large_payload_omitted_from_card_unless_requested(self) -> None:
        payload = build_payload(
            DOMAIN_SCIENCE,
            "publication",
            {"publication_id": "2401.00001", "abstract": "A" * 2000},
            engine="brave",
        )
        state = _mcp_state(_PayloadEngine("brave", payload))
        set_state(state)
        try:
            result = await t.slopsearx_search("paper", engines=["brave"])
            assert "payload" not in result["results"][0]

            requested = await t.slopsearx_search("paper", engines=["brave"], include=["results", "payload"])
            assert requested["results"][0]["payload"] == payload
        finally:
            set_state(None)

    async def test_unserializable_payload_does_not_crash_search_and_is_omitted(self) -> None:
        payload: dict[str, Any] = {"domain": "security", "type": "vulnerability", "data": {}}
        payload["self"] = payload  # circular reference — cannot be JSON-serialized

        state = _mcp_state(_PayloadEngine("brave", payload))
        set_state(state)
        try:
            result = await t.slopsearx_search("cve", engines=["brave"])
            assert "error" not in result
            assert len(result["results"]) == 1
            assert result["results"][0]["title"] == "Payload result"
            assert "payload" not in result["results"][0]
        finally:
            set_state(None)

    async def test_read_result_returns_full_payload(self) -> None:
        payload = build_payload(
            DOMAIN_SCIENCE,
            "publication",
            {"publication_id": "2401.00001", "abstract": "A" * 2000},
            engine="brave",
        )
        state = _mcp_state(_PayloadEngine("brave", payload))
        set_state(state)
        try:
            result = await t.slopsearx_search("paper", engines=["brave"])
            card = result["results"][0]
            assert "payload" not in card  # too large to inline

            expanded = await t.slopsearx_read_result(card["result_id"])
            assert expanded["payload"] == payload
            assert expanded["payload"]["data"]["abstract"] == "A" * 2000
        finally:
            set_state(None)

    async def test_record_without_payload_reports_none(self) -> None:
        state = _mcp_state(_PayloadEngine("brave", None))
        set_state(state)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            expanded = await t.slopsearx_read_result(result["results"][0]["result_id"])
            assert expanded["payload"] is None
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# Representative real-adapter fixtures (one per initial domain family)
# ---------------------------------------------------------------------------


class TestSecurityPayload:
    async def test_nvd_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"nvd": {"enabled": True}})["nvd"]
        response = {
            "vulnerabilities": [
                {
                    "cve": {
                        "id": "CVE-2024-12345",
                        "published": "2024-03-15T10:00:00.000",
                        "descriptions": [{"lang": "en", "value": "A buffer overflow in Example Software."}],
                        "metrics": {
                            "cvssMetricV31": [
                                {
                                    "cvssData": {
                                        "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                                        "baseScore": 9.8,
                                        "baseSeverity": "CRITICAL",
                                    }
                                }
                            ]
                        },
                        "weaknesses": [
                            {"description": [{"lang": "en", "value": "CWE-120"}]},
                        ],
                        "references": [{"url": "https://example.com/advisory"}],
                    }
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("CVE-2024-12345")

        payload = result.results[0].payload
        assert payload is not None
        assert is_valid_payload(payload)
        assert payload["domain"] == DOMAIN_SECURITY
        assert payload["type"] == "vulnerability"
        assert payload["provenance"]["engine"] == "nvd"
        assert payload["data"]["cve_id"] == "CVE-2024-12345"
        assert payload["data"]["cvss"] == {
            "score": 9.8,
            "severity": "CRITICAL",
            "vector": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
            "version": "3.1",
        }
        assert payload["data"]["cwe_ids"] == ["CWE-120"]
        assert payload["data"]["references"] == ["https://example.com/advisory"]

    async def test_nvd_absent_fields_stay_absent(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"nvd": {"enabled": True}})["nvd"]
        response = {
            "vulnerabilities": [
                {
                    "cve": {
                        "id": "CVE-2024-99999",
                        "descriptions": [{"lang": "en", "value": "Minimal record."}],
                    }
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("CVE-2024-99999")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["data"]["cve_id"] == "CVE-2024-99999"
        assert payload["data"]["description"] == "Minimal record."
        assert "cvss" not in payload["data"]
        assert "cwe_ids" not in payload["data"]
        assert "references" not in payload["data"]

    async def test_cve_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"cve": {"enabled": True}})["cve"]
        record = {
            "dataType": "CVE_RECORD",
            "dataVersion": "5.2",
            "cveId": "CVE-2024-12345",
            "containers": {
                "cna": {
                    "datePublic": "2024-02-01",
                    "descriptions": [{"lang": "en", "value": "Authoritative MITRE description."}],
                    "metrics": [
                        {
                            "cvssV3_1": {
                                "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                                "baseScore": 9.8,
                                "baseSeverity": "CRITICAL",
                            }
                        }
                    ],
                    "references": [{"url": "https://example.com/advisory"}],
                }
            },
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=record)):
            result = await adapter.search("CVE-2024-12345")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_SECURITY
        assert payload["type"] == "vulnerability"
        assert payload["provenance"]["engine"] == "cve"
        assert payload["data"]["cve_id"] == "CVE-2024-12345"
        assert payload["data"]["cvss"]["score"] == 9.8
        assert payload["data"]["references"] == ["https://example.com/advisory"]


class TestSciencePayload:
    ARXIV_ATOM = """<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/2401.00001v1</id>
    <title>Attention Is All You Need Revisited</title>
    <summary>  We revisit\nthe attention mechanism\n across domains.  </summary>
    <published>2024-01-05T00:00:00Z</published>
  </entry>
</feed>
"""

    async def test_arxiv_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"arxiv": {"enabled": True}})["arxiv"]
        async with MockHTTP(lambda r: httpx.Response(200, text=self.ARXIV_ATOM)):
            result = await adapter.search("attention")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_SCIENCE
        assert payload["type"] == "publication"
        assert payload["provenance"]["engine"] == "arxiv"
        assert payload["data"]["publication_id"] == "2401.00001v1"
        assert payload["data"]["abstract"] == "We revisit the attention mechanism across domains."
        # arXiv adapter does not parse authors/journal — absent, not fabricated.
        assert "authors" not in payload["data"]
        assert "journal" not in payload["data"]

    async def test_pubmed_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"pubmed": {"enabled": True}})["pubmed"]

        def _handler(r):
            if "esearch" in str(r.url):
                return httpx.Response(200, json={"esearchresult": {"idlist": ["12345"]}})
            return httpx.Response(
                200,
                json={
                    "result": {
                        "12345": {
                            "title": "A biomedical study",
                            "source": "Nature",
                            "pubdate": "2024-05-01",
                            "authors": [{"name": "Alice"}, {"name": "Bob"}],
                        }
                    }
                },
            )

        async with MockHTTP(_handler):
            result = await adapter.search("biomedical")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_SCIENCE
        assert payload["type"] == "publication"
        assert payload["provenance"]["engine"] == "pubmed"
        assert payload["data"]["publication_id"] == "12345"
        assert payload["data"]["journal"] == "Nature"
        assert payload["data"]["authors"] == ["Alice", "Bob"]
        # esummary provides no abstract — absent, not fabricated.
        assert "abstract" not in payload["data"]


class TestPackagesPayload:
    async def test_pypi_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"pypi": {"enabled": True}})["pypi"]
        response = {
            "info": {
                "name": "requests",
                "version": "2.31.0",
                "summary": "Python HTTP for Humans.",
                "license": "Apache-2.0",
                "home_page": "https://requests.readthedocs.io",
            }
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("requests")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_PACKAGES
        assert payload["type"] == "package"
        assert payload["provenance"]["engine"] == "pypi"
        assert payload["data"] == {
            "name": "requests",
            "version": "2.31.0",
            "summary": "Python HTTP for Humans.",
            "license": "Apache-2.0",
            "homepage": "https://requests.readthedocs.io",
        }

    async def test_pypi_missing_license_and_homepage_stay_absent(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"pypi": {"enabled": True}})["pypi"]
        response = {"info": {"name": "requests", "version": "2.31.0", "summary": "HTTP for Humans."}}

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("requests")

        payload = result.results[0].payload
        assert payload is not None
        assert "license" not in payload["data"]
        assert "homepage" not in payload["data"]

    async def test_npm_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"npm": {"enabled": True}})["npm"]
        response = {
            "objects": [
                {
                    "package": {"name": "express", "version": "4.18.2", "description": "Fast web framework"},
                    "score": {"detail": {"popularity": 0.9}},
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("express")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_PACKAGES
        assert payload["type"] == "package"
        assert payload["data"]["name"] == "express"
        assert payload["data"]["version"] == "4.18.2"
        assert payload["data"]["summary"] == "Fast web framework"


class TestJobsPayload:
    async def test_greenhouse_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"greenhouse": {"enabled": True}})["greenhouse"]
        response = {
            "jobs": [
                {
                    "id": 123,
                    "title": "Senior AI Engineer",
                    "absolute_url": "https://boards.greenhouse.io/anthropic/jobs/123",
                    "offices": [{"name": "San Francisco, CA"}],
                    "metadata": [{"name": "Salary", "value": "$200k-$280k"}],
                    "updated_at": "2026-07-01T12:00:00Z",
                },
                {
                    "id": 456,
                    "title": "ML Research Scientist",
                    "absolute_url": "https://boards.greenhouse.io/anthropic/jobs/456",
                    "offices": [{"name": "New York, NY"}],
                    "metadata": [],
                    "updated_at": "2026-07-02T12:00:00Z",
                },
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("Engineer at Anthropic")

        first = result.results[0].payload
        assert first is not None
        assert first["domain"] == DOMAIN_JOBS
        assert first["type"] == "job"
        assert first["provenance"]["engine"] == "greenhouse"
        assert first["data"]["company"] == "Anthropic"
        assert first["data"]["title"] == "Senior AI Engineer"
        assert first["data"]["location"] == "San Francisco, CA"
        assert first["data"]["salary"] == "$200k-$280k"
        assert first["data"]["job_id"] == 123

        second = result.results[1].payload
        assert second is not None
        assert second["data"]["title"] == "ML Research Scientist"
        # No salary metadata — absent, not fabricated.
        assert "salary" not in second["data"]


class TestMediaPayload:
    async def test_tmdb_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"tmdb": {"enabled": True, "api_key": "test-tmdb-key"}})["tmdb"]
        response = {
            "results": [
                {
                    "media_type": "movie",
                    "title": "Test Movie",
                    "release_date": "2024-01-15",
                    "overview": "A test movie about testing.",
                    "vote_average": 7.5,
                    "poster_path": "/testposter.jpg",
                    "id": 12345,
                },
                {
                    "media_type": "person",
                    "name": "Jane Director",
                    "id": 999,
                },
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("test")

        movie = result.results[0].payload
        assert movie is not None
        assert movie["domain"] == DOMAIN_MEDIA
        assert movie["type"] == "media_item"
        assert movie["provenance"]["engine"] == "tmdb"
        assert movie["data"]["media_type"] == "movie"
        assert movie["data"]["title"] == "Test Movie"
        assert movie["data"]["release_date"] == "2024-01-15"
        assert movie["data"]["vote_average"] == 7.5

        person = result.results[1].payload
        assert person is not None
        assert person["data"]["media_type"] == "person"
        # Person result has no release_date/overview/vote_average — absent.
        assert "release_date" not in person["data"]
        assert "overview" not in person["data"]
        assert "vote_average" not in person["data"]


class TestFinancialPayload:
    async def test_fred_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"fred": {"enabled": True, "api_key": "test-fred-key"}})["fred"]
        response = {
            "seriess": [
                {
                    "id": "GDP",
                    "title": "Gross Domestic Product",
                    "observation_start": "1947-01-01",
                    "units": "Billions of Dollars",
                    "frequency": "Quarterly",
                    "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
                    "popularity": 95,
                    "notes": "Gross domestic product (GDP).",
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("GDP")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_FINANCIAL
        assert payload["type"] == "economic_series"
        assert payload["provenance"]["engine"] == "fred"
        assert payload["data"]["series_id"] == "GDP"
        assert payload["data"]["title"] == "Gross Domestic Product"
        assert payload["data"]["units"] == "Billions of Dollars"
        assert payload["data"]["frequency"] == "Quarterly"
        assert payload["data"]["seasonal_adjustment"] == "Seasonally Adjusted Annual Rate"
        assert payload["data"]["observation_start"] == "1947-01-01"
        assert payload["data"]["notes"] == "Gross domestic product (GDP)."


class TestBiomedicalPayload:
    async def test_openfda_payload(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"openfda": {"enabled": True}})["openfda"]
        response = {
            "results": [
                {
                    "openfda": {
                        "brand_name": ["TestDrug"],
                        "generic_name": ["Testzol"],
                        "manufacturer_name": ["Acme Pharma"],
                        "substance_name": ["Testzol"],
                    },
                    "purpose": ["For testing"],
                    "indications_and_usage": ["Used for testing payload extraction"],
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("testdrug")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["domain"] == DOMAIN_BIOMEDICAL
        assert payload["type"] == "drug_label"
        assert payload["provenance"]["engine"] == "openfda"
        assert payload["data"]["brand_name"] == "TestDrug"
        assert payload["data"]["generic_name"] == "Testzol"
        assert payload["data"]["manufacturer"] == "Acme Pharma"
        assert payload["data"]["substance"] == "Testzol"
        assert payload["data"]["purpose"] == "For testing"
        assert payload["data"]["indications"] == "Used for testing payload extraction"

    async def test_openfda_absent_fields_stay_absent(self) -> None:
        from slopsearx.adapter import discover_engines

        adapter = discover_engines({"openfda": {"enabled": True}})["openfda"]
        response = {
            "results": [
                {
                    "openfda": {"brand_name": ["TestDrug"]},
                    "purpose": [""],
                    "indications_and_usage": [""],
                }
            ]
        }

        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("testdrug")

        payload = result.results[0].payload
        assert payload is not None
        assert payload["data"]["brand_name"] == "TestDrug"
        assert "generic_name" not in payload["data"]
        assert "manufacturer" not in payload["data"]
        assert "substance" not in payload["data"]
        assert "purpose" not in payload["data"]
        assert "indications" not in payload["data"]
