"""Temporary offline source oracle; no backend imports. Retire in G08."""

import ast
import asyncio
import copy
import dataclasses
import hashlib
import json
import os
import pathlib
import socket
import subprocess
import tempfile
import time as real_time
import types

ROOT = pathlib.Path(__file__).resolve().parents[4]
SOURCE = ROOT / "slopsearx/staged.py"
tree = ast.parse(SOURCE.read_text())
nodes = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in {"plan_digest", "_normalize_record"}]
ns = {"hashlib": hashlib, "json": json, "Any": object}
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SOURCE), "exec"), ns)
scope = {"requested": {"engines": ["alpha"]}, "selected_engines": ["alpha"], "excluded_engines": []}
record = {
    "operation_id": "legacy:id:雪",
    "tenant": "org:team",
    "version": 1,
    "accepted_at": 100.0,
    "execution_deadline_at": 130.0,
    "expires_at": 3700.0,
    "state": "queued",
    "stop_reason": None,
    "plan": {
        "query": '雪😀 \\"',
        "filters": {"language": "en", "time_range": None, "safesearch": "off", "freshness": "no_preference"},
        "initial_scope": scope,
        "fallback_scope": None,
        "allow_scope_expansion": False,
        "fallback_when": "clean_empty",
    },
    "objectives": {
        "requested": {"deadline_ms": 30000, "max_engine_calls": 3},
        "effective": {"deadline_ms": 30000, "max_engine_calls": 3},
        "unmet": [],
    },
    "budget": {"unit": "adapter_search_invocations", "limit": 3, "reserved": 0, "observed": 0},
    "stages": [
        {"index": 0, "scope": scope, "state": "pending", "skipped_reason": None, "attempts": [], "enforcement": {}}
    ],
    "retry_keys": [],
    "selected_result_attempt_id": None,
    "next_stage": 0,
}
identity = {
    "version": record["version"],
    **{k: record["plan"][k] for k in ("query", "initial_scope", "fallback_scope", "allow_scope_expansion", "filters")},
    "objectives": {k: record["objectives"][k] for k in ("requested", "effective")},
}
record["plan_digest"] = ns["plan_digest"](identity)
normalizations = []
for value in [{}, {"malformed": 1}]:
    candidate = copy.deepcopy(record)
    candidate["retry_keys"] = value
    normalizations.append(
        {"input": candidate, "source": ns["_normalize_record"](copy.deepcopy(candidate)), "native_accept": not value}
    )
sources = []
for path in [
    "slopsearx/staged.py",
    "slopsearx/mcp/staged_tools.py",
    "tests/test_staged_search.py",
    "tests/test_staged_valkey_integration.py",
]:
    sources.append(
        {
            "path": path,
            "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest(),
            "git_blob": subprocess.check_output(["git", "rev-parse", "HEAD:" + path], cwd=ROOT, text=True).strip(),
        }
    )
out = {
    "schema_version": 1,
    "plan_id": "W11",
    "baseline": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "sources": sources,
    "record": record,
    "canonical_identity": json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True),
    "digest": ns["plan_digest"](identity),
    "normalizations": normalizations,
    "expiry_equality": {"read_expired": 3700.0 > record["expires_at"], "retry_expired": record["expires_at"] <= 3700.0},
}
(ROOT / "crates/xtask/fixtures/w11/corpus.json").write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n")

# Supplemental actual-method controls supersede the authored expiry expressions
# retained in the original corpus. Private real Valkey; no backend imports.


def command(sock_path, *args):
    encoded = [arg if isinstance(arg, bytes) else str(arg).encode() for arg in args]
    with socket.socket(socket.AF_UNIX) as client:
        client.connect(str(sock_path))
        client.sendall(
            b"*"
            + str(len(encoded)).encode()
            + b"\r\n"
            + b"".join(b"$" + str(len(arg)).encode() + b"\r\n" + arg + b"\r\n" for arg in encoded)
        )
        stream = client.makefile("rb")

        def read():
            line = stream.readline()
            if not line:
                raise RuntimeError("private Valkey closed")
            kind, value = line[:1], line[1:-2]
            if kind == b"+":
                return value
            if kind == b"-":
                raise RuntimeError(value.decode())
            if kind == b":":
                return int(value)
            if kind == b"$":
                length = int(value)
                if length == -1:
                    return None
                data = stream.read(length)
                tail = stream.read(2)
                if len(data) != length or tail != b"\r\n":
                    raise RuntimeError("malformed private Valkey bulk response")
                return data
            if kind == b"*":
                return [read() for _ in range(int(value))]
            raise RuntimeError("unknown RESP")

        return read()


async def controls(sock_path):
    clock = [3700.0]
    control_ns = {
        **ns,
        "asyncio": asyncio,
        "dataclass": dataclasses.dataclass,
        "time": types.SimpleNamespace(time=lambda: clock[0]),
        "__name__": "builtins",
    }
    selected = [
        n
        for n in tree.body
        if (isinstance(n, ast.ClassDef) and n.name in {"StoreRead", "StagedSearchStore"})
        or (
            isinstance(n, ast.Assign)
            and any(
                isinstance(t, ast.Name)
                and t.id
                in {
                    "PREFIX",
                    "RETENTION_SECONDS",
                    "STORE_MARGIN_SECONDS",
                    "MAX_OPERATIONS_PER_TENANT",
                    "LEASE_SECONDS",
                    "_RETRY_SCRIPT",
                }
                for t in n.targets
            )
        )
    ]
    exec(compile(ast.Module(body=selected, type_ignores=[]), str(SOURCE), "exec"), control_ns)

    class Client:
        async def eval(self, script, num_keys, *args):
            return command(sock_path, "EVAL", script, num_keys, *args)

    class Store:
        is_connected = True
        _client = Client()

        async def get(self, key):
            raw = command(sock_path, "GET", key)
            return json.loads(raw) if raw else None

    store = control_ns["StagedSearchStore"](Store())
    key = store._key(record["tenant"], record["operation_id"])
    command(sock_path, "SET", key, json.dumps(record))
    reads = []
    for when in [3699.0, 3700.0, 3700.001]:
        clock[0] = when
        result = await store.read(record["tenant"], record["operation_id"])
        reads.append(
            {
                "now": when,
                "record_found": result.record is not None,
                "expired": result.expired,
                "unavailable": result.unavailable,
            }
        )
    clock[0] = 3700.0
    status, payload = await store.request_retry(record["tenant"], record["operation_id"], "equality")
    return {
        "source_read": reads,
        "source_real_valkey_retry_at_equality": {"status": status, "record_found": payload is not None},
    }


executable = os.environ.get("SLOPSEARX_TEST_VALKEY_SERVER", "/private/tmp/ssx-f07-valkey-pinned/bin/valkey-server")
with tempfile.TemporaryDirectory(prefix="ssx-w11-source-") as directory:
    sock_path = pathlib.Path(directory) / "s.sock"
    server = subprocess.Popen(
        [
            executable,
            "--port",
            "0",
            "--save",
            "",
            "--appendonly",
            "no",
            "--dir",
            directory,
            "--unixsocket",
            str(sock_path),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        deadline = real_time.monotonic() + 4
        while not sock_path.exists():
            if server.poll() is not None or real_time.monotonic() >= deadline:
                raise RuntimeError("private Valkey startup failed or timed out")
            real_time.sleep(0.01)
        observed = asyncio.run(controls(sock_path))
    finally:
        server.terminate()
        try:
            server.wait(timeout=4)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=4)
observed.update(
    {
        "schema_version": 1,
        "plan_id": "W11",
        "evidence_kind": "actual_source_methods_and_private_valkey",
        "baseline": out["baseline"],
        "python": __import__("sys").version.split()[0],
        "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "exporter_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
    }
)
(ROOT / "crates/xtask/fixtures/w11/source-controls.json").write_text(json.dumps(observed, indent=2) + "\n")
