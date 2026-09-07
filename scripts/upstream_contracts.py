"""Bounded, opt-in upstream contract probes and sanitized replay captures.

Run from the checkout: python -m scripts.upstream_contracts --output report.json
"""

from __future__ import annotations

import argparse
import asyncio
import json
import math
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

import httpx

from engines.crates import CratesAdapter
from engines.npm import NpmAdapter
from engines.openalex import OpenAlexAdapter
from slopsearx.adapter import EngineStatus

PROBES = {
    "npm": (NpmAdapter, "typescript", "registry.npmjs.org", "/-/v1/search"),
    "openalex": (OpenAlexAdapter, "Attention Is All You Need", "api.openalex.org", "/works"),
    "crates": (CratesAdapter, "serde", "crates.io", "/api/v1/crates"),
}
MAX_BYTES = 1024 * 1024
TIMEOUT_SECONDS = 10


class BoundedTransport(httpx.AsyncBaseTransport):
    """One allowlisted HTTPS request; no proxy, retry, redirect, or unbounded body."""

    def __init__(self, name: str, inner: httpx.AsyncBaseTransport | None = None):
        self.host, self.path = PROBES[name][2:]
        self.inner = inner if inner is not None else httpx.AsyncHTTPTransport(retries=0)
        self.calls = 0
        self.status: int | None = None
        self.body = b""
        self.failure: str | None = None
        self.unavailable = False

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        try:
            return await self._send(request)
        except httpx.RequestError:
            self.unavailable = self.failure is None
            raise

    async def _send(self, request: httpx.Request) -> httpx.Response:
        self.calls += 1
        if (
            self.calls > 1
            or request.method != "GET"
            or request.url.scheme != "https"
            or request.url.host != self.host
            or request.url.port not in (None, 443)
            or request.url.path != self.path
        ):
            self.failure = "request_outside_allowlist"
            raise httpx.RequestError(self.failure)
        response = await self.inner.handle_async_request(request)
        self.status = response.status_code
        try:
            # Disable compression on requests so the bound also limits decoded JSON.
            chunks = []
            size = 0
            async for chunk in response.aiter_raw():
                size += len(chunk)
                if size > MAX_BYTES:
                    self.failure = "response_size_limit"
                    raise httpx.RequestError(self.failure)
                chunks.append(chunk)
            self.body = b"".join(chunks)
            if response.headers.get("content-encoding", "identity") != "identity":
                self.failure = "unexpected_content_encoding"
                raise httpx.RequestError(self.failure)
            return httpx.Response(response.status_code, content=self.body, headers=response.headers, request=request)
        finally:
            await response.aclose()

    async def aclose(self) -> None:
        await self.inner.aclose()


def sanitized_payload(name: str, body: bytes) -> dict[str, Any]:
    """Keep only public catalog fields consumed by parsers, never headers/authors."""
    data = json.loads(body)
    if name == "npm":
        return {
            "objects": [
                {"package": {key: item["package"].get(key) for key in ("name", "version", "description")}}
                for item in data["objects"][:3]
            ]
        }
    if name == "openalex":
        return {
            "results": [
                {key: item.get(key) for key in ("id", "doi", "title", "publication_date", "relevance_score")}
                for item in data["results"][:3]
            ]
        }
    return {
        "crates": [
            {key: item.get(key) for key in ("name", "max_version", "description", "downloads", "recent_downloads")}
            for item in data["crates"][:3]
        ]
    }


def usable_url(value: str) -> bool:
    parsed = urlsplit(value)
    return bool(
        parsed.scheme in {"http", "https"}
        and parsed.hostname
        and not parsed.username
        and not parsed.password
        and parsed.path.rstrip("/") not in {"/package", "/crates", ""}
        and "https://" not in parsed.path
        and "http://" not in parsed.path
    )


async def probe(name: str, transport: BoundedTransport) -> dict[str, Any]:
    adapter_class, query, _, _ = PROBES[name]
    adapter = adapter_class(config={"timeout_ms": TIMEOUT_SECONDS * 1000, "max_results": 3})
    # Preserve actual request construction and response parsing, replace only I/O.
    adapter.http_client = lambda **kwargs: httpx.AsyncClient(  # type: ignore[method-assign]
        transport=transport,
        timeout=TIMEOUT_SECONDS,
        follow_redirects=False,
        trust_env=False,
        headers={"Accept-Encoding": "identity"},
    )
    report: dict[str, Any] = {"engine": name, "query": query}
    try:
        async with asyncio.timeout(TIMEOUT_SECONDS):
            response = await adapter.search(query)
        report.update(
            adapter_status=response.status.value,
            result_count=len(response.results),
            latency_ms=round(response.latency_ms, 2),
        )
        if transport.failure:
            outcome, reason = "contract_failure", transport.failure
        elif transport.unavailable or transport.status is None or not 200 <= transport.status < 300:
            outcome, reason = "upstream_unavailable", "network_or_http_failure"
        elif response.status != EngineStatus.OK:
            outcome, reason = "contract_failure", "successful_http_not_parsed"
        elif not response.results:
            outcome, reason = "contract_failure", "empty_known_query"
        elif any(not result.title.strip() or not usable_url(result.url) for result in response.results):
            outcome, reason = "contract_failure", "invalid_title_or_url"
        elif not math.isfinite(response.latency_ms) or response.latency_ms <= 0:
            outcome, reason = "contract_failure", "invalid_latency"
        else:
            outcome, reason = "passed", "nonempty_usable_results"
    except TimeoutError:
        outcome, reason = "upstream_unavailable", "probe_deadline"
    except Exception:
        outcome, reason = "contract_failure", "adapter_raised"
    finally:
        await transport.aclose()
    report.update(outcome=outcome, reason=reason, http_status=transport.status, requests=transport.calls)
    return report


async def run(output: Path, captures: Path | None = None) -> int:
    rows = []
    captured_at = datetime.now(UTC).isoformat()
    for name in PROBES:
        transport = BoundedTransport(name)
        row = await probe(name, transport)
        if captures is not None and row["outcome"] == "passed":
            try:
                payload = sanitized_payload(name, transport.body)
                captures.mkdir(parents=True, exist_ok=True)
                (captures / f"{name}.json").write_text(
                    json.dumps(
                        {
                            "engine": name,
                            "query": PROBES[name][1],
                            "captured_at": captured_at,
                            "source": f"https://{PROBES[name][2]}{PROBES[name][3]}",
                            "sanitization": (
                                "allowlisted public parser fields; top three; no headers or personal author fields"
                            ),
                            "payload": payload,
                        },
                        indent=2,
                    )
                    + "\n"
                )
            except (ValueError, TypeError, KeyError):
                row.update(outcome="contract_failure", reason="capture_schema_mismatch")
        rows.append(row)
    report = {
        "captured_at": captured_at,
        "max_requests": len(PROBES),
        "per_probe_deadline_seconds": TIMEOUT_SECONDS,
        "max_response_bytes": MAX_BYTES,
        "relevance_evaluated": False,
        "probes": rows,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return (
        1
        if any(row["outcome"] == "contract_failure" for row in rows)
        else (2 if any(row["outcome"] == "upstream_unavailable" for row in rows) else 0)
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--capture-dir", type=Path)
    args = parser.parse_args()
    raise SystemExit(asyncio.run(run(args.output, args.capture_dir)))


if __name__ == "__main__":
    main()
