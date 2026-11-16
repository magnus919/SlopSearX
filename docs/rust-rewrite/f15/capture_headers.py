"""Developer-only actual source header admission capture; no network calls."""

import argparse
import asyncio
import hashlib
import json
import subprocess
from pathlib import Path
from unittest.mock import patch

import httpx

from slopsearx.rerank import JevReranker, RerankCandidate


async def capture():
    cases = []
    inputs = [
        ("boundary", [("content-length", "2000000")]),
        ("oversize", [("content-length", "2000001")]),
        ("plus-underscore", [("content-length", "+2_000_001")]),
        ("negative", [("content-length", "-2000001")]),
        ("invalid", [("content-length", "invalid")]),
        ("suffix", [("content-length", "2000001x")]),
        ("double-plus", [("content-length", "++2000001")]),
        ("repeated-length", [("content-length", "2000001"), ("content-length", "2000001")]),
        ("leading-zero-4300", [("content-length", "0" * 4293 + "2000001")]),
        ("leading-zero-4301", [("content-length", "0" * 4294 + "2000001")]),
        ("identity", [("content-length", "2000001"), ("content-encoding", "identity")]),
        (
            "repeated-encoding",
            [("content-length", "2000001"), ("content-encoding", "identity"), ("content-encoding", "identity")],
        ),
    ]
    original = httpx.AsyncClient
    for name, headers in inputs:
        state = {"requests": 0, "body_reads": 0}

        class Stream(httpx.AsyncByteStream):
            async def __aiter__(self):
                state["body_reads"] += 1
                yield (
                    b'{"model":"jev-1.13.0","answers":{"c0":{"type":"score","score":1},'
                    b'"c1":{"type":"score","score":9}}}'
                )

        def handle(_request):
            state["requests"] += 1
            return httpx.Response(200, headers=headers, stream=Stream())

        with patch(
            "slopsearx.rerank.httpx.AsyncClient", lambda **kw: original(transport=httpx.MockTransport(handle), **kw)
        ):
            provider = JevReranker("synthetic-offline-key")
            decision = await provider.rerank(
                "header",
                tuple(RerankCandidate(f"c{i}", f"title{i}", "https://example.test", "snippet") for i in range(2)),
            )
        cases.append(
            {"name": name, "headers": headers, **state, "ordered_ids": list(decision.ordered_ids) if decision else None}
        )
    sources = []
    for path in ["slopsearx/rerank.py"]:
        sources.append(
            {
                "path": path,
                "git_blob": subprocess.check_output(["git", "hash-object", path], text=True).strip(),
                "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest(),
            }
        )
    return {
        "plan": "F15",
        "source_baseline": "4dfc0f5691b6a5a1c039a2837372b1977422ec27",
        "sources": sources,
        "cases": cases,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    Path(args.output).write_text(json.dumps(asyncio.run(capture()), indent=2) + "\n")
