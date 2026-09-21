"""Production contracts for optional TypeSafe Jev specialist routing."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import httpx
import pytest

import engines  # noqa: F401 - populate the engine registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.jev import ROUTING_CARDS, JevRoutingDecision, JevSpecialistRouter, validate_routing_cards
from slopsearx.router import QueryRouter
from slopsearx.service import AppContext, SearchRequest, SearchService
from slopsearx.snapshot import _snapshot_from_payload


class _Engine(EngineAdapter):
    def __init__(self, name: str) -> None:
        super().__init__()
        self.name = name
        self.display_name = name.title()
        self.categories = ["general"]
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        return AdapterResponse(
            [
                SearchResult(
                    url=f"https://{self.name}.example/result",
                    title=self.name,
                    content=f"Evidence from {self.name}",
                    engine=self.name,
                )
            ],
            EngineStatus.OK,
        )


class _JevStub:
    threshold = 0.65

    async def route(self, *_args: Any, **_kwargs: Any) -> JevRoutingDecision:
        return JevRoutingDecision(["pubmed", "arxiv"], {"pubmed": 0.94, "arxiv": 0.81}, 4.0)

    def cache_identity(self) -> str:
        return "test"


class _MemoryCache:
    def __init__(self) -> None:
        self.values: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self.values.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self.values[key] = value


def test_every_registered_engine_has_routing_metadata() -> None:
    engine_names = {
        path.stem for path in (Path(__file__).parents[1] / "engines").glob("*.py") if path.stem != "__init__"
    }
    validate_routing_cards(engine_names)
    assert "brave" not in ROUTING_CARDS
    assert "pubmed" in ROUTING_CARDS


def test_api_key_presence_is_the_only_enablement_switch(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("TYPESAFE_API_KEY", raising=False)
    assert JevSpecialistRouter.from_environment() is None
    monkeypatch.setenv("TYPESAFE_API_KEY", "test-key")
    assert JevSpecialistRouter.from_environment() is not None


async def test_router_scores_specialists_in_one_request_and_applies_threshold(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        body = __import__("json").loads(request.content)
        answers = {name: {"noul": 0.9 if name == "pubmed" else 0.2} for name in body["questions"]}
        return httpx.Response(200, json={"model": "jev-1.13.0", "answers": answers})

    real_client = httpx.AsyncClient

    def client_factory(*_args: Any, **kwargs: Any) -> httpx.AsyncClient:
        kwargs["transport"] = httpx.MockTransport(handler)
        return real_client(**kwargs)

    monkeypatch.setattr("slopsearx.jev.httpx.AsyncClient", client_factory)
    active: dict[str, EngineAdapter] = {name: _Engine(name) for name in ("brave", "pubmed", "arxiv")}
    cache = _MemoryCache()
    router = JevSpecialistRouter("secret", threshold=0.65)
    decision = await router.route("medical paper", active, None, set(), cache=cache)
    cached = await router.route("medical paper", active, None, set(), cache=cache)

    assert decision is not None
    assert decision.engines == ["pubmed"]
    assert cached is not None
    assert cached.engines == ["pubmed"]
    assert cached.latency_ms == 0
    assert len(requests) == 1
    payload = __import__("json").loads(requests[0].content)
    assert set(payload["questions"]) == {"pubmed", "arxiv"}
    assert "brave" not in payload["questions"]
    assert requests[0].headers["Authorization"] == "Bearer secret"


async def test_jev_adds_every_selected_specialist_without_replacing_general_engines(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    active: dict[str, EngineAdapter] = {name: _Engine(name) for name in ("brave", "duckduckgo", "pubmed", "arxiv")}
    context = AppContext(
        active_engines=active,
        router=QueryRouter(),
        tier1_engines={"brave", "duckduckgo"},
        sensitive_engines=set(),
        jev_router=_JevStub(),  # type: ignore[arg-type]
    )
    response = await SearchService(context).search(SearchRequest(query="medical evidence"))

    assert {"brave", "duckduckgo"}.issubset(response.scope.selected_engines)
    assert response.scope.jev_added_engines == ["pubmed", "arxiv"]
    assert {"pubmed", "arxiv"}.issubset(response.scope.selected_engines)
    assert all(getattr(active[name], "calls") == 1 for name in response.scope.selected_engines)
    assert [result.engine for result in response.results[:2]] == ["pubmed", "arxiv"]

    from slopsearx import server

    monkeypatch.setattr(server, "_active_engines", active)
    portal_state = server._portal_state(  # noqa: SLF001 - assert HTTP-to-portal contract
        query="medical evidence",
        categories="",
        engine_selection="",
        language="",
        time_range="",
        safesearch=0,
        page=1,
        response=response,
    )
    assert portal_state["jev_added_engines"] == ["pubmed", "arxiv"]


async def test_explicit_scope_never_calls_jev() -> None:
    class _ExplodingJev(_JevStub):
        async def route(self, *_args: Any, **_kwargs: Any) -> JevRoutingDecision:
            raise AssertionError("Jev must not run for explicit scope")

    engine = _Engine("brave")
    context = AppContext(active_engines={"brave": engine}, jev_router=_ExplodingJev())  # type: ignore[arg-type]
    response = await SearchService(context).search(SearchRequest(query="q", engines=["brave"]))
    assert response.scope.selected_engines == ["brave"]
    assert engine.calls == 1


async def test_jev_failure_preserves_deterministic_scope(monkeypatch: pytest.MonkeyPatch) -> None:
    class BrokenClient:
        async def __aenter__(self) -> BrokenClient:
            return self

        async def __aexit__(self, *_args: Any) -> None:
            return None

        async def post(self, *_args: Any, **_kwargs: Any) -> httpx.Response:
            raise httpx.TimeoutException("timeout")

    monkeypatch.setattr("slopsearx.jev.httpx.AsyncClient", lambda **_kwargs: BrokenClient())
    active: dict[str, EngineAdapter] = {name: _Engine(name) for name in ("brave", "pubmed")}
    decision = await JevSpecialistRouter("secret").route("q", active, None, set())
    assert decision is None


def test_snapshot_rehydrates_jev_scope_provenance() -> None:
    snapshot = _snapshot_from_payload(
        {
            "snapshot_id": "snap-test",
            "query": "medical evidence",
            "query_id": "q-test",
            "results": [],
            "scope": {
                "selected_engines": ["wikipedia", "pubmed"],
                "jev_added_engines": ["pubmed"],
                "jev_scores": {"pubmed": 0.91},
            },
            "total": 0,
            "tenant": "test",
        }
    )
    assert snapshot.scope.jev_added_engines == ["pubmed"]
    assert snapshot.scope.jev_scores == {"pubmed": 0.91}
