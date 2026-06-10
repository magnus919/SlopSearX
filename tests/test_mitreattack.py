"""Tests for MITRE ATT&CK adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestMitreAttackAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"mitreattack": {"enabled": True}})["mitreattack"]

    @pytest.fixture
    def technique_html(self) -> str:
        return """
        <html><body>
        <h1>T1059 Command and Scripting Interpreter</h1>
        <div class="description-body">
          <p>Adversaries may abuse command and script interpreters to execute commands, scripts, or binaries.</p>
        </div>
        <div class="tactic-badge">Execution</div>
        </body></html>
        """

    async def test_technique_id_lookup(self, adapter, technique_html):
        async with MockHTTP(lambda r: httpx.Response(200, content=technique_html.encode())):
            result = await adapter.search("T1059")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "T1059" in result.results[0].title
        assert "command and script" in result.results[0].content.lower()
        assert "Execution" in result.results[0].content

    async def test_group_id_lookup(self, adapter):
        html = (
            "<html><body><h1>APT29</h1>"
            "<div class='description-body'><p>Russian state-sponsored threat actor.</p></div>"
            "</body></html>"
        )
        async with MockHTTP(lambda r: httpx.Response(200, content=html.encode())):
            result = await adapter.search("G0016")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "APT29" in result.results[0].title

    async def test_software_id_lookup(self, adapter):
        html = (
            "<html><body><h1>Cobalt Strike</h1>"
            "<div class='description-body'><p>Commercial adversary simulation software.</p></div>"
            "</body></html>"
        )
        async with MockHTTP(lambda r: httpx.Response(200, content=html.encode())):
            result = await adapter.search("S0154")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Cobalt Strike" in result.results[0].title

    async def test_keyword_search_returns_empty(self, adapter):
        """MITRE ATT&CK only supports direct ID lookups (T/G/S patterns).

        Free-text search is not supported by the upstream service, so
        non-ID queries should return empty results gracefully.
        """
        async with MockHTTP(lambda r: httpx.Response(200, content=b"")):
            result = await adapter.search("command execution")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("T1059")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "mitreattack" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        assert "reference" in list_engines()["mitreattack"].categories
