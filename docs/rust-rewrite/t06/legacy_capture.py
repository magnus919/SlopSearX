"""Manual temporary source SSE tenant characterization; guarded loopback/fake engines only. G08."""
import asyncio
import importlib.util
import json
import socket
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("t06_capture", Path(__file__).with_name("capture.py"))
capture = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = capture
spec.loader.exec_module(capture)


async def main():
    import httpx
    import uvicorn
    from slopsearx.mcp.harness import build_fixture_context, create_fixture_server
    from slopsearx.mcp.oauth import SlopSearxOAuthProvider

    context = build_fixture_context()

    async def factory():
        return context

    provider = SlopSearxOAuthProvider(None, base_url="http://127.0.0.1", issuer_url="http://127.0.0.1")
    a, _ = await provider._issue_tokens("client-A", [], None)
    b, _ = await provider._issue_tokens("client-B", [], None)
    a2, _ = await provider._issue_tokens("client-A", [], None)
    server = create_fixture_server(state_factory=factory, oauth=object(), oauth_provider=provider)
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    sock.listen(128)
    capture.guard._allowed_loopback_ports.add(sock.getsockname()[1])
    host = uvicorn.Server(uvicorn.Config(server.http_app(transport="sse"), log_level="error"))
    task = asyncio.create_task(host.serve(sockets=[sock]))
    while not host.started:
        if task.done():
            await task
        await asyncio.sleep(0.001)
    observations = []
    try:
        async with httpx.AsyncClient(base_url=f"http://127.0.0.1:{sock.getsockname()[1]}", trust_env=False, timeout=5) as client:
            async with client.stream("GET", "/sse", headers={"authorization": f"Bearer {a.token}"}) as response:
                lines = response.aiter_lines()

                async def event():
                    data = []
                    async for line in lines:
                        if line.startswith("data:"):
                            data.append(line[5:].strip())
                        elif not line and data:
                            return "\n".join(data)
                    raise RuntimeError("unexpected SSE EOF")

                endpoint = await event()
                for token, method, params, request_id, label in [
                    (a, "initialize", {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t06-source", "version": "1"}}, 1, "initialize_owner"),
                    (b, "tools/call", {"name": "slopsearx_search", "arguments": {"query": "cross-client-characterization", "engines": ["wikipedia"]}}, 2, "different_verified_client"),
                    (a2, "tools/call", {"name": "slopsearx_search", "arguments": {"query": "same-client-new-token", "engines": ["wikipedia"]}}, 3, "same_client_rotated_token"),
                ]:
                    before = sum(e.calls for e in context.active_engines.values())
                    result = await client.post(endpoint, headers={"authorization": f"Bearer {token.token}"}, json={"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
                    observed = {"case": label, "http_status": result.status_code}
                    if result.status_code == 202:
                        wire = json.loads(await asyncio.wait_for(event(), 3))
                        observed["response_id"] = wire.get("id")
                        observed["sse_owner_received_search_results"] = bool(wire.get("result", {}).get("structuredContent", {}).get("results"))
                    observed["engine_dispatch_delta"] = sum(e.calls for e in context.active_engines.values()) - before
                    observations.append(observed)
                    if request_id == 1:
                        await client.post(endpoint, headers={"authorization": f"Bearer {a.token}"}, json={"jsonrpc": "2.0", "method": "notifications/initialized"})
    finally:
        host.should_exit = True
        await asyncio.wait_for(task, 10)
    Path(sys.argv[1]).write_text(json.dumps({"baseline": capture.BASELINE, "versions": capture.VERSIONS, "observations": observations}, indent=2) + "\n")


if __name__ == "__main__":
    asyncio.run(main())
