"""Manual original IA timeout source controls against guarded private loopback. Retire G08."""

# ruff: noqa: E402
import ast
import asyncio
import hashlib
import json
import math
import os
import pathlib
import subprocess
import sys
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[4]
# Consume the exact literal source guards, never import/execute another producer.
tree = ast.parse((ROOT / "docs/rust-rewrite/a09/oracle/export_source.py").read_text())
guards = ast.literal_eval(
    next(
        n.value
        for n in tree.body
        if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "SOURCE_GUARDS" for t in n.targets)
    )
)
for path, sha in guards.items():
    if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != sha:
        raise RuntimeError("A09 timeout source drift: " + path)
if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("A09 oracle requires Python 3.14.7 and Unicode 16.0.0, including optimized mode")
sys.path.insert(0, str(ROOT))
from engines.internetarchive import InternetArchiveAdapter

for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"]:
    if os.environ.get(key):
        raise RuntimeError("proxy environment denied before private-loopback request: " + key)
flags = {}
if sys.platform == "darwin":
    raw = subprocess.check_output(["/usr/sbin/scutil", "--proxy"], text=True)
    for key in ["HTTPEnable", "HTTPSEnable", "SOCKSEnable", "ProxyAutoConfigEnable", "ProxyAutoDiscoveryEnable"]:
        values = [
            int(line.split(":", 1)[1].strip()) for line in raw.splitlines() if line.partition(":")[0].strip() == key
        ]
        if len(values) > 1 or any(values):
            raise RuntimeError("system proxy enabled/ambiguous: " + key)
        flags[key] = values[0] if values else 0


async def one(label, timeout_ms, stall):
    requests = []
    sockets = []
    tasks = []

    async def handle(reader, writer):
        task = asyncio.current_task()
        tasks.append(task)
        try:
            data = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 0.3)
            request = data.decode("ascii")
            request = request.replace(f"127.0.0.1:{port}", "127.0.0.1:<loopback-port>")
            requests.append(request)
            if not stall:
                body = b'{"response":{"docs":[{"identifier":"book","title":"Book","downloads":5000}]}}'
                writer.write(
                    b"HTTP/1.1 200 OK\r\nContent-Length: "
                    + str(len(body)).encode()
                    + b"\r\nConnection: close\r\n\r\n"
                    + body
                )
                await writer.drain()
            eof = await asyncio.wait_for(reader.read(1), 0.3)
            sockets.append(eof == b"")
        except (TimeoutError, asyncio.IncompleteReadError, ConnectionError):
            sockets.append(False)
        finally:
            writer.close()
            await writer.wait_closed()

    server = await asyncio.start_server(handle, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    adapter = InternetArchiveAdapter({"base_url": f"http://127.0.0.1:{port}", "timeout_ms": timeout_ms})
    try:
        try:
            response = await asyncio.wait_for(adapter.search("books"), 0.4)
            output = dict(
                status=response.status.value,
                error_message=response.error_message,
                results=[
                    {
                        k: getattr(r, k)
                        for k in [
                            "url",
                            "title",
                            "content",
                            "engine",
                            "position",
                            "score",
                            "category",
                            "published_date",
                            "thumbnail",
                            "img_src",
                            "media",
                            "payload",
                        ]
                    }
                    for r in response.results
                ],
            )
        except TimeoutError:
            output = dict(harness_cancelled=True, reason="outer .4s evidence bound; no adapter outcome invented")
    finally:
        await adapter.shutdown()
        server.close()
        await server.wait_closed()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
    return dict(
        id=label,
        timeout_ms=timeout_ms
        if math.isfinite(timeout_ms)
        else {"$nonfinite": "nan" if math.isnan(timeout_ms) else "inf" if timeout_ms > 0 else "-inf"},
        stall=stall,
        requests=requests,
        socket_eof=sockets,
        output=output,
    )


async def main():
    records = []
    for label, value in [
        ("zero", 0),
        ("negative", -1),
        ("positive", 50),
        ("inf", math.inf),
        ("negative-inf", -math.inf),
        ("nan", math.nan),
    ]:
        records.append(await one("archive-timeout-" + label + "-immediate", value, False))
        records.append(await one("archive-timeout-" + label + "-stalled", value, True))
    path = ROOT / "docs/rust-rewrite/a09/timeouts.json"
    path.write_text(
        json.dumps(
            dict(
                schema_version=1,
                python=sys.version,
                unicode=unicodedata.unidata_version,
                producer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
                source_guards=guards,
                system_proxy_flags=flags,
                records=records,
            ),
            indent=2,
            allow_nan=False,
        )
        + "\n"
    )
    print(len(records), hashlib.sha256(path.read_bytes()).hexdigest())


asyncio.run(main())
