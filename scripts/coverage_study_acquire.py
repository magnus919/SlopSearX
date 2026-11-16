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

import asyncio
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
# Matches the live transport's microsecond offset quantization margin. The live
# module hash in the stage attestation pins both implementations.
OFFSET_QUANTIZATION_GUARD_SECONDS = 0.000001
REFERENCE_CATEGORY = "reference"


@dataclass(frozen=True)
class AcquisitionExecutionProfile:
    """A closed, versioned acquisition policy selected by a pinned manifest."""

    identifier: str
    manifest_schema: str
    request_timeout_seconds: float
    logical_query_timeout_seconds: float
    continue_after_ordinary_failure: bool


LEGACY_ACQUISITION_PROFILE = AcquisitionExecutionProfile(
    identifier="coverage-acquisition-profile/1",
    manifest_schema="coverage-live-acquisition-manifest/1",
    request_timeout_seconds=10.0,
    logical_query_timeout_seconds=10.0,
    continue_after_ordinary_failure=False,
)
CONTINUE_ACQUISITION_PROFILE = AcquisitionExecutionProfile(
    identifier="coverage-acquisition-profile/2",
    manifest_schema="coverage-live-acquisition-manifest/2",
    request_timeout_seconds=30.0,
    logical_query_timeout_seconds=30.0,
    continue_after_ordinary_failure=True,
)
_ACQUISITION_PROFILES = {
    LEGACY_ACQUISITION_PROFILE.identifier: LEGACY_ACQUISITION_PROFILE,
    CONTINUE_ACQUISITION_PROFILE.identifier: CONTINUE_ACQUISITION_PROFILE,
}


def pool_input_mode_for_stage(protocol: Mapping[str, object], stage_kind: str | None = None) -> str:
    """Select the registered input method without substituting confirmation data."""
    modes = protocol.get("pool_input_mode_by_stage")
    if modes is None:
        return str(protocol.get("pool_input_mode", "live-acquisition/1"))
    if modes == {"development": "frozen-pool/1", "confirmation": "frozen-pool/1"}:
        if (
            protocol.get("schema") != "coverage-first-study-protocol/4"
            or protocol.get("pool_input_mode") != "frozen-pool/1"
        ):
            raise ValueError("unsupported stage input methods")
        if stage_kind not in {"development", "confirmation"}:
            raise ValueError("stage kind required for fresh inputs")
        return modes[stage_kind]
    if modes != {"development": "saved-corpus/1", "confirmation": "frozen-pool/1"}:
        raise ValueError("unsupported stage input methods")
    if stage_kind not in {"development", "confirmation"}:
        raise ValueError("stage kind required for saved development inputs")
    inputs = protocol.get("inputs")
    if type(inputs) is not dict or inputs.get("source_artifact_reuse") != "development-only":
        raise ValueError("saved development reuse must be explicit")
    return modes[stage_kind]


def execution_profile_from_protocol(
    acquisition: object, *, pool_input_mode: str = "live-acquisition/1"
) -> AcquisitionExecutionProfile:
    """Resolve only registered legacy or exact v2 acquisition descriptors."""
    if type(acquisition) is not dict:
        raise ValueError("acquisition policy must be an object")
    profile_id = acquisition.get("execution_profile")
    if profile_id is None:
        if acquisition.get("timeout_seconds") != LEGACY_ACQUISITION_PROFILE.request_timeout_seconds:
            raise ValueError("legacy acquisition timeout is fixed at ten seconds")
        return LEGACY_ACQUISITION_PROFILE
    profile = _ACQUISITION_PROFILES.get(profile_id)
    if profile is not CONTINUE_ACQUISITION_PROFILE:
        raise ValueError("unsupported acquisition execution profile")
    expected = {
        "execution_profile": profile.identifier,
        "timeout_seconds": profile.request_timeout_seconds,
        "logical_query_timeout_seconds": profile.logical_query_timeout_seconds,
        "ordinary_failure_policy": "continue-planned-operations",
        "pool_failure": (
            "retain empty, failed, wrong-band and overflow outcomes; "
            "stage inconclusive, no replacement/concatenation/trimming"
        ),
        "retries": 0,
        "max_physical_http_calls": MAX_PHYSICAL_REQUESTS_PER_STAGE,
        "per_engine_results": MAX_RESULTS_PER_ENGINE,
        "response_bytes": 2_000_000,
        "pacing_seconds": QUERY_PACING_SECONDS,
        "arxiv_physical_pacing_seconds": 3,
        "brave_calls": 0,
        "brave_remaining_reserved": 2,
        "engines": ["arxiv", "openalex", "github", "wikipedia"],
        "categories": [REFERENCE_CATEGORY],
        "adapter_calls_maximum": MAX_PHYSICAL_REQUESTS_PER_ENGINE,
        "credentials": "keyless public acquisition; no provider key loaded before admitted model phase",
    }
    if pool_input_mode in {"frozen-pool/1", "saved-corpus/1"}:
        expected.update(
            {
                "pool_failure": (
                    "Retain every actual provider and acquisition outcome. Admit only a complete provenance-verified "
                    "natural corpus with all declared slots, registered bands and exact navigation targets. Provider "
                    "availability is separate; no replacements, concatenation, trimming, padding, engine substitution "
                    "or retry."
                ),
                "rate_limit_policy": (
                    "An actual dispatched HTTP 429 installs a stage-local cooldown for that engine; later suppression "
                    "is reported as a rate-limited engine outcome with zero physical dispatch and a reference to the "
                    "original denial, never as another HTTP response."
                ),
                "finalization_reserve_seconds": 300,
            }
        )
    elif pool_input_mode != "live-acquisition/1":
        raise ValueError("unsupported pool input mode")
    if set(acquisition) != set(expected) or any(acquisition.get(key) != value for key, value in expected.items()):
        raise ValueError("versioned acquisition policy values do not match the fixed profile")
    return profile


def execution_profile_from_manifest(manifest: Mapping[str, object]) -> AcquisitionExecutionProfile:
    """Select an acquisition profile from a strict manifest schema and closed profile id."""
    if type(manifest) is not dict:
        raise ValueError("acquisition manifest must be an object")
    schema = manifest.get("schema")
    if schema is None and "execution_profile" not in manifest:
        # The mock-only public helper historically accepted small schema-less
        # fixtures. Production admission still requires the live manifest schema.
        return LEGACY_ACQUISITION_PROFILE
    if schema == LEGACY_ACQUISITION_PROFILE.manifest_schema:
        if "execution_profile" in manifest:
            raise ValueError("legacy acquisition manifest cannot select a profile")
        return LEGACY_ACQUISITION_PROFILE
    if (
        schema == CONTINUE_ACQUISITION_PROFILE.manifest_schema
        and manifest.get("execution_profile") == CONTINUE_ACQUISITION_PROFILE.identifier
    ):
        return CONTINUE_ACQUISITION_PROFILE
    raise ValueError("unsupported or unbound acquisition manifest profile")


_CURRENT_OPERATION: contextvars.ContextVar[str | None] = contextvars.ContextVar(
    "coverage_acquire_operation", default=None
)
_CURRENT_ENGINES: contextvars.ContextVar[tuple[str, ...]] = contextvars.ContextVar(
    "coverage_acquire_engines", default=()
)
_CURRENT_LOGICAL_TIMEOUT: contextvars.ContextVar[object | None] = contextvars.ContextVar(
    "coverage_acquire_logical_timeout", default=None
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

    def __init__(
        self,
        fixtures: Sequence[MockResponseFixture],
        *,
        request_timeout_seconds: float = LEGACY_ACQUISITION_PROFILE.request_timeout_seconds,
    ) -> None:
        if request_timeout_seconds not in {
            LEGACY_ACQUISITION_PROFILE.request_timeout_seconds,
            CONTINUE_ACQUISITION_PROFILE.request_timeout_seconds,
        }:
            raise ValueError("mock acquisition transport accepts only a closed execution-profile timeout")
        self.request_timeout_seconds = request_timeout_seconds
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
        if timeout_s is None or timeout_s > self.request_timeout_seconds:
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
            raise httpx.ConnectError("offline request timeout exceeds the fixed acquisition profile", request=request)
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
    profile = execution_profile_from_manifest(stage_manifest)
    if transport.request_timeout_seconds != profile.request_timeout_seconds:
        raise ValueError("mock transport timeout must match the pinned acquisition profile")
    return await _acquire_coverage_stage(stage_manifest, adapters, transport, clock, _execution_profile=profile)


async def _acquire_coverage_stage(
    stage_manifest: Mapping[str, object],
    adapters: Mapping[str, EngineAdapter],
    transport: httpx.AsyncBaseTransport,
    clock: OfflineClock,
    *,
    _live_token: object | None = None,
    _execution_profile: AcquisitionExecutionProfile | None = None,
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
    profile = _execution_profile or execution_profile_from_manifest(stage_manifest)
    if profile != execution_profile_from_manifest(stage_manifest):
        raise ValueError("acquisition execution profile does not match its pinned manifest")
    if on_operation_complete is not None and _live_token is not _LIVE_ACQUISITION_TOKEN:
        raise TypeError("operation snapshots require authorized coverage live transport")
    if transport.exchanges or transport.transfer_bytes:
        raise ValueError("each stage requires a fresh mock transport and empty receipt ledger")
    _validate_adapters(adapters, transport, profile)

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
    terminal_failure_seen = False

    for index, (operation_id, kind, query, engines, band, target_url) in enumerate(operations):
        if _live_token is _LIVE_ACQUISITION_TOKEN and terminal_failure_seen:
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
            await clock.sleep(QUERY_PACING_SECONDS + OFFSET_QUANTIZATION_GUARD_SECONDS)
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
                interactive_timeout_ms=int(profile.logical_query_timeout_seconds * 1000),
                generate_suggestions=False,
            )
            if profile is CONTINUE_ACQUISITION_PROFILE:
                timeout_context = asyncio.timeout(profile.logical_query_timeout_seconds)
                timeout_token = _CURRENT_LOGICAL_TIMEOUT.set(timeout_context)
                try:
                    async with timeout_context:
                        response = await service.search(request)
                finally:
                    _CURRENT_LOGICAL_TIMEOUT.reset(timeout_token)
            else:
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
            if profile is CONTINUE_ACQUISITION_PROFILE and isinstance(exc, TimeoutError):
                reasons.append("logical_query_timeout")
            else:
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
            if _live_token is _LIVE_ACQUISITION_TOKEN and (
                not profile.continue_after_ordinary_failure or not _may_continue_after_failure(operation_result)
            ):
                terminal_failure_seen = True

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


def _validate_adapters(
    adapters: Mapping[str, EngineAdapter], transport: RecordedMockTransport, profile: AcquisitionExecutionProfile
) -> None:
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
        expected_timeout_ms = int(profile.request_timeout_seconds * 1000)
        if adapter.config.get("timeout_ms") != expected_timeout_ms:
            raise ValueError(f"{name}: timeout_ms does not match the pinned acquisition profile")
        if adapter.config.get("base_url", expected_base[name]) != expected_base[name]:
            raise ValueError(f"{name}: only the fixed public HTTPS endpoint is allowed")
        if adapter._http_transport is not transport:  # noqa: SLF001 - enforce mock-only seam
            raise ValueError(f"{name}: adapter must use the supplied mock transport")
        if name == "github" and adapter.config.get("api_key"):
            raise ValueError("GitHub acquisition is keyless only")


_HARD_TRANSPORT_FAILURES = frozenset(
    {
        "unexpected_engine_or_operation",
        "request_material_cap",
        "unexpected_endpoint",
        "unexpected_method",
        "request_timeout_exceeded",
        "credential_header_forbidden",
        "per_engine_request_cap",
        "stage_request_cap",
        "response_body_cap",
        "stage_transfer_byte_cap",
        "redirect_not_allowed",
    }
)
_HARD_ENGINE_STATUSES = frozenset({"rate_limited", "blocked", "auth_required", "error", "unavailable"})
_ORDINARY_TRANSPORT_FAILURES = frozenset({"request_timeout", "request_deadline_before_dispatch"})


def _may_continue_after_failure(operation: AcquisitionOperation) -> bool:
    """Only ordinary timeout/empty/band/target outcomes may consume later planned slots."""
    if operation.status == "pool_snapshot_write_failure":
        return False
    if operation.exception_type is not None:
        ordinary_after_timeout = {
            "logical_query_timeout",
            "pool_unavailable",
            "natural_pool_band_mismatch",
            "exact_target_not_rank1",
        }
        return bool(
            operation.exception_type == "TimeoutError"
            and "logical_query_timeout" in operation.failure_reasons
            and all(
                reason in ordinary_after_timeout
                or reason in {f"transport:{code}" for code in _ORDINARY_TRANSPORT_FAILURES}
                for reason in operation.failure_reasons
            )
            and all(
                exchange.failure_code in {None, "timeout", *_ORDINARY_TRANSPORT_FAILURES}
                for exchange in operation.exchanges
            )
        )
    if any(
        reason
        in {
            "scope_mismatch",
            "unexpected_ranking_path",
            "exchange_operation_mismatch",
            "pool_snapshot_write_failure",
        }
        for reason in operation.failure_reasons
    ):
        return False
    if operation.canonical_response is None:
        return False
    allowed_reasons = {
        "pool_unavailable",
        "natural_pool_band_mismatch",
        "exact_target_not_rank1",
        "search_deadline_exceeded",
    }
    if any(
        reason not in allowed_reasons
        and not reason.startswith("engine_failures:")
        and reason not in {f"transport:{code}" for code in _ORDINARY_TRANSPORT_FAILURES}
        for reason in operation.failure_reasons
    ):
        return False
    outcomes = operation.canonical_response.engine_outcomes
    if "search_deadline_exceeded" in operation.failure_reasons and not any(
        outcome.status == "timeout" for outcome in outcomes
    ):
        return False
    if any(outcome.status in _HARD_ENGINE_STATUSES for outcome in outcomes):
        return False
    if any(outcome.status not in {"ok", "timeout"} for outcome in outcomes):
        return False
    if any(exchange.failure_code in _HARD_TRANSPORT_FAILURES for exchange in operation.exchanges):
        return False
    return True


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
