"""Purpose-bounded, offline HTTP replay for the frozen coverage-study engines.

This is a deterministic transport fixture for exercising real parsers and the
existing acquisition collector without contacting providers. It is not a
provider twin and its output must never receive retrieval-quality credit.
"""

from __future__ import annotations

import base64
import hashlib
import json
import re
from collections import defaultdict, deque
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from urllib.parse import parse_qsl, urlsplit

import httpx

SCHEMA = "slopsearx-coverage-engine-replay/1"
ENGINES = frozenset({"github", "arxiv", "openalex", "wikipedia"})
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_CREDENTIAL_NAME = re.compile(
    r"(?:^|[-_])(?:api[-_]?key|subscription[-_]?key|key|token|access[-_]?token|secret|auth|authorization|credential|password|passwd)(?:$|[-_])",
    re.IGNORECASE,
)
_REQUEST_BUNDLE_HEADERS = frozenset({"accept", "accept-encoding", "content-type", "user-agent"})
_RESPONSE_HEADERS = frozenset({"cache-control", "content-encoding", "content-type", "etag", "last-modified"})


class ReplayBundleError(ValueError):
    """A replay bundle is malformed, unbound, unsafe, or incomplete."""


class ReplayMismatchError(RuntimeError):
    """A real adapter requested something absent from the pinned replay."""


def canonical_json(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise ReplayBundleError("bundle-json-invalid") from exc


def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _request_key(method: str, url: str, headers: Mapping[str, str] | None = None) -> tuple:
    parsed = urlsplit(url)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password or parsed.fragment:
        raise ReplayBundleError("request-url-invalid")
    query = tuple(sorted(parse_qsl(parsed.query, keep_blank_values=True)))
    if any(_CREDENTIAL_NAME.search(key) for key, _ in query):
        raise ReplayBundleError("request-credential-query-forbidden")
    checked_headers = tuple(sorted((key.casefold(), value) for key, value in (headers or {}).items()))
    if any(_CREDENTIAL_NAME.search(key) for key, _ in checked_headers):
        raise ReplayBundleError("request-credential-header-forbidden")
    if any(key not in _REQUEST_BUNDLE_HEADERS for key, _ in checked_headers):
        raise ReplayBundleError("request-header-not-allowlisted")
    return (method.upper(), parsed.scheme, parsed.hostname.casefold(), parsed.port, parsed.path or "/", query)


def build_bundle(
    mode: str, scenarios: Sequence[Mapping[str, object]], *, capture: Mapping[str, object] | None = None
) -> bytes:
    """Build canonical versioned bytes and insert body hashes for local fixtures."""
    if mode not in {"recorded-replay", "synthetic-scenario"}:
        raise ReplayBundleError("bundle-mode-invalid")
    rows = []
    for scenario in scenarios:
        row = dict(scenario)
        outcome = row.get("outcome")
        if type(outcome) is dict and outcome.get("kind") == "http":
            encoded = outcome.get("body_base64")
            try:
                body = base64.b64decode(encoded, validate=True)
            except (TypeError, ValueError) as exc:
                raise ReplayBundleError("scenario-body-invalid") from exc
            outcome = {**outcome, "body_sha256": sha256(body)}
            row["outcome"] = outcome
        rows.append(row)
    value: dict[str, object] = {"schema": SCHEMA, "mode": mode, "scenarios": rows}
    if capture is not None:
        value["capture"] = dict(capture)
    return canonical_json(value)


def _validate_bundle(raw: bytes) -> tuple[dict, str]:
    try:
        doc = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReplayBundleError("bundle-json-invalid") from exc
    if type(doc) is not dict or canonical_json(doc) != raw:
        raise ReplayBundleError("bundle-not-canonical")
    if set(doc) not in ({"schema", "mode", "scenarios"}, {"schema", "mode", "scenarios", "capture"}):
        raise ReplayBundleError("bundle-shape-invalid")
    if doc.get("schema") != SCHEMA or doc.get("mode") not in {"recorded-replay", "synthetic-scenario"}:
        raise ReplayBundleError("bundle-version-or-mode-invalid")
    mode, scenarios = doc["mode"], doc["scenarios"]
    if type(scenarios) is not list or not scenarios:
        raise ReplayBundleError("bundle-scenarios-empty")
    if mode == "recorded-replay":
        capture = doc.get("capture")
        if type(capture) is not dict or set(capture) != {"capture_id", "captured_at", "source_digest"}:
            raise ReplayBundleError("recorded-capture-provenance-required")
        if (
            type(capture["capture_id"]) is not str
            or not capture["capture_id"]
            or type(capture["source_digest"]) is not str
            or not _SHA256.fullmatch(capture["source_digest"])
        ):
            raise ReplayBundleError("recorded-capture-provenance-invalid")
        try:
            datetime.fromisoformat(capture["captured_at"].replace("Z", "+00:00"))
        except (TypeError, ValueError, AttributeError) as exc:
            raise ReplayBundleError("recorded-capture-time-invalid") from exc
    elif "capture" in doc:
        raise ReplayBundleError("synthetic-capture-provenance-forbidden")
    ids: set[str] = set()
    for row in scenarios:
        if type(row) is not dict or set(row) != {"scenario_id", "engine", "request", "outcome"}:
            raise ReplayBundleError("scenario-shape-invalid")
        scenario_id, engine, request, outcome = row["scenario_id"], row["engine"], row["request"], row["outcome"]
        if type(scenario_id) is not str or not scenario_id or scenario_id in ids:
            raise ReplayBundleError("scenario-id-invalid")
        ids.add(scenario_id)
        if engine not in ENGINES or type(request) is not dict or set(request) - {"method", "url", "headers"}:
            raise ReplayBundleError("scenario-engine-or-request-invalid")
        if type(request.get("method")) is not str or type(request.get("url")) is not str:
            raise ReplayBundleError("scenario-request-invalid")
        headers = request.get("headers", {})
        if type(headers) is not dict or any(type(k) is not str or type(v) is not str for k, v in headers.items()):
            raise ReplayBundleError("scenario-request-headers-invalid")
        _request_key(request["method"], request["url"], headers)
        if type(outcome) is not dict or outcome.get("kind") not in {
            "http",
            "timeout",
            "connection_error",
            "incomplete",
        }:
            raise ReplayBundleError("scenario-outcome-invalid")
        if outcome["kind"] == "http":
            if set(outcome) != {"kind", "status_code", "headers", "body_base64", "body_sha256"}:
                raise ReplayBundleError("scenario-response-shape-invalid")
            if type(outcome["status_code"]) is not int or not 100 <= outcome["status_code"] <= 599:
                raise ReplayBundleError("scenario-status-invalid")
            out_headers = outcome["headers"]
            if type(out_headers) is not dict or any(
                type(k) is not str
                or type(v) is not str
                or _CREDENTIAL_NAME.search(k)
                or k.casefold() not in _RESPONSE_HEADERS
                for k, v in out_headers.items()
            ):
                raise ReplayBundleError("scenario-response-headers-invalid")
            try:
                body = base64.b64decode(outcome["body_base64"], validate=True)
            except (TypeError, ValueError) as exc:
                raise ReplayBundleError("scenario-body-invalid") from exc
            if outcome["body_sha256"] != sha256(body):
                raise ReplayBundleError("scenario-body-hash-mismatch")
        elif set(outcome) != {"kind"}:
            raise ReplayBundleError("scenario-fault-shape-invalid")
    return doc, sha256(raw)


@dataclass(frozen=True)
class ReplayReceipt:
    bundle_sha256: str
    mode: str
    consumed_scenario_ids: tuple[str, ...]
    unused_scenario_ids: tuple[str, ...]
    quality_credit: bool = False


class EngineReplay:
    """One-shot strict `MockTransport`; unmatched requests never reach a network."""

    def __init__(self, raw: bytes, expected_sha256: str):
        if type(raw) is not bytes or type(expected_sha256) is not str or not _SHA256.fullmatch(expected_sha256):
            raise ReplayBundleError("bundle-pin-invalid")
        doc, actual = _validate_bundle(raw)
        if actual != expected_sha256:
            raise ReplayBundleError("bundle-hash-mismatch")
        self.bundle_sha256 = actual
        self.mode = doc["mode"]
        self._scenario_ids: list[str] = []
        self._pending: dict[tuple, deque[dict]] = defaultdict(deque)
        for row in doc["scenarios"]:
            request = row["request"]
            key = _request_key(request["method"], request["url"], request.get("headers", {}))
            self._pending[key].append(row)
            self._scenario_ids.append(row["scenario_id"])
        self._consumed: list[str] = []
        self.transport = httpx.MockTransport(self._handle)

    @classmethod
    def from_file(cls, path: str | Path, expected_sha256: str) -> "EngineReplay":
        return cls(Path(path).read_bytes(), expected_sha256)

    async def _handle(self, request: httpx.Request) -> httpx.Response:
        headers = {key.decode().casefold(): value.decode() for key, value in request.headers.raw}
        if any(_CREDENTIAL_NAME.search(key) for key in headers):
            raise ReplayMismatchError("credentials-forbidden-in-replay")
        try:
            key = _request_key(request.method, str(request.url))
        except ReplayBundleError as exc:
            raise ReplayMismatchError("request-not-replayable") from exc
        queue = self._pending.get(key)
        if not queue:
            raise ReplayMismatchError("unmatched-request-denied")
        row = queue[0]
        expected_headers = {key.casefold(): value for key, value in row["request"].get("headers", {}).items()}
        if any(headers.get(key) != value for key, value in expected_headers.items()):
            raise ReplayMismatchError("request-header-mismatch")
        queue.popleft()
        self._consumed.append(row["scenario_id"])
        outcome = row["outcome"]
        if outcome["kind"] == "timeout":
            raise httpx.ReadTimeout("offline replay timeout", request=request)
        if outcome["kind"] == "connection_error":
            raise httpx.ConnectError("offline replay connection error", request=request)
        if outcome["kind"] == "incomplete":
            raise httpx.RemoteProtocolError("offline replay incomplete response", request=request)
        body = base64.b64decode(outcome["body_base64"], validate=True)
        return httpx.Response(outcome["status_code"], headers=outcome["headers"], content=body, request=request)

    def receipt(self) -> ReplayReceipt:
        consumed = set(self._consumed)
        return ReplayReceipt(
            bundle_sha256=self.bundle_sha256,
            mode=self.mode,
            consumed_scenario_ids=tuple(self._consumed),
            unused_scenario_ids=tuple(item for item in self._scenario_ids if item not in consumed),
            quality_credit=False,
        )

    async def aclose(self) -> None:
        await self.transport.aclose()
