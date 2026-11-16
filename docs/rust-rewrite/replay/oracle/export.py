#!/usr/bin/env python3
"""Re-export deterministic F04 observations from pinned Python source offline."""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import importlib.util
import ipaddress
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
from dataclasses import asdict
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

import httpx
from fastapi.testclient import TestClient

ROOT = Path(__file__).resolve().parents[4]
SOURCE_ROOT = ROOT
BASELINE = "7b2d876307189d7a62000d1f289b3ef4b65eaff6"
ORACLE_COMMAND = (
    ".venv/bin/python docs/rust-rewrite/replay/oracle/export.py "
    "--source-root /private/tmp/slopsearx-f04-oracle-src "
    f"--baseline {BASELINE}"
)
SOURCE_PATHS = [
    "tests/fixtures/upstream_contracts/npm.json",
    "tests/fixtures/upstream_contracts/crates.json",
    "tests/fixtures/upstream_contracts/openalex.json",
    "tests/test_upstream_contracts.py",
    "scripts/upstream_contracts.py",
    "engines/npm.py",
    "engines/crates.py",
    "engines/openalex.py",
    "slopsearx/adapter.py",
    "slopsearx/payload.py",
    "tests/fixtures/searxng_api_contract.json",
    "tests/test_searxng_api_contract.py",
    "tests/test_mcp_harness.py",
    "slopsearx/mcp/harness.py",
    "slopsearx/server.py",
    "slopsearx/mcp/tools.py",
    "slopsearx/service.py",
    "slopsearx/snapshot.py",
    "slopsearx/config.py",
]
ORIGIN = {
    "npm": "https://registry.npmjs.org",
    "crates": "https://crates.io",
    "openalex": "https://api.openalex.org",
}
SAFE_ENV_KEYS = {"PATH", "LANG", "LC_ALL", "TMPDIR", "SYSTEMROOT"}
_private_home: tempfile.TemporaryDirectory[str] | None = None
_allowed_loopback_ports: set[int] = set()


def sanitize_environment(*, candidate: bool) -> None:
    """Keep only process essentials and explicit replay inputs."""
    global _private_home
    safe = {key: value for key, value in os.environ.items() if key in SAFE_ENV_KEYS}
    if candidate:
        # replay-run mounts its candidate root read-only; the Python adapter
        # helper needs no home directory or XDG cache/config path.
        safe["HOME"] = os.devnull
        replay_keys = {
            "SSX_REPLAY_URL",
            "SSX_REPLAY_ORIGIN",
            "SSX_REPLAY_INPUT",
            "SSX_FIXED_UNIX_SECONDS",
            "SSX_FIXED_IDS",
        }
        safe.update({key: value for key, value in os.environ.items() if key in replay_keys})
    else:
        _private_home = tempfile.TemporaryDirectory(prefix="ssx-f04-oracle-home-")
        safe["HOME"] = _private_home.name
        safe["XDG_CONFIG_HOME"] = str(Path(_private_home.name) / ".config")
        safe["XDG_CACHE_HOME"] = str(Path(_private_home.name) / ".cache")
    os.environ.clear()
    os.environ.update(safe)


def verify_candidate_environment_is_read_only_safe() -> None:
    """Prove candidate environment setup never creates a temporary directory."""
    original_env = os.environ.copy()
    original_factory = tempfile.TemporaryDirectory

    def reject_tempdir(*_: Any, **__: Any) -> Any:
        raise AssertionError("candidate environment attempted temporary-directory creation")

    inputs = {
        "SSX_REPLAY_URL": "http://127.0.0.1:43210",
        "SSX_REPLAY_ORIGIN": "https://example.invalid",
        "SSX_REPLAY_INPUT": "{}",
        "SSX_FIXED_UNIX_SECONDS": "1792022400",
        "SSX_FIXED_IDS": '["fixture-one"]',
        "VALKEY_URL": "redis://unexpected.invalid/0",
        "SENTRY_DSN": "https://sentinel.invalid/1",
    }
    try:
        os.environ.clear()
        os.environ.update(inputs)
        tempfile.TemporaryDirectory = reject_tempdir
        sanitize_environment(candidate=True)
        if os.environ.get("HOME") != os.devnull:
            raise AssertionError("candidate environment has a writable-home path")
        if any(key.startswith("XDG_") for key in os.environ):
            raise AssertionError("candidate environment retained XDG config/cache paths")
        if any(os.environ.get(key) != value for key, value in inputs.items() if key.startswith("SSX_")):
            raise AssertionError("candidate environment did not preserve all replay controls")
        assert_no_shared_store_url()
    finally:
        tempfile.TemporaryDirectory = original_factory
        os.environ.clear()
        os.environ.update(original_env)


def assert_no_shared_store_url() -> None:
    if "VALKEY_URL" in os.environ or "SLOPSEARX_TEST_VALKEY_URL" in os.environ:
        raise RuntimeError("developer oracle environment retained a shared-store URL")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def git_blob(path: str) -> str:
    return subprocess.check_output(
        ["git", "rev-parse", f"{BASELINE}:{path}"], cwd=SOURCE_ROOT, text=True, env=git_environment()
    ).strip()


def git_environment() -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if key in SAFE_ENV_KEYS}
    env.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_NO_REPLACE_OBJECTS": "1",
        }
    )
    return env


def install_loopback_network_guard() -> None:
    """Permit only named fixture TCP ports; deny DNS, other sockets and Unix sockets."""
    original_getaddrinfo = socket.getaddrinfo
    original_connect = socket.socket.connect
    original_connect_ex = socket.socket.connect_ex

    def guarded_getaddrinfo(host: Any, *args: Any, **kwargs: Any) -> Any:
        flags = kwargs.get("flags", args[4] if len(args) > 4 else 0)
        if flags & getattr(socket, "AI_NUMERICHOST", 0):
            return original_getaddrinfo(host, *args, **kwargs)
        try:
            address = ipaddress.ip_address(host)
        except (ValueError, TypeError):
            raise OSError("oracle export permits loopback addresses only")
        if not address.is_loopback:
            raise OSError("oracle export permits loopback addresses only")
        return original_getaddrinfo(host, *args, **kwargs)

    def guarded_connect(sock: socket.socket, address: Any) -> Any:
        if (
            not isinstance(address, tuple)
            or len(address) < 2
            or not ipaddress.ip_address(address[0]).is_loopback
            or address[1] not in _allowed_loopback_ports
        ):
            raise OSError("oracle export permits loopback addresses only")
        return original_connect(sock, address)

    def guarded_connect_ex(sock: socket.socket, address: Any) -> Any:
        if (
            not isinstance(address, tuple)
            or len(address) < 2
            or not ipaddress.ip_address(address[0]).is_loopback
            or address[1] not in _allowed_loopback_ports
        ):
            raise OSError("oracle export permits loopback addresses only")
        return original_connect_ex(sock, address)

    def guarded_sendto(sock: socket.socket, *args: Any, **kwargs: Any) -> Any:
        raise OSError("oracle export does not permit datagram sockets")

    socket.getaddrinfo = guarded_getaddrinfo
    socket.socket.connect = guarded_connect
    socket.socket.connect_ex = guarded_connect_ex
    socket.socket.sendto = guarded_sendto


def allow_loopback_port(port: int) -> None:
    _allowed_loopback_ports.add(port)


def revoke_loopback_port(port: int) -> None:
    _allowed_loopback_ports.discard(port)


class LocalValkeySentinel:
    """Record loopback connects to a synthetic Valkey endpoint during self-checks."""

    def __init__(self) -> None:
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._listener.bind(("127.0.0.1", 0))
        self._listener.listen(4)
        self._listener.settimeout(0.1)
        self._stop = threading.Event()
        self._condition = threading.Condition()
        self._connection_count = 0
        self._baseline_count = 0
        self._thread = threading.Thread(target=self._serve, daemon=True)

    @property
    def url(self) -> str:
        return f"redis://127.0.0.1:{self._listener.getsockname()[1]}/0"

    def _serve(self) -> None:
        while not self._stop.is_set():
            try:
                connection, _ = self._listener.accept()
            except TimeoutError:
                continue
            except OSError:
                return
            with self._condition:
                self._connection_count += 1
                self._condition.notify_all()
            try:
                connection.sendall(b"\0")
            except OSError:
                pass
            connection.close()

    def _marker_connection(self) -> int:
        with self._condition:
            before = self._connection_count
        with socket.create_connection(("127.0.0.1", self._listener.getsockname()[1]), timeout=1) as marker:
            marker.settimeout(1)
            if marker.recv(1) != b"\0":
                raise RuntimeError("Valkey sentinel marker acknowledgement was invalid")
        with self._condition:
            if self._connection_count <= before:
                raise RuntimeError("Valkey sentinel did not count its marker connection")
            return self._connection_count

    def positive_control(self) -> None:
        allow_loopback_port(self._listener.getsockname()[1])
        self._baseline_count = self._marker_connection()

    def has_connections(self) -> bool:
        # TCP accepts are FIFO; this marker is accepted after handshakes from
        # both bounded children and therefore drains their queued contacts.
        return self._marker_connection() - self._baseline_count > 1

    def __enter__(self) -> LocalValkeySentinel:
        self._thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        revoke_loopback_port(self._listener.getsockname()[1])
        self._stop.set()
        self._listener.close()
        self._thread.join(timeout=1)


def adapter_case(name: str) -> dict[str, Any]:
    from engines.crates import CratesAdapter
    from engines.npm import NpmAdapter
    from engines.openalex import OpenAlexAdapter

    adapter_class = {"npm": NpmAdapter, "crates": CratesAdapter, "openalex": OpenAlexAdapter}[name]
    fixture = load_json(SOURCE_ROOT / "tests/fixtures/upstream_contracts" / f"{name}.json")
    response_body = json.dumps(fixture["payload"], separators=(",", ":"), ensure_ascii=False)
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, content=response_body, headers={"content-type": "application/json"})

    config: dict[str, Any] = {"max_results": 3, "timeout_ms": 5000}
    if name == "npm":
        config["base_url"] = "https://registry.npmjs.org/-/v1/search"
    elif name == "crates":
        config["base_url"] = "https://crates.io/api/v1/crates"
    else:
        config["base_url"] = "https://api.openalex.org"

    async def invoke() -> tuple[dict[str, Any], httpx.Request]:
        adapter = adapter_class(config=config)
        adapter.http_client = lambda **kwargs: httpx.AsyncClient(
            transport=httpx.MockTransport(handler), follow_redirects=False, trust_env=False, **kwargs
        )
        response = await adapter.search(fixture["query"])
        output = {
            "status": response.status.value,
            "body": {
                "results": [{**asdict(result), "engines": sorted(result.engines)} for result in response.results],
                "latency_ms": 0.0,  # explicitly volatile; canonicalized for byte-stable exports
                "error_message": response.error_message,
            },
        }
        return output, requests[-1]

    first, request = asyncio.run(invoke())
    second, _ = asyncio.run(invoke())
    parsed = urlsplit(str(request.url))
    target = parsed.path + (f"?{parsed.query}" if parsed.query else "")
    headers = {
        key.lower(): value
        for key, value in request.headers.items()
        if key.lower() not in {"host", "connection", "content-length"}
    }
    exchange = {
        "request": {"method": request.method, "target": target, "headers": headers, "body": ""},
        "response": {
            "status": 200,
            "headers": {"content-type": "application/json"},
            "body": response_body,
        },
    }
    return {
        "id": f"upstream-{name}-captured",
        "kind": "adapter_http",
        "input": {"engine": name, "query": fixture["query"], "params": {}, "max_results": 3},
        "upstream_origin": ORIGIN[name],
        "clock": {"unix_seconds": 1792022400, "ids": [f"fixture-{name}-001"]},
        "exchanges": [exchange],
        "expected": first,
        "candidate": second,
        "volatility": ["/body/latency_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent pinned Python adapter executions against this sanitized "
                "recorded response; not Rust parity evidence."
            ),
        },
    }


def import_contract_engine() -> type:
    path = SOURCE_ROOT / "tests/test_searxng_api_contract.py"
    spec = importlib.util.spec_from_file_location("f04_searxng_contract", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load pinned HTTP contract source")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module._ContractEngine


def search_http_observation() -> dict[str, Any]:
    import slopsearx.server as server_module
    import slopsearx.service as service
    from slopsearx.config import Config, EngineEntry, SearchConfig
    from slopsearx.mcp.harness import InMemoryStore
    from slopsearx.service import AppContext

    engine_class = import_contract_engine()
    engine = engine_class()
    server_module._active_engines = {"contract": engine}
    server_module._router = None
    server_module._health_config_cache = Config(
        engines={"contract": EngineEntry(categories=["general", "science"])},
        search=SearchConfig(formats=["json"]),
    )
    server_module._health_catalog_cache = None
    server_module._health_catalog_engines = None
    server_module._routing_budget_cache = None
    server_module.load_config = lambda: (_ for _ in ()).throw(
        RuntimeError("HTTP oracle attempted to read host configuration")
    )
    store = InMemoryStore()

    async def fixture_context() -> AppContext:
        return AppContext(
            active_engines={"contract": engine},
            cache=store,
            engine_semaphore=asyncio.Semaphore(10),
            tier1_engines={"contract"},
            sensitive_engines=set(),
        )

    # Preserve FastAPI's real lifespan and HTTP pipeline while supplying the
    # same deterministic app-context seam used by the pinned test harness.
    server_module.build_context = fixture_context
    service.generate_query_id = lambda: "ssx-f04-search-http"
    with TestClient(server_module.app) as client:
        response = client.get("/search", params={"q": "F04 contract", "format": "json", "engines": "contract"})
    body = response.json()
    body["meta"]["response_time_ms"] = 0  # explicitly volatile; canonicalized for byte-stable exports
    return {"status": response.status_code, "body": body}


def search_http_case() -> dict[str, Any]:
    expected = search_http_observation()
    candidate = search_http_observation()
    return {
        "id": "search-http-json-contract",
        "kind": "search_http",
        "input": {
            "method": "GET",
            "path": "/search",
            "params": {"q": "F04 contract", "format": "json", "engines": "contract"},
        },
        "upstream_origin": None,
        "clock": {"unix_seconds": 1792022400, "ids": ["ssx-f04-search-http"]},
        "exchanges": [],
        "expected": expected,
        "candidate": candidate,
        "volatility": ["/body/meta/response_time_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent FastAPI TestClient executions using the pinned "
                "deterministic contract engine; no upstream traffic."
            ),
        },
    }


async def _mcp_observation() -> dict[str, Any]:
    import uvicorn
    from mcp import ClientSession
    from mcp.client.streamable_http import streamable_http_client

    import slopsearx.service as service
    import slopsearx.snapshot as snapshot_module
    from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app

    service.generate_query_id = lambda: "ssx-f04-mcp-search"
    import types

    original_uuid4 = snapshot_module.uuid.uuid4
    original_time = snapshot_module.time.time
    allowed_port: int | None = None
    snapshot_module.uuid.uuid4 = lambda: types.SimpleNamespace(hex="f04fixedsnapshot")
    snapshot_module.time.time = lambda: 1792022400.0
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=2)])
    server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=0, log_level="error"))
    task = asyncio.create_task(server.serve())
    try:
        async with asyncio.timeout(10):
            while not server.started:
                if task.done():
                    await task
                    raise RuntimeError("MCP fixture server exited before startup")
                await asyncio.sleep(0.01)
        port = server.servers[0].sockets[0].getsockname()[1]
        allowed_port = port
        observed_statuses: list[int] = []

        async def record_response(response: httpx.Response) -> None:
            observed_statuses.append(response.status_code)

        allow_loopback_port(port)
        async with httpx.AsyncClient(event_hooks={"response": [record_response]}, trust_env=False) as client:
            async with streamable_http_client(f"http://127.0.0.1:{port}/mcp", http_client=client) as streams:
                async with ClientSession(streams[0], streams[1]) as session:
                    await session.initialize()
                    result = await session.call_tool(
                        "slopsearx_search",
                        {
                            "query": "F04 MCP contract",
                            "engines": ["wikipedia"],
                            "language": "de",
                            "time_range": "week",
                            "safesearch": "moderate",
                        },
                    )
                    if not observed_statuses:
                        raise RuntimeError("MCP client produced no HTTP response")
                    body = json.loads(result.content[0].text)
                    body["meta"]["response_time_ms"] = 0  # explicitly volatile; canonicalized for byte-stable exports
                    return {"status": observed_statuses[-1], "body": body}
    finally:
        snapshot_module.uuid.uuid4 = original_uuid4
        snapshot_module.time.time = original_time
        if allowed_port is not None:
            revoke_loopback_port(allowed_port)
        server.should_exit = True
        try:
            await asyncio.wait_for(task, timeout=10)
        except TimeoutError:
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass


def mcp_search_observation() -> dict[str, Any]:
    return asyncio.run(_mcp_observation())


def mcp_search_case() -> dict[str, Any]:
    expected = mcp_search_observation()
    candidate = mcp_search_observation()
    return {
        "id": "mcp-search-filter-enforcement",
        "kind": "mcp_search",
        "input": {
            "tool": "slopsearx_search",
            "query": "F04 MCP contract",
            "engines": ["wikipedia"],
            "language": "de",
            "time_range": "week",
            "safesearch": "moderate",
        },
        "upstream_origin": None,
        "clock": {"unix_seconds": 1792022400, "ids": ["ssx-f04-mcp-search"]},
        "exchanges": [],
        "expected": expected,
        "candidate": candidate,
        "volatility": ["/body/meta/response_time_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent calls over the real MCP streamable-HTTP surface with "
                "FakeEngine and InMemoryStore; no upstream traffic."
            ),
        },
    }


def run_adapter_candidate(input_value: dict[str, Any]) -> dict[str, Any]:
    from engines.crates import CratesAdapter
    from engines.npm import NpmAdapter
    from engines.openalex import OpenAlexAdapter

    name = input_value["engine"]
    adapter_class = {"npm": NpmAdapter, "crates": CratesAdapter, "openalex": OpenAlexAdapter}[name]
    origin = os.environ["SSX_REPLAY_URL"]
    path = {"npm": "/-/v1/search", "crates": "/api/v1/crates", "openalex": "/works"}[name]
    config = {"max_results": input_value["max_results"], "timeout_ms": 5000}
    config["base_url"] = origin + ("" if name == "openalex" else path)
    if name == "openalex":
        config["base_url"] = origin

    async def run() -> dict[str, Any]:
        adapter = adapter_class(config=config)
        parsed_origin = urlsplit(origin)
        if parsed_origin.hostname not in {"127.0.0.1", "::1", "localhost"} or parsed_origin.port is None:
            raise RuntimeError("candidate replay origin must be an explicit loopback socket")
        allow_loopback_port(parsed_origin.port)
        adapter.http_client = lambda **kwargs: httpx.AsyncClient(trust_env=False, **kwargs)
        try:
            response = await adapter.search(input_value["query"], input_value.get("params"))
            return {
                "status": response.status.value,
                "body": {
                    "results": [{**asdict(row), "engines": sorted(row.engines)} for row in response.results],
                    "latency_ms": response.latency_ms,
                    "error_message": response.error_message,
                },
            }
        finally:
            await adapter.shutdown()
            revoke_loopback_port(parsed_origin.port)

    return asyncio.run(run())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", action="store_true", help="emit one observation for cargo xtask replay-run")
    parser.add_argument("--case", help="case ID for --candidate mode")
    parser.add_argument(
        "--verify-ambient-format",
        action="store_true",
        help="prove hostile inherited runtime settings cannot change the exported corpus or reach Valkey",
    )
    parser.add_argument(
        "--source-root", type=Path, required=True, help="clean detached checkout at the pinned Python oracle commit"
    )
    parser.add_argument("--output", default="crates/xtask/fixtures/replay/corpus.json")
    args = parser.parse_args()
    global SOURCE_ROOT
    if args.baseline != BASELINE:
        raise SystemExit(f"baseline must be the pinned F04 source revision {BASELINE}")
    sanitize_environment(candidate=args.candidate)
    assert_no_shared_store_url()
    SOURCE_ROOT = args.source_root.resolve()
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=SOURCE_ROOT, text=True, env=git_environment()
    ).strip()
    dirty = subprocess.check_output(
        [
            "git",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "status",
            "--porcelain",
            "--untracked-files=all",
        ],
        cwd=SOURCE_ROOT,
        text=True,
        env=git_environment(),
    )
    if commit != BASELINE or dirty:
        raise SystemExit(f"oracle source root must be clean at {BASELINE}; got {commit}, dirty={bool(dirty)}")
    bytecode_roots = [SOURCE_ROOT / directory for directory in ("slopsearx", "engines", "tests")]
    stale_bytecode = [path for root in bytecode_roots if root.exists() for path in root.rglob("*.pyc")]
    if stale_bytecode:
        raise SystemExit(f"ignored bytecode could shadow pinned source: {stale_bytecode[0]}")
    sys.path.insert(0, str(SOURCE_ROOT))
    package_spec = importlib.util.find_spec("slopsearx")
    if (
        package_spec is None
        or package_spec.origin is None
        or not Path(package_spec.origin).resolve().is_relative_to(SOURCE_ROOT)
    ):
        raise SystemExit("Python package import does not resolve from the pinned source root")
    engines_spec = importlib.util.find_spec("engines")
    if (
        engines_spec is None
        or engines_spec.origin is None
        or not Path(engines_spec.origin).resolve().is_relative_to(SOURCE_ROOT)
    ):
        raise SystemExit("engine imports do not resolve from the pinned source root")
    sys.dont_write_bytecode = True
    # The HTTP surface reads this setting while the pinned server module loads.
    # Explicit JSON keeps the export independent of the developer's shell.
    os.environ["SEARCH_FORMATS"] = "json"
    install_loopback_network_guard()
    if args.candidate:
        if args.case not in {"upstream-npm-captured", "upstream-crates-captured", "upstream-openalex-captured"}:
            raise SystemExit("--candidate currently supports adapter_http cases only")
        observation = run_adapter_candidate(json.loads(os.environ["SSX_REPLAY_INPUT"]))
        print(json.dumps(observation, separators=(",", ":")))
        return
    if args.verify_ambient_format:
        verify_candidate_environment_is_read_only_safe()
        with tempfile.TemporaryDirectory(prefix="ssx-f04-env-check-") as temporary:
            clean_output = Path(temporary) / "clean.json"
            ambient_output = Path(temporary) / "ambient.json"
            command = [
                sys.executable,
                str(Path(__file__).resolve()),
                "--baseline",
                args.baseline,
                "--source-root",
                str(SOURCE_ROOT),
                "--output",
            ]
            clean_env = os.environ.copy()
            clean_env.pop("SEARCH_FORMATS", None)
            ambient_env = clean_env.copy()
            ambient_env["SEARCH_FORMATS"] = "html"
            ambient_env["SENTRY_DSN"] = "https://sentinel.invalid/1"
            ambient_env["TYPESAFE_API_KEY"] = "sentinel-no-call"
            ambient_env["SLOPSEARX_WORKFLOW_PORTAL_ENABLED"] = "true"
            ambient_env["MAX_CONCURRENT_ENGINES"] = "1"
            ambient_env["PER_CLIENT_REQUESTS"] = "1"
            ambient_env["PER_CLIENT_WINDOW_SECONDS"] = "1"
            ambient_env["FAIL_CLOSED"] = "true"
            ambient_env["FEATURE_EMPTY_SCRAPE_DIAGNOSTICS"] = "true"
            ambient_env["FEATURE_BRAVE_CATEGORY_ROUTING"] = "true"
            ambient_env["ROUTING_MAX_ENGINES_PER_INTENT"] = "1"
            ambient_env["PAYLOAD_MAX_PERSIST_BYTES"] = "1"
            ambient_env["HF_TOKEN"] = "sentinel-no-call"
            ambient_env["ENGINE_NPM_API_KEY"] = "sentinel-no-call"
            ambient_env["PYTHONPATH"] = str(SOURCE_ROOT / "untrusted-python-path")
            ambient_env["GIT_DIR"] = "/untrusted/git-dir"
            ambient_env["GIT_WORK_TREE"] = "/untrusted/work-tree"
            ambient_env["GIT_CONFIG_GLOBAL"] = "/untrusted/gitconfig"
            with LocalValkeySentinel() as valkey_sentinel:
                valkey_sentinel.positive_control()
                ambient_env["VALKEY_URL"] = valkey_sentinel.url
                ambient_env["SLOPSEARX_TEST_VALKEY_URL"] = valkey_sentinel.url
                ambient_env["HTTP_PROXY"] = f"http://127.0.0.1:{valkey_sentinel._listener.getsockname()[1]}"
                ambient_env["HTTPS_PROXY"] = ambient_env["HTTP_PROXY"]
                subprocess.run(
                    [*command, str(clean_output)],
                    cwd=ROOT,
                    env=clean_env,
                    check=True,
                    timeout=60,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                subprocess.run(
                    [*command, str(ambient_output)],
                    cwd=ROOT,
                    env=ambient_env,
                    check=True,
                    timeout=60,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                valkey_contact = valkey_sentinel.has_connections()
            if valkey_contact:
                raise SystemExit("export attempted to connect to the synthetic Valkey endpoint")
            if clean_output.read_bytes() != ambient_output.read_bytes():
                raise SystemExit("SEARCH_FORMATS=html changed the exported corpus")
        print("hostile inherited settings preserve output bytes and made no Valkey connections")
        return
    provenance = {
        "license": (
            "Corpus structure and generated observations are MIT-licensed with this repository; "
            "embedded sanitized upstream API facts retain their source terms. This corpus does "
            "not assert a separate license grant for those facts."
        ),
        "redaction": (
            "Only existing allowlisted public parser fields are retained; no request/response "
            "identity headers, credentials, author/publisher identities, raw errors, or "
            "unsanitized provider payloads."
        ),
        "sources": [{"path": path, "git_blob": git_blob(path)} for path in SOURCE_PATHS],
        "oracle": {
            "path": "docs/rust-rewrite/replay/oracle/export.py",
            "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "invocation": ORACLE_COMMAND,
        },
    }
    corpus = {
        "schema_version": 1,
        "baseline_commit": BASELINE,
        "provenance": provenance,
        "cases": [
            *(adapter_case(name) for name in ("npm", "crates", "openalex")),
            search_http_case(),
            mcp_search_case(),
        ],
    }
    output = ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(corpus, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    display_output = output.resolve()
    if display_output.is_relative_to(ROOT):
        display_output = display_output.relative_to(ROOT)
    print(f"wrote {len(corpus['cases'])} cases to {display_output} from Python source at {commit}")


if __name__ == "__main__":
    main()
