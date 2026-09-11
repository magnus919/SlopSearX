"""Dependency dossier workflow contract and policy tests."""

from __future__ import annotations

import asyncio
from collections.abc import Generator
from typing import Any, cast

import pytest

import engines  # noqa: F401
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.dependency_dossier import normalize_package, normalize_repository, resolve_package_results
from slopsearx.mcp import dependency_tools as dossier
from slopsearx.mcp import tools as research_tools
from slopsearx.mcp.state import McpState, set_state
from slopsearx.payload import DOMAIN_PACKAGES, build_payload
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore


class _MemoryStore:
    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, dict[str, Any]] = {}
        self._client = _MemoryClient(self)

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _MemoryClient:
    def __init__(self, store: _MemoryStore) -> None:
        self._store = store

    async def keys(self, pattern: str) -> list[str]:
        prefix = pattern.rstrip("*")
        return [key for key in self._store._data if key.startswith(prefix)]


class _Engine(EngineAdapter):
    def __init__(
        self,
        name: str,
        results: list[SearchResult],
        *,
        status: EngineStatus = EngineStatus.OK,
    ) -> None:
        super().__init__()
        self.name = name
        self.results = results
        self.status = status
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        return AdapterResponse(
            results=self.results if self.status == EngineStatus.OK else [],
            status=self.status,
            error_message=None if self.status == EngineStatus.OK else "fixture failure",
            latency_ms=1.0,
        )


def _package_result(name: str = "requests", version: str = "2.32.5") -> SearchResult:
    return SearchResult(
        url=f"https://pypi.org/project/{name}/",
        title=f"{name} {version}",
        content="Package metadata",
        engine="pypi",
        payload=build_payload(
            DOMAIN_PACKAGES,
            "package",
            {
                "name": name,
                "version": version,
                "homepage": "https://github.com/psf/requests",
            },
            engine="pypi",
        ),
    )


def _build_state(*, nvd_status: EngineStatus = EngineStatus.OK) -> McpState:
    active: dict[str, EngineAdapter] = {
        "pypi": _Engine("pypi", [_package_result()]),
        "npm": _Engine("npm", []),
        "github": _Engine(
            "github",
            [
                SearchResult(
                    url="https://github.com/unrelated/project",
                    title="unrelated/project",
                    content="Unrelated repository",
                    engine="github",
                ),
                SearchResult(
                    url="https://github.com/psf/requests",
                    title="psf/requests",
                    content="Requested repository",
                    engine="github",
                ),
            ],
        ),
        "nvd": _Engine(
            "nvd",
            [
                SearchResult(
                    url="https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    title="CVE-2024-0001",
                    content="Advisory search lead",
                    engine="nvd",
                )
            ],
            status=nvd_status,
        ),
    }
    policy = load_mcp_policy(config_path=None)
    policy.enabled_tools.update({"dependency_dossier": True, "research": True, "security": True})
    durable = _MemoryStore()
    context = AppContext(
        active_engines=active,
        cache=None,
        tier1_engines=set(),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(context)
    snapshots = SnapshotStore(durable, ttl_seconds=policy.snapshot_ttl_seconds)
    jobs = ResearchJobStore(durable)
    runner = ResearchJobRunner(service, jobs, snapshots, catalog, policy)
    return McpState(
        ctx=context,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=jobs,
        runner=runner,
        version="test",
    )


@pytest.fixture
def state() -> Generator[McpState, None, None]:
    value = _build_state()
    set_state(value)
    yield value
    set_state(None)


async def _run_started(state: McpState, started: dict[str, Any]) -> None:
    job = await state.job_store.load(started["job_id"])
    assert job is not None
    await state.runner.run_pending(job)


def _calls(state: McpState) -> dict[str, int]:
    return {name: cast("_Engine", engine).calls for name, engine in state.ctx.active_engines.items()}


class TestIdentity:
    def test_normalizes_ecosystem_names(self) -> None:
        assert normalize_package("pypi", "Foo_Bar") == "foo-bar"
        assert normalize_package("npm", "@scope/name") == "@scope/name"
        with pytest.raises(ValueError):
            normalize_package("npm", "UpperCase")

    def test_accepts_only_canonical_github_repositories(self) -> None:
        assert normalize_repository("https://github.com/psf/requests.git") == "psf/requests"
        assert normalize_repository("psf/requests") == "psf/requests"
        with pytest.raises(ValueError):
            normalize_repository("https://example.com/psf/requests")

    def test_resolves_only_exact_attributed_package_payloads(self) -> None:
        result = resolve_package_results("pypi", "requests", [_package_result(), _package_result("requests-extra")])
        assert result["status"] == "resolved"
        assert [match["name"] for match in result["matches"]] == ["requests"]
        assert result["repository_candidates"] == ["psf/requests"]

    def test_rejects_cross_ecosystem_and_bare_repository_metadata(self) -> None:
        cross_ecosystem = _package_result()
        cross_ecosystem.engine = "npm"
        bare_metadata = _package_result()
        assert bare_metadata.payload is not None
        bare_metadata.payload["data"]["homepage"] = "psf/requests"

        rejected = resolve_package_results("pypi", "requests", [cross_ecosystem])
        resolved = resolve_package_results("pypi", "requests", [bare_metadata])

        assert rejected["status"] == "unresolved"
        assert resolved["status"] == "resolved"
        assert resolved["repository_candidates"] == []

    def test_malformed_attributed_metadata_never_crashes_resolution(self) -> None:
        malformed = _package_result()
        assert malformed.payload is not None
        malformed.payload["data"].update({"version": {}, "homepage": "https://[bad"})

        resolved = resolve_package_results("pypi", "requests", [malformed])

        assert resolved["status"] == "resolved"
        assert resolved["matches"][0]["version"] is None
        assert resolved["repository_candidates"] == []


class TestWorkflow:
    async def test_start_is_idempotent_and_conflicts_on_identity_change(self, state: McpState) -> None:
        first = await dossier.slopsearx_start_dependency_dossier(
            "pypi", "Requests", version="2.32.5", idempotency_key="dossier-1"
        )
        replay = await dossier.slopsearx_start_dependency_dossier(
            "pypi", "requests", version="2.32.5", idempotency_key="dossier-1"
        )
        conflict = await dossier.slopsearx_start_dependency_dossier(
            "pypi", "requests", version="2.31.0", idempotency_key="dossier-1"
        )
        assert first["job_id"] == replay["job_id"]
        assert replay["replay"] is True
        assert conflict["error"]["code"] == "idempotency_conflict"

    async def test_concurrent_idempotency_admission_creates_one_identity(self, state: McpState) -> None:
        first, second = await asyncio.gather(
            dossier.slopsearx_start_dependency_dossier(
                "pypi", "requests", version="2.32.5", idempotency_key="concurrent"
            ),
            dossier.slopsearx_start_dependency_dossier(
                "pypi", "requests", version="2.31.0", idempotency_key="concurrent"
            ),
        )
        successes = [item for item in (first, second) if "error" not in item]
        conflicts = [item for item in (first, second) if item.get("error", {}).get("code") == "idempotency_conflict"]
        assert len(successes) == 1
        assert len(conflicts) == 1

    async def test_completed_dossier_is_attributed_and_read_only(self, state: McpState) -> None:
        started = await dossier.slopsearx_start_dependency_dossier(
            "pypi",
            "requests",
            version="2.32.5",
            repository="psf/requests",
        )
        await _run_started(state, started)
        before = _calls(state)

        report = await dossier.slopsearx_get_dependency_dossier(started["job_id"], max_results=1)
        after = _calls(state)

        assert report["state"] == "succeeded"
        assert report["resolved_package_identity"]["status"] == "resolved"
        assert report["resolved_package_identity"]["version_match"] == "exact"
        assert report["repository_identity"]["basis"] == "caller_supplied"
        assert report["repository_identity"]["proven_package_ownership"] is False
        assert [item["url"] for item in report["sections"]["repository_records"]["results"]] == [
            "https://github.com/psf/requests"
        ]
        assert report["sections"]["advisory_leads"]["applicability"]["status"] == "not_evaluated"
        assert after == before

    async def test_requested_version_matches_among_multiple_observed_versions(self, state: McpState) -> None:
        older = _package_result(version="2.32.4")
        older.url += "?version=2.32.4"
        requested = _package_result(version="2.32.5")
        requested.url += "?version=2.32.5"
        state.ctx.active_engines["pypi"] = _Engine(
            "pypi",
            [older, requested],
        )
        started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests", version="2.32.5")
        await _run_started(state, started)

        report = await dossier.slopsearx_get_dependency_dossier(started["job_id"])

        assert report["resolved_package_identity"]["observed_versions"] == ["2.32.4", "2.32.5"]
        assert report["resolved_package_identity"]["version_match"] == "exact"

    async def test_partial_advisory_failure_uses_stable_error(self) -> None:
        state = _build_state(nvd_status=EngineStatus.ERROR)
        set_state(state)
        try:
            started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
            await _run_started(state, started)
            report = await dossier.slopsearx_get_dependency_dossier(started["job_id"])
            advisory = report["sections"]["advisory_leads"]
            assert report["partial"] is True
            assert advisory["state"] == "failed"
            assert advisory["error"] == "upstream_error"
            assert advisory["applicability"]["status"] == "not_evaluated"
        finally:
            set_state(None)

    async def test_captured_scope_policy_change_blocks_reads_retry_and_all_dispatch(self, state: McpState) -> None:
        started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
        state.policy.sensitive_engines.add("nvd")
        read = await dossier.slopsearx_get_dependency_dossier(started["job_id"])
        retry = await research_tools.slopsearx_retry_research(started["job_id"])
        job = await state.job_store.load(started["job_id"])
        assert job is not None
        await state.runner.run_pending(job)
        finished = await state.job_store.load(started["job_id"])

        assert read["error"]["code"] == "policy_rejected"
        assert retry["error"]["code"] == "policy_rejected"
        assert finished is not None and finished.state == "failed"
        assert all(query.error and query.error.startswith("policy_rejected:") for query in finished.queries)
        assert all(count == 0 for count in _calls(state).values())

    async def test_generic_extend_rejects_dossier(self, state: McpState) -> None:
        started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
        result = await research_tools.slopsearx_extend_research(started["job_id"], "more evidence")
        assert result["error"]["code"] == "invalid_input"

    async def test_retry_stops_at_cumulative_adapter_call_budget(self) -> None:
        state = _build_state(nvd_status=EngineStatus.ERROR)
        set_state(state)
        try:
            started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
            await _run_started(state, started)
            await research_tools.slopsearx_retry_research(started["job_id"])
            await research_tools.slopsearx_retry_research(started["job_id"])
            exhausted = await research_tools.slopsearx_retry_research(started["job_id"])

            assert exhausted["error"]["code"] == "budget_exceeded"
            assert cast("_Engine", state.ctx.active_engines["nvd"]).calls == 3
        finally:
            set_state(None)

    async def test_result_budget_preserves_full_snapshot(self, state: McpState) -> None:
        state.policy.job_max_results = 1
        pypi = cast("_Engine", state.ctx.active_engines["pypi"])
        pypi.results = [
            SearchResult(
                url=f"https://pypi.org/project/requests/{index}",
                title=f"requests record {index}",
                content="Package metadata",
                engine="pypi",
                payload=_package_result().payload,
            )
            for index in range(3)
        ]
        started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
        await _run_started(state, started)
        job = await state.job_store.load(started["job_id"])
        assert job is not None and job.queries[0].cursor is not None
        snapshot = await state.snapshots.read(job.queries[0].cursor)
        report = await dossier.slopsearx_get_dependency_dossier(started["job_id"])

        assert snapshot.snapshot is not None and len(snapshot.snapshot.results) == 3
        assert len(report["sections"]["package_information"]["results"]) == 1
        assert report["budget"]["captured_results"] == 1

    async def test_predispatch_charge_survives_interrupted_execution(
        self, state: McpState, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        state.policy.job_max_queries = 2
        started = await dossier.slopsearx_start_dependency_dossier("pypi", "requests")
        job = await state.job_store.load(started["job_id"])
        assert job is not None

        async def interrupted(_request: Any) -> Any:
            raise asyncio.CancelledError

        monkeypatch.setattr(state.runner._service, "search", interrupted)
        with pytest.raises(asyncio.CancelledError):
            await state.runner.run_pending(job)
        charged = await state.job_store.load(started["job_id"])
        assert charged is not None
        assert charged.workflow["budget"]["used_adapter_calls"] == 1
        assert charged.queries[0].attempts == []
