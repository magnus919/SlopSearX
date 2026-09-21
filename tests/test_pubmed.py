"""Deterministic stage and payload tests for the PubMed adapter."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from engines.pubmed import PubMedAdapter
from slopsearx.adapter import EngineStatus
from slopsearx.service import AppContext, SearchRequest, SearchService


class FakeClock:
    def __init__(self) -> None:
        self.value = 100.0

    def monotonic(self) -> float:
        return self.value

    def advance(self, seconds: float) -> None:
        self.value += seconds


@dataclass
class Stage:
    payload: Any = None
    elapsed: float = 0.0
    parse_elapsed: float = 0.0
    status_code: int = 200
    timeout: bool = False


class PubMedFixture:
    """Fake two-stage client with clocked, request-level timeout behavior."""

    def __init__(self, stages: dict[str, Stage]) -> None:
        self.stages = stages
        self.clock = FakeClock()
        self.calls: list[tuple[str, dict[str, Any]]] = []
        self.timeouts: list[float] = []
        self.client_timeout: float | None = None

    async def get(
        self,
        url: str,
        *,
        params: dict[str, Any],
        headers: dict[str, str],
        timeout: float,
    ) -> httpx.Response:
        del headers
        stage_name = "esearch" if url.endswith("esearch.fcgi") else "esummary"
        stage = self.stages[stage_name]
        self.calls.append((stage_name, params))
        self.timeouts.append(timeout)
        if stage.timeout or stage.elapsed > timeout:
            self.clock.advance(timeout)
            raise httpx.ReadTimeout(f"{stage_name} timeout")
        self.clock.advance(stage.elapsed)
        response = httpx.Response(stage.status_code, json=stage.payload, request=httpx.Request("GET", url))
        if stage.parse_elapsed:
            original_json = response.json

            def delayed_json() -> Any:
                self.clock.advance(stage.parse_elapsed)
                return original_json()

            response.json = delayed_json  # type: ignore[method-assign]
        return response


class FixtureContext:
    def __init__(self, fixture: PubMedFixture, client_timeout: float) -> None:
        self.fixture = fixture
        self.fixture.client_timeout = client_timeout

    async def __aenter__(self) -> PubMedFixture:
        return self.fixture

    async def __aexit__(self, *args: object) -> None:
        return None


@pytest.fixture
def adapter() -> PubMedAdapter:
    return PubMedAdapter({"timeout_ms": 5_000, "max_results": 5})


async def run_fixture(monkeypatch: pytest.MonkeyPatch, adapter: PubMedAdapter, fixture: PubMedFixture):
    from types import SimpleNamespace

    from engines import pubmed

    monkeypatch.setattr(pubmed, "time", SimpleNamespace(monotonic=fixture.clock.monotonic))
    monkeypatch.setattr(
        adapter,
        "http_client",
        lambda **kwargs: FixtureContext(fixture, float(kwargs["timeout"])),
    )
    return await adapter.search("type 2 diabetes")


def esearch(ids: list[str]) -> Stage:
    return Stage(payload={"esearchresult": {"idlist": ids}})


def summary(*entries: tuple[str, dict[str, Any]]) -> Stage:
    return Stage(payload={"result": {"uids": [pmid for pmid, _ in entries], **dict(entries)}})


def test_fixture_measurement_shows_independent_stages_exceed_shared_budget():
    """The observed 4.2s + 3.8s stages exceed the shared 5s dispatch budget."""
    fixture = PubMedFixture(
        {
            "esearch": Stage(elapsed=4.2),
            "esummary": Stage(elapsed=3.8),
        }
    )

    fixture.clock.advance(fixture.stages["esearch"].elapsed)
    fixture.clock.advance(fixture.stages["esummary"].elapsed)

    assert fixture.clock.value - 100 == pytest.approx(8.0)
    assert fixture.stages["esearch"].elapsed <= 5.0
    assert fixture.stages["esummary"].elapsed <= 5.0
    assert fixture.clock.value - 100 > 5.0


async def test_aggregate_deadline_fits_shared_dispatch_budget(monkeypatch, adapter):
    """The default aggregate budget leaves headroom under timeout_ms."""
    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}, elapsed=5.2),
            "esummary": Stage(
                payload=summary(("1", {"title": "One"})).payload,
                elapsed=3.8,
            ),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.TIMEOUT
    assert response.latency_ms == pytest.approx(4_500)
    assert fixture.timeouts == [pytest.approx(4.5)]
    assert fixture.clock.value == pytest.approx(104.5)


async def test_aggregate_deadline_bounds_second_stage(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}, elapsed=4.2),
            "esummary": Stage(payload=summary(("1", {"title": "One"})).payload, elapsed=3.8),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.TIMEOUT
    assert response.results == []
    assert response.latency_ms == pytest.approx(4_500)
    assert fixture.timeouts == [pytest.approx(4.5), pytest.approx(0.3)]
    assert fixture.clock.value == pytest.approx(104.5)


async def test_aggregate_deadline_covers_response_parsing(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}, elapsed=0.2),
            "esummary": Stage(
                payload=summary(("1", {"title": "One"})).payload,
                elapsed=0.2,
                parse_elapsed=4.2,
            ),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.TIMEOUT
    assert response.results == []
    assert response.latency_ms == pytest.approx(4_600)


async def test_service_dispatch_deadline_is_not_the_timeout_observed_by_pubmed(monkeypatch, adapter):
    from types import SimpleNamespace

    from engines import pubmed

    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}, elapsed=4.2),
            "esummary": Stage(payload=summary(("1", {"title": "One"})).payload, elapsed=3.8),
        }
    )
    monkeypatch.setattr(pubmed, "time", SimpleNamespace(monotonic=fixture.clock.monotonic))
    monkeypatch.setattr(
        adapter,
        "http_client",
        lambda **kwargs: FixtureContext(fixture, float(kwargs["timeout"])),
    )

    service = SearchService(AppContext(active_engines={"pubmed": adapter}))
    assert service._resolve_engine_timeout_s(adapter) == 5.0
    response = await asyncio.wait_for(
        service.search(SearchRequest("type 2 diabetes", engines=["pubmed"], generate_suggestions=False)),
        timeout=0.5,
    )

    assert response.engine_outcomes[0].status == EngineStatus.TIMEOUT.value
    assert not response.deadline_exceeded
    assert fixture.client_timeout == 5.0
    assert fixture.timeouts == [pytest.approx(4.5), pytest.approx(0.3)]


async def test_success_preserves_two_stage_request_semantics(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1", "2"]}}, elapsed=0.2),
            "esummary": Stage(
                payload=summary(
                    ("1", {"title": "One", "source": "Journal", "pubdate": "2024", "authors": [{"name": "A"}]}),
                    ("2", {"title": "Two"}),
                ).payload,
                elapsed=0.3,
            ),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.OK
    assert [result.title for result in response.results] == ["One", "Two"]
    assert response.results[0].content == "Journal — A"
    assert fixture.calls[0][0] == "esearch"
    assert fixture.calls[0][1] == {
        "db": "pubmed",
        "term": "type 2 diabetes",
        "retmax": 5,
        "retmode": "json",
        "sort": "relevance",
    }
    assert fixture.calls[1][0] == "esummary"
    assert fixture.calls[1][1] == {"db": "pubmed", "id": "1,2", "retmode": "json"}


async def test_valid_empty_id_list_is_ok_and_skips_esummary(monkeypatch, adapter):
    fixture = PubMedFixture({"esearch": esearch([]), "esummary": Stage()})

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.OK
    assert response.results == []
    assert [stage for stage, _ in fixture.calls] == ["esearch"]


async def test_esearch_timeout_is_classified(monkeypatch, adapter):
    fixture = PubMedFixture({"esearch": Stage(elapsed=5.0, timeout=True), "esummary": Stage()})

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.TIMEOUT
    assert [stage for stage, _ in fixture.calls] == ["esearch"]


async def test_esummary_timeout_is_classified(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}, elapsed=0.5),
            "esummary": Stage(elapsed=5.0, timeout=True),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.TIMEOUT
    assert [stage for stage, _ in fixture.calls] == ["esearch", "esummary"]


@pytest.mark.parametrize(
    "payload",
    [
        [],
        {"esearchresult": None},
        {"esearchresult": {}},
        {"esearchresult": {"idlist": "1"}},
        {"esearchresult": {"idlist": [1]}},
    ],
)
async def test_malformed_esearch_payload_is_error(monkeypatch, adapter, payload):
    fixture = PubMedFixture({"esearch": Stage(payload=payload), "esummary": Stage()})

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.ERROR
    assert response.results == []


@pytest.mark.parametrize("payload", [[], {"result": None}, {"result": []}])
async def test_malformed_esummary_payload_is_error(monkeypatch, adapter, payload):
    fixture = PubMedFixture(
        {
            "esearch": esearch(["1"]),
            "esummary": Stage(payload=payload),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.ERROR
    assert response.results == []


@pytest.mark.parametrize(
    ("config_key", "value"),
    [
        ("aggregate_timeout_ms", 0),
        ("aggregate_timeout_ms", -1),
        ("aggregate_timeout_ms", "not-a-number"),
        ("aggregate_timeout_ms", float("nan")),
        ("aggregate_timeout_ms", 6_000),
        ("timeout_ms", 0),
        ("timeout_ms", "not-a-number"),
    ],
)
async def test_invalid_timeout_configuration_never_raises(monkeypatch, adapter, config_key, value):
    adapter.config[config_key] = value
    fixture = PubMedFixture({"esearch": esearch(["1"]), "esummary": Stage()})

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.ERROR
    assert response.results == []
    assert fixture.calls == []


async def test_missing_summary_entry_returns_valid_partial_results(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": esearch(["1", "2"]),
            "esummary": summary(("1", {"title": "One"})),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.OK
    assert [result.title for result in response.results] == ["One"]
    assert response.results[0].position == 1


async def test_missing_first_summary_entry_preserves_upstream_position(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": esearch(["1", "2"]),
            "esummary": summary(("2", {"title": "Two"})),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.OK
    assert [result.title for result in response.results] == ["Two"]
    assert response.results[0].position == 2


async def test_malformed_summary_entry_is_error_not_a_blank_result(monkeypatch, adapter):
    fixture = PubMedFixture(
        {
            "esearch": esearch(["1"]),
            "esummary": Stage(payload={"result": {"uids": ["1"], "1": None}}),
        }
    )

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.ERROR
    assert response.results == []


@pytest.mark.parametrize("stage_name", ["esearch", "esummary"])
async def test_rate_limit_is_preserved_for_either_stage(monkeypatch, adapter, stage_name):
    stages = {
        "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}),
        "esummary": Stage(payload=summary(("1", {"title": "One"})).payload),
    }
    stages[stage_name].status_code = 429
    fixture = PubMedFixture(stages)

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.RATE_LIMITED


@pytest.mark.parametrize("stage_name", ["esearch", "esummary"])
async def test_other_http_error_is_preserved_for_either_stage(monkeypatch, adapter, stage_name):
    stages = {
        "esearch": Stage(payload={"esearchresult": {"idlist": ["1"]}}),
        "esummary": Stage(payload=summary(("1", {"title": "One"})).payload),
    }
    stages[stage_name].status_code = 500
    fixture = PubMedFixture(stages)

    response = await run_fixture(monkeypatch, adapter, fixture)

    assert response.status == EngineStatus.ERROR
