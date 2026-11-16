"""T06 temporary manual reference oracle. Never a backend/native gate dependency; retire G08."""

import asyncio
import hashlib
import importlib.metadata
import importlib.util
import json
import subprocess
import sys
import socket
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BASELINE = "6c44976444340cc8f343fef61e2e8d723b189c19"
spec = importlib.util.spec_from_file_location("oracle_guard", ROOT / "docs/rust-rewrite/replay/oracle/export.py")
guard = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = guard
spec.loader.exec_module(guard)
guard.sanitize_environment(candidate=False)
guard.assert_no_shared_store_url()
guard.install_loopback_network_guard()
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
if sys.version_info[:3] != (3, 14, 7):
    raise RuntimeError("T06 oracle requires pinned CPython 3.14.7")
if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, env=guard.git_environment()).strip() != BASELINE:
    raise RuntimeError("wrong source revision")
VERSIONS = {k: importlib.metadata.version(k) for k in ("fastmcp", "mcp")}
if VERSIONS != {"fastmcp": "4.0.11", "mcp": "2.3.0"}:
    raise RuntimeError("wrong source SDKs")
NAMES = ["search", "search_targeted", "search_jobs", "search_security", "search_science", "list_capabilities", "explain_search_scope", "get_service_status", "read_results", "read_result", "read_entities"]
NAMES = ["slopsearx_" + name for name in NAMES]


class WireClient:
    """Capture actual protocol 2025-11-25 wire JSON, without SDK-added defaults."""

    def __init__(self, server):
        self.server = server
        self.sequence = 0

    async def __aenter__(self):
        import httpx
        import uvicorn

        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        sock.listen(128)
        guard._allowed_loopback_ports.add(sock.getsockname()[1])
        self.host = uvicorn.Server(uvicorn.Config(self.server.http_app(transport="http"), log_level="error"))
        self.task = asyncio.create_task(self.host.serve(sockets=[sock]))
        while not self.host.started:
            if self.task.done():
                await self.task
            await asyncio.sleep(0.001)
        self.http = httpx.AsyncClient(base_url=f"http://127.0.0.1:{sock.getsockname()[1]}", trust_env=False, headers={"accept": "application/json, text/event-stream"})
        await self.rpc("initialize", {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t06-oracle", "version": "1"}})
        await self.http.post("/mcp", json={"jsonrpc": "2.0", "method": "notifications/initialized"})
        return self

    async def __aexit__(self, *exc):
        await self.http.aclose()
        self.host.should_exit = True
        await asyncio.wait_for(self.task, 10)

    async def rpc(self, method, params=None):
        self.sequence += 1
        response = await self.http.post("/mcp", json={"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params or {}})
        response.raise_for_status()
        if response.headers.get("mcp-session-id"):
            self.http.headers["mcp-session-id"] = response.headers["mcp-session-id"]
        self.http.headers["mcp-protocol-version"] = "2025-11-25"
        if response.headers.get("content-type", "").startswith("text/event-stream"):
            lines = [line[5:].strip() for line in response.text.splitlines() if line.startswith("data:")]
            value = json.loads(lines[-1])
        else:
            value = response.json()
        if "error" in value:
            return {"rpc_error": value["error"]}
        return value["result"]

    async def list_tools(self):
        return (await self.rpc("tools/list"))["tools"]

    async def list_resources(self):
        return (await self.rpc("resources/list"))["resources"]

    async def list_resource_templates(self):
        return (await self.rpc("resources/templates/list"))["resourceTemplates"]

    async def list_prompts(self):
        return (await self.rpc("prompts/list"))["prompts"]

    async def get_prompt_mcp(self, name, arguments):
        return await self.rpc("prompts/get", {"name": name, "arguments": arguments})

    async def call_tool_mcp(self, name, arguments):
        return await self.rpc("tools/call", {"name": name, "arguments": arguments})


async def main():
    from slopsearx.mcp.harness import create_fixture_server, build_fixture_context
    from slopsearx.mcp import prompts, resources

    def dump(v):
        return v

    context = build_fixture_context()

    async def factory():
        return context

    async with WireClient(create_fixture_server(state_factory=factory)) as client:
        catalog = {
            "tools": [dump(v) for v in await client.list_tools() if v["name"] in NAMES],
            "resources": [dump(v) for v in await client.list_resources()],
            "resourceTemplates": [dump(v) for v in await client.list_resource_templates()],
            "prompts": [dump(v) for v in await client.list_prompts()],
        }
        if {v["name"] for v in catalog["tools"]} != set(NAMES):
            raise RuntimeError("incomplete source subset")
        prompt_calls = {}
        for v in catalog["prompts"]:
            argument = v["arguments"][0]["name"]
            prompt_calls[v["name"]] = {
                "argument": argument,
                "template": getattr(prompts, v["name"])("<CALLER_DATA>"),
                "result": dump(await client.get_prompt_mcp(v["name"], {argument: "<CALLER_DATA>"})),
            }
        routing_profiles = resources.render_routing_profiles()
        calls = []
        for name, args in [
            ("search", {"query": ""}),
            ("search", {"query": "q", "safesearch": "strict"}),
            ("search", {"query": "q", "engines": ["wikipedia", "hibp"]}),
            ("search_jobs", {"company": "example"}),
            ("search_science", {"query": "q"}),
            ("search_security", {"query": "q"}),
            ("read_results", {"cursor": "missing"}),
            ("read_result", {"result_id": "bad"}),
            ("read_entities", {"cursor": "missing"}),
            ("search", {}),
            ("search", {"query": None}),
            ("search", {"query": "q", "unexpected": 1}),
            ("read_entities", {"cursor": "missing", "version": True}),
            ("search", {"query": "q", "interactive_timeout_ms": True}),
            ("search", {"query": "q", "max_results": "1"}),
        ]:
            calls.append({"name": "slopsearx_" + name, "arguments": args, "result": dump(await client.call_tool_mcp("slopsearx_" + name, args))})
        validation_cases = []
        seeds = {
            "slopsearx_search": {"query": "q", "engines": ["wikipedia"]},
            "slopsearx_search_targeted": {"query": "q", "engines": ["wikipedia"]},
            "slopsearx_search_jobs": {"company": "example", "sources": ["wikipedia"]},
            "slopsearx_search_security": {"query": "q", "engines": ["wikipedia"]},
            "slopsearx_search_science": {"query": "q", "engines": ["wikipedia"]},
            "slopsearx_list_capabilities": {},
            "slopsearx_explain_search_scope": {"query": "q", "engines": ["wikipedia"]},
            "slopsearx_get_service_status": {},
            "slopsearx_read_results": {"cursor": "missing"},
            "slopsearx_read_result": {"result_id": "missing:0"},
            "slopsearx_read_entities": {"cursor": "missing"},
        }

        async def validation_case(name, label, args):
            before = {name: engine.calls for name, engine in context.active_engines.items()}
            result = await client.call_tool_mcp(name, args)
            after = {name: engine.calls for name, engine in context.active_engines.items()}
            validation_cases.append({"name": name, "case": label, "arguments": args, "result": result, "dispatch_delta": {k: after[k] - before[k] for k in before}})

        for tool in catalog["tools"]:
            name = tool["name"]
            await validation_case(name, "defaults", dict(seeds[name]))
            await validation_case(name, "unknown_argument", {**seeds[name], "unexpected": 1})
            for required in tool["inputSchema"].get("required", []):
                args = dict(seeds[name])
                args.pop(required, None)
                await validation_case(name, "missing_" + required, args)
            for key in tool["inputSchema"]["properties"]:
                for label, value in [("null", None), ("object", {}), ("array", [1]), ("bool", True), ("integer", 1), ("float_integer", 1.0), ("float_fraction", 1.5), ("numeric_string", "1"), ("float_string", "1.0"), ("exponent_string", "1e1"), ("huge_integer", 18446744073709551617)]:
                    await validation_case(name, key + "_" + label, {**seeds[name], key: value})
    sources = ["slopsearx/mcp/" + v + ".py" for v in ("server", "state", "tool_registry", "tools", "result_serialization", "entity_projection", "resources", "prompts", "harness")]
    out = {"baseline": BASELINE, "versions": VERSIONS, "protocol": "2025-11-25", "sources": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in sources}, "catalog": catalog, "prompt_calls": prompt_calls, "routing_profiles": routing_profiles, "calls": calls, "validation_cases": validation_cases}
    target = Path(sys.argv[1])
    target.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    asyncio.run(main())
