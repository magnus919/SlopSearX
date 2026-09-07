"""Controlled HTTP/1.1 reuse probe and transport ownership/isolation contracts."""

import asyncio
import statistics
import time

import httpx
import pytest

from engines.wikipedia import WikipediaAdapter
from slopsearx.adapter import EngineStatus


@pytest.mark.asyncio
async def test_connection_reuse_probe(monkeypatch, capsys):
    """Twenty identical searches: count real TCP accepts, not mocked calls.

    Run with pytest -s tests/test_http_pooling.py to see local p50/p95 timings.
    This proves reuse, not a production latency improvement or TLS benefit.
    """
    monkeypatch.setattr("slopsearx.adapter.getproxies", lambda: {})
    connections = 0
    handlers = set()

    async def serve(reader, writer):
        nonlocal connections
        connections += 1
        task = asyncio.current_task()
        handlers.add(task)
        try:
            while await reader.readuntil(b"\r\n\r\n"):
                body = b'["query", [], [], []]'
                writer.write(b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
                await writer.drain()
        except asyncio.IncompleteReadError:
            pass
        finally:
            writer.close()
            await writer.wait_closed()
            handlers.discard(task)

    server = await asyncio.start_server(serve, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    url = f"http://127.0.0.1:{port}"
    adapter = WikipediaAdapter({"base_url": url})
    ephemeral_adapter = WikipediaAdapter({"base_url": url})
    ephemeral_adapter.http_client = lambda **kwargs: httpx.AsyncClient(trust_env=False, **kwargs)
    samples = {}
    counts = {}
    try:
        for mode in ("ephemeral", "pooled"):
            before = connections
            timings = []
            for _ in range(20):
                start = time.perf_counter()
                selected = ephemeral_adapter if mode == "ephemeral" else adapter
                response = await selected.search("query")
                assert response.status is EngineStatus.OK
                assert response.results == []
                timings.append((time.perf_counter() - start) * 1000)
            counts[mode] = connections - before
            samples[mode] = (statistics.median(timings), sorted(timings)[18])
        assert counts == {"ephemeral": 20, "pooled": 1}
        with capsys.disabled():
            print(f"\nTCP accepts={counts}; local milliseconds (p50,p95)={samples}")
    finally:
        await adapter.shutdown()
        await adapter.shutdown()
        server.close()
        await server.wait_closed()
        if handlers:
            await asyncio.gather(*handlers)


@pytest.mark.asyncio
async def test_sessions_isolate_cookies_and_injected_transport_ownership():
    seen = []

    class Transport(httpx.MockTransport):
        closed = False

        async def aclose(self):
            self.closed = True

    def respond(request):
        seen.append(request.headers.get("cookie"))
        return httpx.Response(200, headers={"set-cookie": "session=one; Path=/"}, json=[])

    transport = Transport(respond)
    adapter = WikipediaAdapter()
    adapter.set_http_transport(transport)
    async with adapter.http_client() as client:
        await client.get("https://example.org")
        await client.get("https://example.org")
    async with adapter.http_client() as client:
        await client.get("https://example.org")
    assert seen == [None, "session=one", None]
    await adapter.shutdown()
    assert not transport.closed
    assert (await adapter.search("query")).status is EngineStatus.ERROR
    await transport.aclose()


@pytest.mark.asyncio
async def test_proxy_pools_separate_bounded_and_closed(monkeypatch):
    created = []

    class Transport(httpx.MockTransport):
        closed = False

        async def aclose(self):
            self.closed = True

    def factory(**kwargs):
        created.append((kwargs, Transport(lambda request: httpx.Response(200))))
        return created[-1][1]

    monkeypatch.setattr("slopsearx.adapter.httpx.AsyncHTTPTransport", factory)
    adapter = WikipediaAdapter()
    for index in range(10):
        proxy = f"http://proxy{index}:8080"
        async with adapter.http_client(proxies={"http://": proxy, "https://": proxy}) as client:
            # No actual proxy lookup is needed to inspect pool lifecycle.
            assert not client.is_closed
    assert len(adapter._http_pools) == 8
    assert len(created) == 8
    for kwargs, _ in created:
        assert kwargs["limits"].max_connections == 20
        assert kwargs["limits"].max_keepalive_connections == 10
    await adapter.shutdown()
    assert all(transport.closed for _, transport in created)


async def test_environment_proxy_fallback_preserves_httpx_resolution(monkeypatch):
    monkeypatch.setattr("slopsearx.adapter.getproxies", lambda: {"http": "http://proxy:8080"})
    adapter = WikipediaAdapter()
    client = adapter.http_client()
    assert not adapter._http_pools
    assert client.trust_env
    await client.aclose()


async def test_no_proxy_alone_does_not_disable_pooling(monkeypatch):
    monkeypatch.setattr("slopsearx.adapter.getproxies", lambda: {"no": "localhost"})
    adapter = WikipediaAdapter()
    async with adapter.http_client():
        assert len(adapter._http_pools) == 1
    await adapter.shutdown()
