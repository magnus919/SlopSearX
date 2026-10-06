"""Tests for the Bing HTML scrape adapter (issue 674)."""

from __future__ import annotations

import base64

import httpx
import pytest

import engines  # noqa: F401
from engines.bing import decode_bing_url
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP


def _ck(url: str) -> str:
    """A Bing click-tracking link the way the results page serves it."""
    token = base64.urlsafe_b64encode(url.encode()).decode().rstrip("=")
    return f"https://www.bing.com/ck/a?!&amp;&amp;p=abc123&amp;ptn=3&amp;ver=2&amp;u=a1{token}&amp;ntb=1"


SAMPLE_HTML = f"""
<html><body><ol id="b_results">
<li class="b_algo">
  <h2><a href="{_ck("https://kubernetes.io/releases/1.34/")}">Kubernetes 1.34 | Kubernetes</a></h2>
  <div class="b_caption"><p class="b_lineclamp2">This page provides an overview of the 1.34 release.</p></div>
</li>
<li class="b_algo">
  <h2><a href="https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/">Announcing Rust 1.90.0</a></h2>
  <div class="b_caption"><p>If you have a previous version of Rust installed via rustup.</p></div>
</li>
<li class="b_algo">
  <h2><a href="https://www.bing.com/images/search?q=x">Images for x</a></h2>
</li>
<li class="b_algo">
  <h2><a href="{_ck("https://kubernetes.io/releases/1.34/")}">Duplicate</a></h2>
</li>
<li class="b_ad"><h2><a href="https://ads.example.com/">Sponsored</a></h2></li>
</ol>
<script>var challenge = 1;</script>
</body></html>
"""

CAPTCHA_HTML = "<html><body><div id='b_captcha'>Please solve the challenge below to continue</div></body></html>"


@pytest.fixture
def adapter():
    return discover_engines({"bing": {"enabled": True}})["bing"]


class TestBingRegistration:
    def test_registered_as_general_scrape_engine(self):
        cls = list_engines()["bing"]
        assert cls.engine_type == "scrape"
        assert "general" in cls.categories

    def test_in_default_config_and_tier1_fallback(self):
        from slopsearx.config import load_config
        from slopsearx.service import DEFAULT_TIER1_ENGINES

        assert load_config("/nonexistent/slopsearx.yaml").engines["bing"].enabled
        assert "bing" in DEFAULT_TIER1_ENGINES


class TestDecodeBingUrl:
    def test_click_tracking_link_decodes_to_target(self):
        href = _ck("https://example.com/a?b=1&c=2").replace("&amp;", "&")
        assert decode_bing_url(href) == "https://example.com/a?b=1&c=2"

    def test_direct_link_is_kept(self):
        assert decode_bing_url("https://example.com/page") == "https://example.com/page"

    @pytest.mark.parametrize(
        "href",
        [
            "https://www.bing.com/images/search?q=x",
            "https://www.bing.com/ck/a?!&p=abc&u=zz-not-a1",
            "https://www.bing.com/ck/a?!&p=abc&u=a1%%%",
            "javascript:void(0)",
            "/search?q=more",
            "",
        ],
    )
    def test_internal_or_broken_links_are_dropped(self, href):
        assert decode_bing_url(href) is None


class TestBingSearch:
    async def test_parses_organic_results_with_real_urls(self, adapter):
        seen: list[httpx.Request] = []

        def _handler(request: httpx.Request) -> httpx.Response:
            seen.append(request)
            return httpx.Response(200, text=SAMPLE_HTML)

        async with MockHTTP(_handler):
            result = await adapter.search("kubernetes 1.34")

        assert result.status is EngineStatus.OK
        assert [r.url for r in result.results] == [
            "https://kubernetes.io/releases/1.34/",
            "https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/",
        ]
        first = result.results[0]
        assert first.title == "Kubernetes 1.34 | Kubernetes"
        assert first.content == "This page provides an overview of the 1.34 release."
        assert first.engine == "bing"
        assert [r.position for r in result.results] == [1, 2]
        assert seen[0].url.host == "www.bing.com"
        assert seen[0].url.params["q"] == "kubernetes 1.34"
        assert seen[0].headers["User-Agent"].startswith("Mozilla/5.0")

    async def test_respects_max_results(self):
        adapter = discover_engines({"bing": {"enabled": True, "max_results": 1}})["bing"]

        async with MockHTTP(lambda r: httpx.Response(200, text=SAMPLE_HTML)):
            result = await adapter.search("q")

        assert len(result.results) == 1

    async def test_captcha_page_is_blocked(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, text=CAPTCHA_HTML)):
            result = await adapter.search("q")

        assert result.status is EngineStatus.BLOCKED
        assert result.results == []

    async def test_empty_page_is_ok_with_no_results(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, text="<html><body></body></html>")):
            result = await adapter.search("q")

        assert result.status is EngineStatus.OK
        assert result.results == []

    @pytest.mark.parametrize(("code", "status"), [(429, EngineStatus.RATE_LIMITED), (403, EngineStatus.BLOCKED)])
    async def test_http_refusals_are_classified(self, adapter, code, status):
        async with MockHTTP(lambda r: httpx.Response(code, text="nope")):
            result = await adapter.search("q")

        assert result.status is status

    async def test_server_error_is_error_not_exception(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(500, text="boom")):
            result = await adapter.search("q")

        assert result.status is EngineStatus.ERROR

    async def test_timeout_is_classified(self, adapter):
        def _handler(request: httpx.Request) -> httpx.Response:
            raise httpx.ReadTimeout("slow", request=request)

        async with MockHTTP(_handler):
            result = await adapter.search("q")

        assert result.status is EngineStatus.TIMEOUT


class TestBingRouting:
    @pytest.mark.parametrize(
        "query",
        [
            "Kubernetes 1.34 release date",  # code
            "quantum error correction paper",  # science
            "latest news on rust",  # news
            "reddit discussion about tabs",  # social
            "how to configure nginx",  # reference
        ],
    )
    def test_topic_routes_include_bing_beside_the_other_general_scrapers(self, query):
        from slopsearx.router import QueryRouter

        routed = QueryRouter().route(query)
        assert routed is not None
        assert "bing" in routed

    def test_every_default_topic_that_uses_duckduckgo_also_uses_bing(self):
        from slopsearx.router import _DEFAULT_FALLBACK, _DEFAULT_TOPICS

        for topic in _DEFAULT_TOPICS:
            if "duckduckgo" in topic["engines"]:
                assert "bing" in topic["engines"], topic["name"]
        assert "bing" in _DEFAULT_FALLBACK
