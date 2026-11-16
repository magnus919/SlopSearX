"""Temporary real private-loopback source timeout evidence, retired G08."""

import asyncio
import hashlib
import json
import pathlib
import sys
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
assert sys.version_info[:3] == (3, 14, 7) and unicodedata.unidata_version == "16.0.0"
from engines.edgar import EdgarAdapter  # noqa: E402
from engines.fred import FredAdapter  # noqa: E402
from engines.openfda import OpenFDAAdapter  # noqa: E402
from engines.oyez import OyezAdapter  # noqa: E402


async def main():
    records = []
    for cls in [EdgarAdapter, FredAdapter, OpenFDAAdapter, OyezAdapter]:
        for timeout in [0, -1, 50]:
            requests = []
            closed = []
            tasks = []

            async def handle(reader, writer):
                task = asyncio.current_task()
                tasks.append(task)
                try:
                    try:
                        wire = await reader.readuntil(b"\r\n\r\n")
                    except asyncio.IncompleteReadError as exc:
                        assert not exc.partial
                        closed.append(True)
                        return
                    requests.append(wire.decode().splitlines()[0].replace("synthetic-fred-key", "<synthetic-key>"))
                    if b"content-length:" in wire.lower():
                        n = int(
                            next(
                                line.split(b":", 1)[1]
                                for line in wire.lower().splitlines()
                                if line.startswith(b"content-length:")
                            )
                        )
                        await reader.readexactly(n)
                    closed.append((await reader.read(1)) == b"")
                finally:
                    writer.close()
                    await writer.wait_closed()

            server = await asyncio.start_server(handle, "127.0.0.1", 0)
            port = server.sockets[0].getsockname()[1]
            cfg = {"timeout_ms": timeout, "api_key": "synthetic-fred-key"}
            cfg["search_url" if cls.name == "oyez" else "base_url"] = f"http://127.0.0.1:{port}/slow"
            adapter = cls(cfg)
            result = await adapter.search("test")
            await adapter.shutdown()
            await asyncio.sleep(0.05)
            server.close()
            await server.wait_closed()
            if tasks:
                await asyncio.wait_for(asyncio.gather(*tasks), 2)
            records.append(
                dict(
                    id=f"{cls.name}-loopback-timeout-{timeout}",
                    engine=cls.name,
                    timeout_ms=timeout,
                    status=result.status.value,
                    error_message=result.error_message,
                    requests=requests,
                    socket_eof=closed,
                )
            )
    out = {
        "schema_version": 1,
        "python": sys.version,
        "unicode": unicodedata.unidata_version,
        "producer_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
        "scope": (
            "actual source HTTPX private loopback, held response, source timer and socket closure; "
            "no mock transport or provider calls"
        ),
        "records": records,
    }
    p = ROOT / "docs/rust-rewrite/a08/timeout-loopback.json"
    p.write_text(json.dumps(out, indent=2) + "\n")
    print(len(records), hashlib.sha256(p.read_bytes()).hexdigest())


asyncio.run(main())
