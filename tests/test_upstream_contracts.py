"""Offline replay of minimally retained live payloads; no network dependency."""

from __future__ import annotations

import json
from pathlib import Path

import httpx
import pytest

from scripts import upstream_contracts as contracts

FIXTURES = Path(__file__).parent / "fixtures" / "upstream_contracts"


class Body(httpx.AsyncByteStream):
    def __init__(self, content):
        self.content = content

    async def __aiter__(self):
        yield self.content


def transport(name, content, status=200, headers=None):
    def handler(request):
        assert request.headers["accept-encoding"] == "identity"
        assert "authorization" not in request.headers
        return httpx.Response(status, stream=Body(content), headers=headers)

    return contracts.BoundedTransport(name, httpx.MockTransport(handler))


@pytest.mark.parametrize("name", contracts.PROBES)
async def test_captured_payload_replays_actual_adapter(name):
    fixture = json.loads((FIXTURES / f"{name}.json").read_text())
    body = json.dumps(fixture["payload"]).encode()
    io = transport(name, body)
    report = await contracts.probe(name, io)
    assert report["outcome"] == "passed"
    assert report["result_count"] == 3
    assert report["requests"] == 1
    assert report["latency_ms"] >= 0
    assert contracts.sanitized_payload(name, body) == fixture["payload"]
    assert fixture["captured_at"] and fixture["source"].startswith("https://")


@pytest.mark.parametrize("status", [403, 429, 503])
async def test_upstream_failure_is_not_parser_regression(status):
    report = await contracts.probe("npm", transport("npm", b"upstream denied", status))
    assert report["outcome"] == "upstream_unavailable"
    assert report["http_status"] == status
    assert report["adapter_status"] == ("rate_limited" if status == 429 else "error")
    assert "upstream denied" not in json.dumps(report)


@pytest.mark.parametrize(
    "body", [b"not json", b"{}", b'{"objects": []}', b'{"objects": null}', b'{"objects": [{"package": {}}]}']
)
async def test_http_success_without_usable_results_fails_contract(body):
    report = await contracts.probe("npm", transport("npm", body))
    assert report["outcome"] == "contract_failure"


async def test_size_bound():
    report = await contracts.probe("npm", transport("npm", b"x" * (contracts.MAX_BYTES + 1)))
    assert report["reason"] == "response_size_limit"


async def test_no_compressed_payload_expansion():
    report = await contracts.probe("npm", transport("npm", b"zip", headers={"content-encoding": "gzip"}))
    assert report["reason"] == "unexpected_content_encoding"


async def test_network_error_not_contract_failure():
    def handler(request):
        raise httpx.ConnectError("secret must not reach report", request=request)

    report = await contracts.probe("npm", contracts.BoundedTransport("npm", httpx.MockTransport(handler)))
    assert report["outcome"] == "upstream_unavailable"
    assert "secret" not in json.dumps(report)


async def test_probe_deadline(monkeypatch):
    import asyncio

    async def handler(request):
        await asyncio.sleep(10)
        return httpx.Response(200)

    monkeypatch.setattr(contracts, "TIMEOUT_SECONDS", 0.01)
    report = await contracts.probe("npm", contracts.BoundedTransport("npm", httpx.MockTransport(handler)))
    assert report["reason"] == "probe_deadline"


async def test_allowlist_rejects_other_hosts_and_extra_requests():
    io = transport("npm", b"{}")
    with pytest.raises(httpx.RequestError):
        await io.handle_async_request(httpx.Request("GET", "https://example.com/"))
    assert io.failure == "request_outside_allowlist"
    io = transport("npm", b"{}")
    request = httpx.Request("GET", "https://registry.npmjs.org/-/v1/search", headers={"accept-encoding": "identity"})
    await io.handle_async_request(request)
    with pytest.raises(httpx.RequestError):
        await io.handle_async_request(request)
    await io.aclose()


def test_sanitization_discards_unneeded_identity_fields():
    body = json.dumps(
        {
            "objects": [
                {
                    "package": {
                        "name": "x",
                        "version": "1",
                        "description": "d",
                        "publisher": {"email": "private@example.com"},
                        "maintainers": ["person"],
                    }
                }
            ]
        }
    ).encode()
    assert "private" not in json.dumps(contracts.sanitized_payload("npm", body))
    assert "maintainers" not in json.dumps(contracts.sanitized_payload("npm", body))


@pytest.mark.parametrize(
    "url", ["https://doi.org/https://doi.org/x", "https://user:pass@example.com/x", "javascript:x", ""]
)
def test_bad_urls(url):
    assert not contracts.usable_url(url)


@pytest.mark.parametrize("name", contracts.PROBES)
async def test_captured_records_preserve_title_and_canonical_link(name):
    fixture = json.loads((FIXTURES / f"{name}.json").read_text())
    payload = fixture["payload"]
    adapter = contracts.PROBES[name][0](config={"max_results": 3})
    adapter.http_client = lambda **kwargs: httpx.AsyncClient(
        transport=httpx.MockTransport(lambda request: httpx.Response(200, json=payload))
    )
    response = await adapter.search(fixture["query"])
    if name == "npm":
        expected = [
            (
                f"{row['package']['name']}@{row['package']['version']}",
                f"https://www.npmjs.com/package/{row['package']['name']}",
            )
            for row in payload["objects"]
        ]
    elif name == "crates":
        expected = [
            (f"{row['name']} v{row['max_version']}", f"https://crates.io/crates/{row['name']}")
            for row in payload["crates"]
        ]
    else:
        expected = [(row["title"], row["doi"] or row["id"]) for row in payload["results"]]
    assert [(result.title, result.url) for result in response.results] == expected
