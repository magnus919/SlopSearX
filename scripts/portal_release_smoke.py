"""Smoke-test a deployed SlopSearX portal without third-party dependencies.

The check intentionally accepts HTTP 503 for a search when upstream engines are
unavailable: the response must still be a valid, truthful SlopSearX envelope.
This keeps deployment verification separate from external-engine availability.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


@dataclass(frozen=True)
class Probe:
    status: int
    headers: dict[str, str]
    body: bytes


def _probe(url: str, *, accept: str = "*/*") -> Probe:
    request = Request(url, headers={"Accept": accept})
    try:
        with urlopen(request, timeout=30) as response:
            return Probe(response.status, dict(response.headers.items()), response.read())
    except HTTPError as error:
        return Probe(error.code, dict(error.headers.items()), error.read())
    except URLError as error:
        raise RuntimeError(f"request failed for {url}: {error.reason}") from error


def _text(probe: Probe) -> str:
    return probe.body.decode("utf-8", errors="replace")


def _check(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8080")
    parser.add_argument("--query", default="portal-smoke")
    parser.add_argument("--expected-theme", choices=("dark", "darker"), default="dark")
    args = parser.parse_args()
    base_url = args.base_url.rstrip("/")

    landing = _probe(f"{base_url}/", accept="text/html")
    landing_text = _text(landing)
    _check(landing.status == 200, f"landing returned HTTP {landing.status}")
    _check('<form class="search-form"' in landing_text, "landing page has no search form")
    _check(f'data-default-theme="{args.expected_theme}"' in landing_text, "unexpected portal default theme")
    _check("content-security-policy" in {key.lower() for key in landing.headers}, "missing CSP header")

    healthz = _probe(f"{base_url}/healthz", accept="text/plain")
    _check(healthz.status == 200 and _text(healthz).strip() == "OK", "healthz did not return OK")

    # An intentionally empty category keeps deployment verification
    # deterministic and avoids making the smoke check depend on live engines.
    query = urlencode(
        {
            "q": args.query,
            "format": "json",
            "categories": "__portal_smoke__",
            "interactive_timeout_ms": "1000",
        }
    )
    search = _probe(f"{base_url}/search?{query}", accept="application/json")
    _check(search.status in {200, 503}, f"search returned unexpected HTTP {search.status}")
    try:
        payload = json.loads(_text(search))
    except json.JSONDecodeError as error:
        raise RuntimeError("search response was not JSON") from error
    _check(payload.get("query") == args.query, "search response did not preserve the query")
    _check(isinstance(payload.get("results"), list), "search response has no results list")

    print(f"portal smoke passed: landing=200 healthz=200 search={search.status} theme={args.expected_theme}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
