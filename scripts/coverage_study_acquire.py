"""Inert, mock-transport-only acquisition seam for the coverage study.

There is deliberately no CLI, live transport factory, environment/key lookup,
provider call, or callback to the legacy experiment runner. All acquisition
requires a caller-supplied :class:`RecordedMockTransport`. A future live runner
requires separate source/runtime qualification and admission.

The helper uses the actual four supported adapters and shared SearchService.
The acquisition response is the canonical presence-ranked pool with reranking
and Jev routing disabled. That pool is not a qualified W0 output: a future
baseline qualification must apply the actual deployment's Jev reranking or
keyless specialist-promotion path to the frozen acquisition inputs before
comparing it with the coverage candidate.
"""

from __future__ import annotations

import contextvars
import hashlib
from collections import Counter
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Protocol
from urllib.parse import parse_qsl, urljoin, urlsplit

import httpx

from scripts import coverage_study_core as core
from slopsearx.adapter import EngineAdapter
from slopsearx.merger import _normalise_url
from slopsearx.service import AppContext, SearchRequest, SearchResponse, SearchService

ALLOWED_ENGINES = frozenset({"arxiv", "github", "openalex", "wikipedia"})
ENGINE_HOSTS = {
    "arxiv": "export.arxiv.org",
    "github": "api.github.com",
    "openalex": "api.openalex.org",
    "wikipedia": "en.wikipedia.org",
}
ENGINE_PATHS = {
    "arxiv": "/api/query",
    "github": "/search/repositories",
    "openalex": "/works",
    "wikipedia": "/w/api.php",
}
MAX_PHYSICAL_REQUESTS_PER_ENGINE = {"arxiv": 2, "github": 1, "openalex": 1, "wikipedia": 2}
MAX_PHYSICAL_REQUESTS_PER_STAGE = 53
MAX_RESPONSE_BYTES = 2_000_000
MAX_REQUEST_MATERIAL_BYTES = core.MAX_MATERIAL_BYTES
# Aggregate ceiling derives from the unchanged per-response, request-material,
# and 53-physical-request ceilings; it is not a smaller stage-wide cap.
MAX_STAGE_TRANSFER_BYTES = MAX_PHYSICAL_REQUESTS_PER_STAGE * (MAX_RESPONSE_BYTES + MAX_REQUEST_MATERIAL_BYTES)
ENGINE_TIMEOUT_MS = 10_000
MAX_RESULTS_PER_ENGINE = 20
QUERY_PACING_SECONDS = 7.0
REFERENCE_CATEGORY = "reference"

_CURRENT_OPERATION: contextvars.ContextVar[str | None] = contextvars.ContextVar(
    "coverage_acquire_operation", default=None
)
_CURRENT_ENGINES: contextvars.ContextVar[tuple[str, ...]] = contextvars.ContextVar(
    "coverage_acquire_engines", default=()
)
_LIVE_ACQUISITION_TOKEN = object()


class OfflineClock(Protocol):
    """Injected clock; the production pacing interval stays fixed at seven seconds."""

    async def sleep(self, seconds: float) -> None: ...


@dataclass(frozen=True)
class MockResponseFixture:
    """A static response fixture matched to an actual adapter request."""

    fixture_id: str
    host: str
    path: str
    body: bytes
    status_code: int = 200
    method: str = "GET"
    headers: tuple[tuple[str, str], ...] = (("content-type", "application/json"),)
    required_query: tuple[tuple[str, str], ...] = ()
    transport_error: str | None = None  # supported synthetic errors: timeout, connect_error
    max_uses: int | None = None


@dataclass(frozen=True)
class PhysicalExchange:
    operation_id: str | None
    engine: str | None
    method: str
    url: str
    request_sha256: str
    request_bytes: int
    response_status: int | None
    response_sha256: str | None
    response_bytes: int
    fixture_id: str | None
    timeout_s: float | None
    accepted: bool
    failure_code: str | None = None
    redirect_allowed: bool | None = None


class RecordedMockTransport(httpx.MockTransport):
    """A hermetic HTTPX transport backed only by static injected fixtures."""

    def __init__(self, fixtures: Sequence[MockResponseFixture]) -> None:
        self.fixtures = tuple(fixtures)
        self.exchanges: list[PhysicalExchange] = []
        self._fixture_uses: Counter[str] = Counter()
        self._transfer_bytes = 0
        self._engine_uses: Counter[tuple[str | None, str]] = Counter()
        super().__init__(self._handle_fixture)

    @property
    def transfer_bytes(self) -> int:
        return self._transfer_bytes

    def _handle_fixture(self, request: httpx.Request) -> httpx.Response:
        method = request.method.upper()
        url = str(request.url)
        host = (request.url.host or "").lower()
        engine = next((name for name, allowed_host in ENGINE_HOSTS.items() if host == allowed_host), None)
        operation_id = _CURRENT_OPERATION.get()
        expected_engines = _CURRENT_ENGINES.get()
        request_material = _request_material(request)
        request_digest = hashlib.sha256(request_material).hexdigest()
        timeout_s = _timeout_value(request)
        failure_code: str | None = None
        accepted = False

        if engine is None or engine not in expected_engines:
            failure_code = "unexpected_engine_or_host"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline fixture rejected an unplanned host", request=request)
        if request.url.scheme != "https" or request.url.path != ENGINE_PATHS[engine]:
            failure_code = "unexpected_scheme_or_endpoint"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline fixture rejected an unplanned endpoint", request=request)
        if method != "GET":
            failure_code = "unexpected_method"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline fixture rejected a non-GET request", request=request)
        if timeout_s is None or timeout_s > ENGINE_TIMEOUT_MS / 1000:
            failure_code = "request_timeout_exceeded"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline request timeout exceeds the fixed 10-second cap", request=request)
        if any(name.lower() in {b"authorization", b"x-api-key", b"api-key"} for name, _ in request.headers.raw):
            failure_code = "credential_header_forbidden"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline acquisition forbids credentials", request=request)

        per_engine_count = self._engine_uses[(operation_id, engine)]
        if per_engine_count >= MAX_PHYSICAL_REQUESTS_PER_ENGINE[engine]:
            failure_code = "per_engine_request_cap"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline per-engine request cap exceeded", request=request)
        if len(self.exchanges) >= MAX_PHYSICAL_REQUESTS_PER_STAGE:
            failure_code = "stage_request_cap"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("offline stage request cap exceeded", request=request)

        fixture = self._find_fixture(request)
        if fixture is None:
            failure_code = "fixture_missing"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    None,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise httpx.ConnectError("no matching offline response fixture", request=request)
        self._engine_uses[(operation_id, engine)] += 1
        self._fixture_uses[fixture.fixture_id] += 1

        if fixture.transport_error:
            if fixture.transport_error == "timeout":
                exc: httpx.TransportError = httpx.ReadTimeout("synthetic offline timeout", request=request)
            elif fixture.transport_error == "connect_error":
                exc = httpx.ConnectError("synthetic offline connect error", request=request)
            else:
                raise ValueError("unsupported mock transport error")
            failure_code = f"mock_{fixture.transport_error}"
            self._append_exchange(
                PhysicalExchange(
                    operation_id,
                    engine,
                    method,
                    url,
                    request_digest,
                    len(request_material),
                    None,
                    None,
                    0,
                    fixture.fixture_id,
                    timeout_s,
                    False,
                    failure_code,
                )
            )
            raise exc

        response = httpx.Response(
            fixture.status_code,
            headers=dict(fixture.headers),
            content=fixture.body,
            request=request,
        )
        response_bytes = response.content
        response_digest = hashlib.sha256(response_bytes).hexdigest()
        redirect_allowed: bool | None = None
        if 300 <= response.status_code < 400:
            redirect_allowed = _safe_redirect(request, response.headers.get("location")) if engine == "arxiv" else False
            if not redirect_allowed:
                failure_code = "redirect_not_allowed"
        exchange_bytes = len(request_material) + len(response_bytes)
        if len(request_material) > MAX_REQUEST_MATERIAL_BYTES:
            failure_code = "request_material_cap"
        elif len(response_bytes) > MAX_RESPONSE_BYTES:
            failure_code = "response_body_cap"
        elif self._transfer_bytes + exchange_bytes > MAX_STAGE_TRANSFER_BYTES:
            failure_code = "stage_transfer_byte_cap"
        else:
            accepted = True
            self._transfer_bytes += exchange_bytes

        self._append_exchange(
            PhysicalExchange(
                operation_id,
                engine,
                method,
                url,
                request_digest,
                len(request_material),
                response.status_code,
                response_digest,
                len(response_bytes),
                fixture.fixture_id,
                timeout_s,
                accepted,
                failure_code,
                redirect_allowed,
            )
        )
        if failure_code == "stage_transfer_byte_cap":
            # Do not deliver an over-budget fixture body to the production parser.
            return httpx.Response(413, content=b"", request=request)
        return response

    def _find_fixture(self, request: httpx.Request) -> MockResponseFixture | None:
        query = dict(parse_qsl(request.url.query.decode("ascii", errors="replace"), keep_blank_values=True))
        for fixture in self.fixtures:
            if fixture.method.upper() != request.method.upper():
                continue
            if fixture.host.lower() != (request.url.host or "").lower() or fixture.path != request.url.path:
                continue
            if not all(query.get(key) == value for key, value in fixture.required_query):
                continue
            if fixture.max_uses is not None and self._fixture_uses[fixture.fixture_id] >= fixture.max_uses:
                continue
            return fixture
        return None

    def _append_exchange(self, exchange: PhysicalExchange) -> None:
        self.exchanges.append(exchange)


@dataclass
class AcquisitionOperation:
    operation_id: str
    kind: str
    query: str
    engines: tuple[str, ...]
    status: str
    pool_count: int | None
    band: str | None
    band_valid: bool | None
    target_url: str | None
    target_found_at_rank1: bool | None
    scope: object | None
    canonical_response: SearchResponse | None
    exchanges: tuple[PhysicalExchange, ...]
    failure_reasons: list[str] = field(default_factory=list)
    exception_type: str | None = None


@dataclass
class StageAcquisition:
    stage: str
    status: str
    operations: list[AcquisitionOperation]
    exchanges: list[PhysicalExchange]
    physical_request_count: int
    transfer_bytes: int
    reserved_worst_case_requests: int
    failure_reasons: list[str] = field(default_factory=list)


async def acquire_coverage_stage(
    stage_manifest: Mapping[str, object],
    adapters: Mapping[str, EngineAdapter],
    transport: RecordedMockTransport,
    clock: OfflineClock,
) -> StageAcquisition:
    """Acquire one manifest stage through SearchService and injected MockTransport only.

    This function has no default clock, transport, adapter builder, file reader,
    command entry point, network fallback, credential lookup, or result rescue.
    The caller supplies exact mock fixtures, already-configured real adapters,
    and an offline clock implementation.
    """
    if type(transport) is not RecordedMockTransport:
        raise TypeError("a caller-injected RecordedMockTransport is required")
    return await _acquire_coverage_stage(stage_manifest, adapters, transport, clock)


async def _acquire_coverage_stage(
    stage_manifest: Mapping[str, object],
    adapters: Mapping[str, EngineAdapter],
    transport: httpx.AsyncBaseTransport,
    clock: OfflineClock,
    *,
    _live_token: object | None = None,
    on_operation_complete: Callable[[AcquisitionOperation], None] | None = None,
) -> StageAcquisition:
    stage_name = stage_manifest.get("stage")
    if not isinstance(stage_name, str) or not stage_name:
        raise ValueError("stage manifest needs a stage name")
    if _live_token is None:
        if type(transport) is not RecordedMockTransport:
            raise TypeError("a caller-injected RecordedMockTransport is required")
    elif (
        _live_token is not _LIVE_ACQUISITION_TOKEN or getattr(transport, "_coverage_live_transport", False) is not True
    ):
        raise TypeError("authorized coverage live transport required")
    if on_operation_complete is not None and _live_token is not _LIVE_ACQUISITION_TOKEN:
        raise TypeError("operation snapshots require authorized coverage live transport")
    if transport.exchanges or transport.transfer_bytes:
        raise ValueError("each stage requires a fresh mock transport and empty receipt ledger")
    _validate_adapters(adapters, transport)

    research = stage_manifest.get("research_cases")
    navigation = stage_manifest.get("navigation_targets")
    if not isinstance(research, list) or not isinstance(navigation, list):
        raise ValueError("stage manifest needs research_cases and navigation_targets lists")

    operations: list[tuple[str, str, str, tuple[str, ...], str | None, str | None]] = []
    for case in research:
        if not isinstance(case, Mapping):
            raise ValueError("research cases must be objects")
        operation_id = _required_string(case, "task_id")
        query = _required_string(case, "search_query")
        plan = case.get("pool_plan")
        if not isinstance(plan, Mapping):
            raise ValueError(f"{operation_id}: pool_plan is required")
        engines = _validate_plan(plan, navigation=False)
        band = plan.get("band")
        if band not in {"research_le_40", "research_41_80"}:
            raise ValueError(f"{operation_id}: unsupported pool band")
        operations.append((operation_id, "research", query, engines, str(band), None))
    for target in navigation:
        if not isinstance(target, Mapping):
            raise ValueError("navigation targets must be objects")
        operation_id = _required_string(target, "target_id")
        query = _required_string(target, "search_query")
        navigation_url = _required_string(target, "target_url")
        plan = target.get("navigation_pool_plan")
        if not isinstance(plan, Mapping):
            raise ValueError(f"{operation_id}: navigation_pool_plan is required")
        engines = _validate_plan(plan, navigation=True)
        operations.append((operation_id, "navigation", query, engines, None, navigation_url))

    reserved = sum(
        sum(MAX_PHYSICAL_REQUESTS_PER_ENGINE[name] for name in engines) for _, _, _, engines, _, _ in operations
    )
    if reserved > MAX_PHYSICAL_REQUESTS_PER_STAGE:
        raise ValueError("stage worst-case request reservation exceeds 53; stop before acquisition")

    service = SearchService(
        AppContext(
            active_engines=dict(adapters),
            ranking_strategy="presence",
            rerank_provider=None,
            jev_router=None,
            cache=None,
            suggestion_service=None,
            rate_limiter=None,
        )
    )
    start_exchange_index = len(transport.exchanges)
    results: list[AcquisitionOperation] = []
    stage_failures: list[str] = []

    for index, (operation_id, kind, query, engines, band, target_url) in enumerate(operations):
        if _live_token is _LIVE_ACQUISITION_TOKEN and stage_failures:
            results.append(
                AcquisitionOperation(
                    operation_id=operation_id,
                    kind=kind,
                    query=query,
                    engines=engines,
                    status="not_invoked_after_terminal_failure",
                    pool_count=None,
                    band=band,
                    band_valid=None,
                    target_url=target_url,
                    target_found_at_rank1=None,
                    scope=None,
                    canonical_response=None,
                    exchanges=(),
                    failure_reasons=["prior_terminal_acquisition_failure"],
                )
            )
            continue
        if index:
            await clock.sleep(QUERY_PACING_SECONDS)
        if _live_token is _LIVE_ACQUISITION_TOKEN:
            # The admitted producer transport owns this one-shot operation
            # ledger. Mock-only/offline acquisition stays independent of it.
            begin_operation = getattr(transport, "begin_operation", None)
            if not callable(begin_operation):
                raise TypeError("live transport must own operation invocation receipts")
            begin_operation(operation_id)
        context_token = _CURRENT_OPERATION.set(operation_id)
        engines_token = _CURRENT_ENGINES.set(engines)
        before = len(transport.exchanges)
        response: SearchResponse | None = None
        exception_type: str | None = None
        reasons: list[str] = []
        try:
            request = SearchRequest(
                query=query,
                engines=list(engines),
                categories=[REFERENCE_CATEGORY],
                max_results=None,
                freshness="prefer_fresh",
                interactive_timeout_ms=ENGINE_TIMEOUT_MS,
                generate_suggestions=False,
            )
            response = await service.search(request)
            if response.scope.selected_engines != list(engines):
                reasons.append("scope_mismatch")
            if response.ranking_explanation != "tier_then_cross_engine_presence":
                reasons.append("unexpected_ranking_path")
            if response.deadline_exceeded:
                reasons.append("search_deadline_exceeded")
            failed = [outcome.engine for outcome in response.engine_outcomes if outcome.status != "ok"]
            if failed:
                reasons.append("engine_failures:" + ",".join(sorted(failed)))
            if any(item.operation_id != operation_id for item in transport.exchanges[before:]):
                reasons.append("exchange_operation_mismatch")
        except Exception as exc:  # preserve every task failure; never retry or rescue
            exception_type = type(exc).__name__
            reasons.append("search_exception")
        finally:
            _CURRENT_ENGINES.reset(engines_token)
            _CURRENT_OPERATION.reset(context_token)

        op_exchanges = tuple(transport.exchanges[before:])
        pool_count = len(response.results) if response is not None else None
        band_valid: bool | None = None
        target_found: bool | None = None
        if kind == "research":
            assert band is not None
            if pool_count is None:
                band_valid = False
                reasons.append("pool_unavailable")
            elif band == "research_le_40":
                band_valid = 1 <= pool_count <= 40
            else:
                band_valid = 41 <= pool_count <= 80
            if not band_valid:
                reasons.append("natural_pool_band_mismatch")
            if pool_count is not None and pool_count > 80:
                reasons.append("natural_pool_overflow")
        else:
            assert target_url is not None
            target_normalized = _normalise_url(target_url)
            target_found = bool(
                response and response.results and _normalise_url(response.results[0].url) == target_normalized
            )
            if not target_found:
                reasons.append("exact_target_not_rank1")

        transport_failures = [exchange.failure_code for exchange in op_exchanges if exchange.failure_code]
        if transport_failures:
            reasons.extend(f"transport:{code}" for code in transport_failures)
        status = "complete" if not reasons else "inconclusive"
        operation_result = AcquisitionOperation(
            operation_id=operation_id,
            kind=kind,
            query=query,
            engines=engines,
            status=status,
            pool_count=pool_count,
            band=band,
            band_valid=band_valid,
            target_url=target_url,
            target_found_at_rank1=target_found,
            scope=response.scope if response else None,
            canonical_response=response,
            exchanges=op_exchanges,
            failure_reasons=reasons,
            exception_type=exception_type,
        )
        if on_operation_complete is not None:
            try:
                on_operation_complete(operation_result)
            except Exception as exc:
                operation_result.status = "pool_snapshot_write_failure"
                failure_code = type(exc).__name__
                operation_result.failure_reasons.append(f"pool_snapshot_write_failure:{failure_code}")
                operation_result.exception_type = type(exc).__name__
        if _live_token is _LIVE_ACQUISITION_TOKEN:
            finish_operation = getattr(transport, "finish_operation", None)
            if not callable(finish_operation):
                raise TypeError("live transport must own operation invocation receipts")
            finish_operation(operation_id)
        results.append(operation_result)
        if operation_result.status != "complete":
            stage_failures.append(operation_id)

    stage_exchanges = transport.exchanges[start_exchange_index:]
    if len(stage_exchanges) > MAX_PHYSICAL_REQUESTS_PER_STAGE:
        stage_failures.append("stage_physical_request_limit")
    if transport.transfer_bytes > MAX_STAGE_TRANSFER_BYTES:
        stage_failures.append("stage_transfer_byte_limit")
    stage_status = "complete" if not stage_failures else "inconclusive"
    return StageAcquisition(
        stage=stage_name,
        status=stage_status,
        operations=results,
        exchanges=list(stage_exchanges),
        # Count every adapter transport attempt, including rejected fixture
        # attempts; those still consume the real study's request budget.
        physical_request_count=len(stage_exchanges),
        transfer_bytes=transport.transfer_bytes,
        reserved_worst_case_requests=reserved,
        failure_reasons=stage_failures,
    )


def _validate_adapters(adapters: Mapping[str, EngineAdapter], transport: RecordedMockTransport) -> None:
    if not adapters or set(adapters) - ALLOWED_ENGINES:
        raise ValueError("only the four fixed public engines are allowed")
    expected_types = {
        "arxiv": "ArxivAdapter",
        "github": "GitHubAdapter",
        "openalex": "OpenAlexAdapter",
        "wikipedia": "WikipediaAdapter",
    }
    expected_base = {
        "arxiv": "https://export.arxiv.org/api/query",
        "github": "https://api.github.com",
        "openalex": "https://api.openalex.org",
        "wikipedia": "https://en.wikipedia.org/w/api.php",
    }
    for name, adapter in adapters.items():
        if type(adapter).__name__ != expected_types[name] or adapter.name != name:
            raise TypeError(f"{name}: the actual supported adapter class is required")
        if adapter.config.get("max_results") != MAX_RESULTS_PER_ENGINE:
            raise ValueError(f"{name}: max_results must be fixed at 20")
        if adapter.config.get("timeout_ms") != ENGINE_TIMEOUT_MS:
            raise ValueError(f"{name}: timeout_ms must be fixed at 10000")
        if adapter.config.get("base_url", expected_base[name]) != expected_base[name]:
            raise ValueError(f"{name}: only the fixed public HTTPS endpoint is allowed")
        if adapter._http_transport is not transport:  # noqa: SLF001 - enforce mock-only seam
            raise ValueError(f"{name}: adapter must use the supplied mock transport")
        if name == "github" and adapter.config.get("api_key"):
            raise ValueError("GitHub acquisition is keyless only")


def _validate_plan(plan: Mapping[str, object], *, navigation: bool) -> tuple[str, ...]:
    engines_value = plan.get("engines")
    categories = plan.get("searx_categories")
    engine_configs = plan.get("engine_configs")
    if not isinstance(engines_value, list) or not engines_value:
        raise ValueError("plan needs an explicit engines list")
    engines = tuple(engines_value)
    if len(set(engines)) != len(engines) or set(engines) - ALLOWED_ENGINES:
        raise ValueError("plan contains duplicate or unsupported engines")
    if categories != [REFERENCE_CATEGORY]:
        raise ValueError("all acquisition scopes must use the explicit SearXNG reference category")
    if navigation and engines != ("github",):
        raise ValueError("navigation scope must use GitHub repository search only")
    if not navigation and len(engines) not in {2, 4}:
        raise ValueError("research scopes must use two or four explicit engines")
    if not navigation:
        expected_count = {"research_le_40": 2, "research_41_80": 4}.get(str(plan.get("band")))
        if expected_count is None or len(engines) != expected_count:
            raise ValueError("research band requires exactly two engines (<=40) or four engines (41-80)")
    if not isinstance(engine_configs, Mapping) or set(engine_configs) != set(engines):
        raise ValueError("engine_configs must explicitly match the engines list")
    for name in engines:
        config = engine_configs[name]
        if not isinstance(config, Mapping) or config.get("max_results") != MAX_RESULTS_PER_ENGINE:
            raise ValueError(f"{name}: plan must explicitly cap max_results at 20")
        if set(config) != {"max_results"}:
            raise ValueError(f"{name}: plan engine config may contain only max_results")
    return engines


def _required_string(value: Mapping[str, object], field_name: str) -> str:
    result = value.get(field_name)
    if not isinstance(result, str) or not result.strip():
        raise ValueError(f"{field_name} must be a non-empty string")
    return result


def _request_material(request: httpx.Request) -> bytes:
    headers = b"\n".join(name + b":" + value for name, value in request.headers.raw)
    return b"\0".join((request.method.encode("ascii"), str(request.url).encode("utf-8"), headers, request.content))


def _timeout_value(request: httpx.Request) -> float | None:
    value = request.extensions.get("timeout")
    if not isinstance(value, Mapping):
        return None
    numeric = [item for item in value.values() if isinstance(item, (int, float))]
    return max(numeric) if numeric else None


def _safe_redirect(request: httpx.Request, location: str | None) -> bool:
    if not location:
        return False
    source = urlsplit(str(request.url))
    target = urlsplit(urljoin(str(request.url), location))
    return (
        target.scheme == "https"
        and target.hostname == source.hostname
        and target.port == source.port
        and target.username is None
        and target.password is None
    )
