#!/usr/bin/env python3
"""Developer-only T01 observations; extends the accepted F04 offline oracle guards."""

import asyncio
import hashlib
import importlib.util
import json
import socket
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
BASELINE = "7b2d876307189d7a62000d1f289b3ef4b65eaff6"
SOURCE = Path(sys.argv[1]).resolve()
spec = importlib.util.spec_from_file_location("f04_oracle", Path(__file__).with_name("export.py"))
guard = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = guard
spec.loader.exec_module(guard)
guard.sanitize_environment(candidate=False)
guard.assert_no_shared_store_url()
if (
    subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=SOURCE, text=True, env=guard.git_environment()).strip()
    != BASELINE
):
    raise RuntimeError("wrong pinned source revision")
if subprocess.check_output(
    ["git", "status", "--porcelain", "--untracked-files=all"], cwd=SOURCE, env=guard.git_environment()
):
    raise RuntimeError("dirty oracle source")
if any(list((SOURCE / root).rglob("*.pyc")) for root in ("slopsearx", "engines", "tests")):
    raise RuntimeError("ignored bytecode in oracle source")
sys.dont_write_bytecode = True
sys.path.insert(0, str(SOURCE))
guard.install_loopback_network_guard()


async def main():
    import importlib.metadata

    import httpx
    import uvicorn

    versions = {name: importlib.metadata.version(name) for name in ("fastmcp", "mcp")}
    if versions != {"fastmcp": "4.0.11", "mcp": "2.3.0"}:
        raise RuntimeError(f"wrong oracle SDK identity: {versions}")
    from fastmcp.tools import FunctionTool

    from slopsearx.mcp.harness import build_fixture_context, fixture_config
    from slopsearx.mcp.server import create_server

    async def state():
        return build_fixture_context()

    # Exercise the pinned application construction and its actual SSE route implementation.
    server = create_server(state_factory=state, config=fixture_config())

    async def echo(payload: dict, note: str | None = None) -> dict:
        return {"payload": payload, "note": note}

    tool = FunctionTool.from_function(
        echo, name="t01_echo", description="Echo nested values for transport qualification"
    )
    # Deliberately exercise schema pass-through rather than signature reconstruction.
    schema = {
        "type": "object",
        "properties": {
            "payload": {"$ref": "#/$defs/Nested"},
            "note": {"anyOf": [{"type": "string"}, {"type": "null"}], "default": None},
        },
        "required": ["payload"],
        "$defs": {
            "Nested": {
                "type": "object",
                "properties": {"items": {"type": "array", "items": {"type": "integer"}}},
                "required": ["items"],
                "additionalProperties": False,
            }
        },
        "additionalProperties": False,
    }
    tool.parameters = schema
    server.add_tool(tool)
    large_tool = FunctionTool.from_function(
        echo, name="t01_large", description="Opaque numeric descriptor qualification"
    )
    large_tool.parameters = {
        "type": "object",
        "properties": {"payload": {"type": "object"}},
        "x-boundary": {"integer": 18446744073709551617, "ordinary_object": {"$serde_json::private::Number": "123"}},
    }
    server.add_tool(large_tool)

    @server.resource("t01://text", name="t01_text", mime_type="text/plain")
    def text_resource() -> str:
        return "fixture text"

    @server.prompt(name="t01_prompt")
    def prompt(topic: str) -> str:
        return f"topic: {topic}"

    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    sock.listen()
    port = sock.getsockname()[1]
    guard._allowed_loopback_ports.add(port)
    host = uvicorn.Server(uvicorn.Config(server.http_app(transport="sse"), log_level="error", lifespan="on"))
    task = asyncio.create_task(host.serve(sockets=[sock]))
    while not host.started:
        await asyncio.sleep(0.01)
    base = f"http://127.0.0.1:{port}"
    observed = {}
    try:
        async with httpx.AsyncClient(base_url=base, trust_env=False, timeout=5) as client:
            async with client.stream("GET", "/sse") as stream:
                lines = stream.aiter_lines()

                async def event():
                    kind = None
                    data = []
                    async for line in lines:
                        if line.startswith("event:"):
                            kind = line[6:].strip()
                        elif line.startswith("data:"):
                            data.append(line[5:].strip())
                        elif not line and data:
                            return kind, "\n".join(data)
                    raise RuntimeError("SSE ended")

                kind, endpoint = await event()
                observed["endpoint_event"] = kind
                observed["message_prefix"] = endpoint.split("?")[0]
                observed["content_type"] = stream.headers["content-type"]
                for label, url, content in [
                    ("missing_session", "/messages/", "{}"),
                    ("invalid_session", "/messages/?session_id=invalid", "{}"),
                    ("unknown_session", "/messages/?session_id=00000000000000000000000000000000", "{}"),
                    ("invalid_json", endpoint, "{"),
                    ("invalid_rpc", endpoint, "{}"),
                ]:
                    r = await client.post(url, content=content, headers={"content-type": "application/json"})
                    observed[label] = {"status": r.status_code, "body": r.text}
                for label, method, url, content, headers in [
                    ("wrong_method", "GET", endpoint, "{}", {"content-type": "application/json"}),
                    ("wrong_content_type", "POST", endpoint, "{}", {"content-type": "text/plain"}),
                    ("oversized", "POST", endpoint, "x" * (4 * 1024 * 1024 + 1), {"content-type": "application/json"}),
                ]:
                    r = await client.request(method, url, content=content, headers=headers)
                    observed[label] = {"status": r.status_code, "body": r.text}
                lifecycle = {}
                for label, request_id, method, params in [
                    ("before_initialize", 100, "tools/list", {}),
                    ("malformed_initialize", 101, "initialize", {}),
                    (
                        "unsupported_initialize",
                        102,
                        "initialize",
                        {
                            "protocolVersion": "1900-01-01",
                            "capabilities": {},
                            "clientInfo": {"name": "t01-oracle", "version": "1"},
                        },
                    ),
                    ("before_initialized_notification", 103, "tools/list", {}),
                    (
                        "repeated_initialize",
                        104,
                        "initialize",
                        {
                            "protocolVersion": "2024-11-05",
                            "capabilities": {},
                            "clientInfo": {"name": "t01-oracle", "version": "1"},
                        },
                    ),
                ]:
                    await client.post(
                        endpoint, json={"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
                    )
                    _, body = await event()
                    reply = json.loads(body)
                    if method == "tools/list" and "result" in reply:
                        reply["result"] = {"tool_names": [tool["name"] for tool in reply["result"]["tools"]]}
                    lifecycle[label] = reply
                observed["lifecycle"] = lifecycle
                request = {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {},
                        "clientInfo": {"name": "t01-oracle", "version": "1"},
                    },
                }
                r = await client.post(endpoint, json=request)
                observed["accepted"] = {"status": r.status_code, "body": r.text}
                _, body = await event()
                observed["initialize"] = json.loads(body)
                await client.post(endpoint, json={"jsonrpc": "2.0", "method": "notifications/initialized"})
                await client.post(endpoint, json={"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
                _, body = await event()
                descriptor = json.loads(body)
                observed["tools"] = [t for t in descriptor["result"]["tools"] if t["name"] == "t01_echo"]
                observed["large_tools"] = [t for t in descriptor["result"]["tools"] if t["name"] == "t01_large"]
                observed["public_tools"] = [
                    t for t in descriptor["result"]["tools"] if not t["name"].startswith("t01_")
                ]
                for request_id, method, params, key in [
                    (3, "resources/list", {}, "resources"),
                    (4, "prompts/list", {}, "prompts"),
                    (
                        5,
                        "tools/call",
                        {"name": "t01_echo", "arguments": {"payload": {"items": [1, 2]}, "note": None}},
                        "echo_result",
                    ),
                    (
                        8,
                        "tools/call",
                        {
                            "name": "t01_echo",
                            "arguments": {
                                "payload": {
                                    "items": [9223372036854775808, 18446744073709551617, -18446744073709551617]
                                },
                                "note": None,
                            },
                        },
                        "integer_echo_result",
                    ),
                    (6, "resources/read", {"uri": "t01://text"}, "resource_result"),
                    (7, "prompts/get", {"name": "t01_prompt", "arguments": {"topic": "test"}}, "prompt_result"),
                ]:
                    await client.post(
                        endpoint, json={"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
                    )
                    _, body = await event()
                    result = json.loads(body)["result"]
                    if key in ("resources", "prompts"):
                        observed["public_" + key] = [v for v in result[key] if not v["name"].startswith("t01_")]
                        result = [v for v in result[key] if v["name"].startswith("t01_")]
                    observed[key] = result
                head = await client.head("/sse")
                observed["head"] = {
                    "status": head.status_code,
                    "content_type": head.headers.get("content-type"),
                    "body": head.text,
                }
    finally:
        host.should_exit = True
        await asyncio.wait_for(task, 5)
    paths = ["slopsearx/mcp/server.py", "slopsearx/mcp/harness.py", "slopsearx/mcp/security.py", "uv.lock"]
    out = {
        "schema_version": 1,
        "baseline": BASELINE,
        "sources": {
            p: subprocess.check_output(["git", "hash-object", p], cwd=SOURCE, text=True).strip() for p in paths
        },
        "oracle": {
            "path": "docs/rust-rewrite/replay/oracle/export_mcp.py",
            "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "git_blob": subprocess.check_output(
                ["git", "hash-object", str(Path(__file__))], cwd=ROOT, text=True
            ).strip(),
            "f04_helper_git_blob": subprocess.check_output(
                ["git", "hash-object", str(Path(__file__).with_name("export.py"))], cwd=ROOT, text=True
            ).strip(),
        },
        "versions": versions,
        "python": sys.version,
        "observations": observed,
    }
    print(json.dumps(out, indent=2, sort_keys=True))


asyncio.run(asyncio.wait_for(main(), 30))
