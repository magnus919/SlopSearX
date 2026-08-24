"""Resilience tests for the DuckDuckGo scrape adapter.

Covers the zero-config-fallback hardening: a lite.duckduckgo.com/lite/
fallback when the primary html.duckduckgo.com scrape is blocked or
unusable, a realistic browser-like session (headers + homepage cookie
bootstrap), and honest blocked/CAPTCHA-wall classification surfaced via
the adapter's declared ``failure_classes`` (never "success with zero
results" for a walled session).
"""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines

# ---------------------------------------------------------------------------
# Fixtures modeled on the real endpoint layouts
# ---------------------------------------------------------------------------

HOME_HTML = b"<html><body><a href='/'>DuckDuckGo</a></body></html>"

HTML_RESULTS_HTML = b"""<!DOCTYPE html>
<html><body>
<div class="result">
  <div class="result__a">
    <span class="result__url">https://example.com/one</span>
    <h2>Primary Hit</h2>
  </div>
  <div class="result__snippet">Primary snippet.</div>
</div>
</body></html>
"""

LITE_RESULTS_HTML = b"""<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>query at DuckDuckGo</title></head>
<body>
<form action="/lite/" method="POST"><input type="text" name="q"></form>
<div class="filters"><table border="0" cellpadding="0" cellspacing="0">
  <tr>
    <td valign="top">&nbsp;&nbsp;</td>
    <td valign="top">
      <a rel="nofollow" class="result-link"
         href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Fa">Alpha Result</a>
    </td>
  </tr>
  <tr>
    <td class="result-snippet" valign="middle">Snippet for alpha.</td>
  </tr>
  <tr>
    <td valign="middle"><span class="link-text">example.org</span>&nbsp;&nbsp;<span class="date">12 Aug 2026</span></td>
  </tr>
  <tr>
    <td valign="top">&nbsp;&nbsp;</td>
    <td valign="top">
      <a rel="nofollow" class="result-link"
         href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.net%2Fb">Beta Result</a>
    </td>
  </tr>
  <tr>
    <td class="result-snippet" valign="middle">Snippet for beta.</td>
  </tr>
  <tr>
    <td valign="middle"><span class="link-text">example.net</span></td>
  </tr>
</table></div>
</body></html>
"""

CHALLENGE_HTML = b"""<!DOCTYPE html>
<html><head><title>Verifying you're human - DuckDuckGo</title></head>
<body>
<div class="badge-wrap--home">
<h1 class="logo-wrap--home">DuckDuckGo Search</h1>
<p>Unfortunately, bots use DuckDuckGo too.</p>
<p>Please complete the following challenge so we can verify you're human.</p>
<form action="" method="post">
<div class="hcaptcha" data-sitekey="xxx"></div>
<input type="hidden" name="challenge" value="1">
</form>
</div>
</body></html>
"""

NO_RESULTS_HTML = b"""<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>query at DuckDuckGo</title></head>
<body>
<div id="links">
<div class="no-results">No results for &quot;zqxwv1234&quot;.</div>
<p>You might find what you're looking for on the <a href="/">front page</a>.</p>
</div>
</body></html>
"""

EMPTY_HTML = b'<html><body><div id="links"></div></body></html>'


class MockHTTP:
    """Context manager that patches httpx.AsyncClient with a scripted transport."""

    def __init__(self, handler):
        self.transport = httpx.MockTransport(handler)

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


def _hosts(seen: list[httpx.Request]) -> list[str]:
    return [(r.url.host or "") for r in seen]


HTML_HOST = "html.duckduckgo.com"
LITE_HOST = "lite.duckduckgo.com"
HOME_HOST = "duckduckgo.com"


class TestDuckDuckGoResilience:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"duckduckgo": {"enabled": True}})
        return instances["duckduckgo"]

    # -- lite.duckduckgo.com fallback ---------------------------------

    async def test_challenge_on_primary_falls_back_to_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            if host == HTML_HOST:
                return httpx.Response(200, content=CHALLENGE_HTML)
            if host == LITE_HOST:
                return httpx.Response(200, content=LITE_RESULTS_HTML)
            return httpx.Response(500)

        async with MockHTTP(_handler):
            resp = await adapter.search("project gutenberg")

        assert resp.status == EngineStatus.OK
        assert [r.url for r in resp.results] == [
            "https://example.org/a",
            "https://example.net/b",
        ]
        hosts = _hosts(seen)
        assert HTML_HOST in hosts
        assert LITE_HOST in hosts

    async def test_blocked_primary_403_falls_back_to_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            if host == HTML_HOST:
                return httpx.Response(403)
            if host == LITE_HOST:
                return httpx.Response(200, content=LITE_RESULTS_HTML)
            return httpx.Response(500)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.OK
        assert len(resp.results) == 2
        assert LITE_HOST in _hosts(seen)

    async def test_both_endpoints_challenged_report_blocked(self, adapter):
        def _wall(r):
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=CHALLENGE_HTML)

        async with MockHTTP(_wall):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.BLOCKED
        assert resp.results == []
        # Honest surfacing: the detail names both attempted endpoints.
        assert HTML_HOST in (resp.error_message or "")
        assert LITE_HOST in (resp.error_message or "")
        assert "blocked" in adapter.failure_classes

    async def test_rate_limited_short_circuits_without_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(429)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.RATE_LIMITED
        assert LITE_HOST not in _hosts(seen)

    async def test_timeout_does_not_fall_back(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            raise httpx.TimeoutException("timed out")

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.TIMEOUT
        assert resp.results == []
        assert LITE_HOST not in _hosts(seen)

    async def test_legitimate_no_results_marker_skips_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=NO_RESULTS_HTML)

        async with MockHTTP(_handler):
            resp = await adapter.search("zqxwv1234")

        assert resp.status == EngineStatus.OK
        assert resp.results == []
        assert LITE_HOST not in _hosts(seen)

    async def test_structurally_empty_primary_triggers_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            if host == HTML_HOST:
                return httpx.Response(200, content=EMPTY_HTML)
            if host == LITE_HOST:
                return httpx.Response(200, content=LITE_RESULTS_HTML)
            return httpx.Response(500)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.OK
        assert len(resp.results) == 2
        assert LITE_HOST in _hosts(seen)

    async def test_all_endpoints_structurally_empty_reports_error(self, adapter):
        def _handler(r):
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=EMPTY_HTML)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        # A walled/unusable session must NOT look like a healthy empty
        # result set — surface the failure via failure_classes instead.
        assert resp.status != EngineStatus.OK
        assert "blocked" in adapter.failure_classes
        assert resp.results == []

    # -- realistic session -------------------------------------------------

    async def test_headers_are_browser_realistic(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=HTML_RESULTS_HTML)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.OK
        post = next(r for r in seen if r.method == "POST")
        assert post.headers.get("user-agent", "").startswith("Mozilla/5.0")
        assert "Chrome" in post.headers.get("user-agent", "")
        assert post.headers.get("sec-fetch-mode") == "navigate"
        assert post.headers.get("sec-fetch-dest") == "document"
        # The search POST leaves the homepage host, so a real browser marks
        # it same-site (never same-origin).
        assert (post.headers.get("referer") or "").startswith("https://" + HOME_HOST + "/")
        assert post.headers.get("origin") == "https://" + HOME_HOST
        assert post.headers.get("sec-fetch-site") == "same-site"
        assert "text/html" in post.headers.get("accept", "")

    async def test_homepage_bootstrap_precedes_search_post(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=HTML_RESULTS_HTML)

        async with MockHTTP(_handler):
            await adapter.search("q")

        hosts = _hosts(seen)
        assert HOME_HOST in hosts
        search_idx = min(i for i, h in enumerate(hosts) if i > 0)
        home_idx = hosts.index(HOME_HOST)
        assert home_idx < search_idx

    async def test_bootstrap_failure_does_not_break_search(self, adapter):
        def _handler(r):
            host = r.url.host or ""
            if host == HOME_HOST:
                raise httpx.ConnectError("bootstrap refused")
            return httpx.Response(200, content=HTML_RESULTS_HTML)

        async with MockHTTP(_handler):
            resp = await adapter.search("q")

        assert resp.status == EngineStatus.OK
        assert len(resp.results) == 1

    # -- lite layout parser ------------------------------------------------

    def test_parse_lite_html_extracts_results(self, adapter):
        results = adapter._parse_lite_html(LITE_RESULTS_HTML.decode(), "query", 10)
        assert len(results) == 2
        assert results[0].url == "https://example.org/a"
        assert results[0].title == "Alpha Result"
        assert results[0].content == "Snippet for alpha."
        assert results[0].engine == "duckduckgo"
        assert results[0].position == 1
        assert results[1].url == "https://example.net/b"
        assert results[1].position == 2

    def test_parse_lite_html_respects_max_results(self, adapter):
        results = adapter._parse_lite_html(LITE_RESULTS_HTML.decode(), "query", 1)
        assert len(results) == 1

    def test_parse_lite_html_on_challenge_returns_empty(self, adapter):
        results = adapter._parse_lite_html(CHALLENGE_HTML.decode(), "query", 10)
        assert results == []

    def test_strip_ddg_redirect_unwraps_uddg_target(self, adapter):
        raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Fx&rut=rut1"
        assert adapter._strip_ddg_redirect(raw) == "https://example.org/x"
        direct = "https://direct.example.com/y"
        assert adapter._strip_ddg_redirect(direct) == direct

    # -- challenge indicators ------------------------------------------------

    def test_bot_wall_phrases_are_challenge_indicators(self, adapter):
        assert adapter._is_challenge_page("Unfortunately, bots use DuckDuckGo too.")
        assert adapter._is_challenge_page("Anomaly detected in your traffic pattern")
        assert not adapter._is_challenge_page(NO_RESULTS_HTML.decode())

    # -- image search path ---------------------------------------------------

    async def test_image_search_challenge_blocked_without_lite(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(r):
            seen.append(r)
            host = r.url.host or ""
            if host == HOME_HOST:
                return httpx.Response(200, content=HOME_HTML)
            return httpx.Response(200, content=CHALLENGE_HTML)

        async with MockHTTP(_handler):
            resp = await adapter.search("cats", {"categories": ["images"]})

        assert resp.status == EngineStatus.BLOCKED
        # lite.duckduckgo.com serves no tile layout for image searches.
        assert LITE_HOST not in _hosts(seen)
