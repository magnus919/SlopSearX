"""Tests for the search-to-retrieval handoff boundary (issue 189).

Covers the machine-readable ``retrieval`` handoff record exposed on MCP
cards and expanded records: eligibility classification for missing, unsafe,
non-HTTP, and canonicalization-ambiguous URLs; the stable linkage that lets a
downstream retriever (e.g. GroktoCrawl) associate a capture with the
originating result and snapshot without parsing prose; and the contract
fixtures showing search -> handoff -> downstream capture provenance.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import SearchService
from slopsearx.snapshot import SnapshotStore

FIXTURES = Path(__file__).parent / "fixtures" / "retrieval_handoff"

LONG_CONTENT = "Word " * 200  # > SNIPPET_LENGTH (300)
SHORT_CONTENT = "Short snippet."

# The closed url_status vocabulary documented in docs/RETRIEVAL_HANDOFF.md.
URL_STATUS_VOCABULARY = {"ok", "missing", "non_http", "unsafe_scheme", "ambiguous"}


class _FakeStore:
    """In-memory key-value store (SearchCache-like)."""

    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _UrlEngine(EngineAdapter):
    """One result per configured URL, with optional content/provenance."""

    def __init__(
        self,
        name: str,
        urls: list[str],
        *,
        content: str = "Content.",
        contrib_engines: set[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._urls = urls
        self._content = content
        self._engines = contrib_engines
        self.categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        return AdapterResponse(
            results=[
                SearchResult(
                    url=url,
                    title=f"result {i}",
                    content=self._content,
                    engine=self.name,
                    engines=set(self._engines or {self.name}),
                    score=1.0,
                    position=i + 1,
                    category="general",
                    tier=1,
                )
                for i, url in enumerate(self._urls)
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
        )


def _build_state(engine_map: dict[str, EngineAdapter], *, policy: MCPPolicy | None = None) -> McpState:
    from slopsearx.service import AppContext

    policy = policy or load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines=engine_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_map),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
    store = _FakeStore()
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )


@pytest.fixture
def state() -> McpState:
    state_obj = _build_state(
        {
            "brave": _UrlEngine(
                "brave",
                ["https://example.com/0", "https://example.com/1"],
                content=LONG_CONTENT,
            ),
        }
    )
    set_state(state_obj)
    yield state_obj
    set_state(None)


def _first_card(result: dict[str, Any]) -> dict[str, Any]:
    return result["results"][0]


# ---------------------------------------------------------------------------
# The handoff record on cards and expanded records
# ---------------------------------------------------------------------------


class TestHandoffRecord:
    async def test_card_exposes_compact_retrieval_eligibility(self, state: McpState) -> None:
        """Cards carry a compact eligibility block; no fetch target is invented."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        retrieval = card["retrieval"]
        assert set(retrieval) == {"contract", "version", "eligible", "url_status", "url_reason", "scheme"}
        assert retrieval["contract"] == t.RETRIEVAL_HANDOFF_CONTRACT
        assert retrieval["version"] == t.RETRIEVAL_HANDOFF_VERSION
        assert retrieval["eligible"] is True
        assert retrieval["url_status"] == "ok"
        assert retrieval["url_reason"] is None
        assert retrieval["scheme"] == "https"

    async def test_record_exposes_stable_handoff_record(self, state: McpState) -> None:
        """The expanded record carries the full, self-contained handoff record."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        expanded = await t.slopsearx_read_result(card["result_id"])
        handoff = expanded["retrieval"]
        assert set(handoff) == {
            "contract",
            "version",
            "result_id",
            "url",
            "url_status",
            "url_reason",
            "scheme",
            "eligible",
            "snippet_only",
            "verified",
            "verification_note",
            "provenance",
        }
        assert handoff["contract"] == t.RETRIEVAL_HANDOFF_CONTRACT
        assert handoff["version"] == t.RETRIEVAL_HANDOFF_VERSION
        assert handoff["result_id"] == card["result_id"]
        assert handoff["url"] == card["url"]
        assert handoff["eligible"] is True
        assert handoff["snippet_only"] is False  # LONG_CONTENT exceeds the snippet bound
        assert handoff["verified"] is False
        assert handoff["verification_note"] == "SlopSearX did not fetch or verify the linked page"
        assert handoff["provenance"]["source_engines"] == card["source_engines"]

    async def test_snippet_only_tracks_content_availability(self, state: McpState) -> None:
        """snippet_only is True exactly when the record reports content unavailable."""
        short = _build_state({"brave": _UrlEngine("brave", ["https://example.com/x"], content=SHORT_CONTENT)})
        set_state(short)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert expanded["retrieval"]["snippet_only"] is True
            assert expanded["content_available"] is False
        finally:
            set_state(None)

    async def test_handoff_linkage_survives_cards_expansion_and_snapshot_reads(self, state: McpState) -> None:
        """Provenance survives compact cards, expansion, and snapshot reads.

        A downstream retriever can associate a capture with the originating
        result and snapshot without parsing prose: the handoff's result_id,
        url, and provenance match the card, the expanded record, and the
        paginated snapshot cards.
        """
        result = await t.slopsearx_search("some query", max_results=1)
        cursor = result["meta"]["cursor"]
        assert cursor is not None
        card = _first_card(result)

        expanded = await t.slopsearx_read_result(card["result_id"])
        page = await t.slopsearx_read_results(cursor, page=1, max_results=1)

        handoff = expanded["retrieval"]
        assert handoff["result_id"] == card["result_id"] == page["results"][0]["result_id"]
        assert handoff["url"] == card["url"] == page["results"][0]["url"]
        assert handoff["provenance"]["snapshot_cursor"] == cursor
        assert handoff["provenance"]["query_id"] == result["meta"]["query_id"]
        assert handoff["provenance"]["query"] == "some query"
        assert handoff["provenance"]["source_engines"] == card["source_engines"]
        assert handoff["eligible"] is True


# ---------------------------------------------------------------------------
# URL classification: missing / non-HTTP / unsafe / ambiguous
# ---------------------------------------------------------------------------


class TestUrlClassification:
    @pytest.mark.parametrize(
        ("url", "expected_status", "expected_scheme"),
        [
            ("https://example.com/page", "ok", "https"),
            ("http://example.com/page", "ok", "http"),
            # A literal public IP host is still eligible — only non-global
            # literal IPs (loopback, private, CGNAT, link-local, reserved,
            # documentation, unspecified) are blocked.
            ("http://8.8.8.8/", "ok", "http"),
            ("", "missing", None),
            ("   ", "missing", None),
            ("mailto:someone@example.com", "non_http", "mailto"),
            ("tel:+15551234567", "non_http", "tel"),
            ("file:///etc/passwd", "unsafe_scheme", "file"),
            ("data:text/plain;base64,SGVsbG8=", "unsafe_scheme", "data"),
            ("javascript:alert(1)", "unsafe_scheme", "javascript"),
            ("gopher://example.com/1", "unsafe_scheme", "gopher"),
            ("/relative/path", "ambiguous", None),
            ("https://", "ambiguous", "https"),
            ("https://[::1", "ambiguous", None),
            # Backslash-authority host confusion (CWE-918): urlparse reports
            # host "public.com" but a WHATWG client connects to
            # "internal.example"; never handed off.
            ("https://internal.example\\@public.com/", "ambiguous", "https"),
            # Invalid / out-of-range / zero ports are not fetchable targets.
            ("http://host:abc/", "ambiguous", "http"),
            ("http://host:99999/", "ambiguous", "http"),
            ("http://host:0/", "ambiguous", "http"),
        ],
    )
    async def test_url_status_classification(
        self,
        url: str,
        expected_status: str,
        expected_scheme: str | None,
    ) -> None:
        """Each URL class maps to the documented machine-readable status token."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            assert card["retrieval"]["url_status"] == expected_status
            assert card["retrieval"]["scheme"] == expected_scheme
            assert card["retrieval"]["eligible"] == (expected_status == "ok")
            if expected_status == "ok":
                assert card["retrieval"]["url_reason"] is None
            else:
                assert card["retrieval"]["url_reason"]
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        ("url", "expected_status", "expected_reason"),
        [
            ("", "missing", "result has no URL to retrieve"),
            ("mailto:someone@example.com", "non_http", "scheme 'mailto' is not HTTP(S); not retrievable over HTTP"),
            ("file:///etc/passwd", "unsafe_scheme", "scheme 'file' is unsafe for downstream HTTP retrieval"),
            ("/relative/path", "ambiguous", "URL has no scheme; canonicalization is ambiguous"),
            ("https://", "ambiguous", "URL has no host; canonicalization is ambiguous"),
            # Exact reason strings for the remaining ambiguous branches (pinned
            # contract strings — see _retrieval_url): backslash authority,
            # invalid port, out-of-range port, zero port, unparseable.
            (
                "https://internal.example\\@public.com/",
                "ambiguous",
                "URL authority contains a backslash, whitespace, or control character; canonicalization is ambiguous",
            ),
            ("http://host:abc/", "ambiguous", "URL port is invalid; canonicalization is ambiguous"),
            # 99999 parses as a port out of Python's uint16 range, so .port
            # itself raises ValueError — the "invalid port" branch, not the
            # sane-range branch (only port 0 reaches the 1-65535 check).
            ("http://host:99999/", "ambiguous", "URL port is invalid; canonicalization is ambiguous"),
            (
                "http://host:0/",
                "ambiguous",
                "URL port is outside the sane range (1-65535); canonicalization is ambiguous",
            ),
            ("https://[::1", "ambiguous", "URL cannot be parsed unambiguously"),
        ],
    )
    async def test_url_reasons_are_machine_readable(
        self,
        url: str,
        expected_status: str,
        expected_reason: str,
    ) -> None:
        """Ineligible URLs carry an explicit, stable reason on card and record."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == expected_status
            assert card["retrieval"]["url_reason"] == expected_reason
            assert expanded["retrieval"]["url_status"] == expected_status
            assert expanded["retrieval"]["url_reason"] == expected_reason
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            # Percent-encoded hosts: Python's urlparse does not percent-decode
            # the host, but a WHATWG client does, so "%31%36%39.%32%35%34..."
            # resolves to 169.254.169.254 after handoff (metadata-IP SSRF).
            "http://%31%32%37.0.0.1/",
            "http://%31%36%39.%32%35%34.%31%36%39.%32%35%34/",
            # Fullwidth host: urlparse does not IDNA-map, but WHATWG clients
            # do; the effective host is not the one reported here.
            "http://\uff45\uff58\uff41\uff4d\uff50\uff4c\uff45.example/",
        ],
    )
    async def test_percent_encoded_or_non_ascii_host_is_never_handed_off(self, url: str) -> None:
        """A host that cannot be canonicalized unambiguously is never handed off.

        Python's ``urlparse`` neither percent-decodes nor IDNA-maps the host,
        but WHATWG clients (browsers, HTTP stacks) do, so a percent-encoded
        metadata IP (``%31%32%37.0.0.1`` -> 127.0.0.1) or a fullwidth host
        would otherwise be certified ``ok`` and handed off verbatim. Such
        hosts are classified ``ambiguous`` and never handed off as a fetch
        target (CWE-918 host-confusion guard).
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL host contains a percent-encoded or non-ASCII character; canonicalization is ambiguous"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            "http://127.0.0.1/",
            "http://[::1]/",
            "http://10.0.0.1/",
            "http://169.254.169.254/latest/meta-data/",
        ],
    )
    async def test_literal_loopback_private_link_local_reserved_ip_host_is_never_handed_off(self, url: str) -> None:
        """A literal SSRF-prone IP host is never certified ``ok`` or handed off.

        Loopback, private, link-local, and reserved literal IP hosts
        (including the cloud metadata IP) are classified ``ambiguous`` and
        the handoff URL is null, so a downstream retriever never fetches
        them. Only literal IPs are checked (via ``ipaddress`` and
        ``getaddrinfo`` with ``AI_NUMERICHOST`` — no DNS resolution), so
        hostnames still classify normally.
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL host is a non-global literal IP address; not a safe fetch target"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            # Numeric-IP-literal host encodings WHATWG clients resolve with no
            # DNS: decimal/hex integer forms, octal and abbreviated dotted
            # forms, a trailing dot, and a bare "0x" (Chromium -> 0.0.0.0).
            # ipaddress.ip_address rejects all of these, so the guard must
            # parse them the way a WHATWG client would.
            "http://2130706433/",  # decimal integer -> 127.0.0.1
            "http://127.1/",  # abbreviated dotted -> 127.0.0.1
            "http://0177.0.0.1/",  # octal component -> 127.0.0.1
            "http://0x7f000001/",  # hex integer -> 127.0.0.1
            "http://0x7f.1/",  # hex component + abbreviated -> 127.0.0.1
            "http://2852039166/",  # decimal integer -> 169.254.169.254
            "http://0xa9fea9fe/",  # hex integer -> 169.254.169.254
            "http://0x/",  # bare 0x -> 0.0.0.0 (unspecified)
            "http://127.0.0.1./",  # trailing dot -> 127.0.0.1
        ],
    )
    async def test_noncanonical_numeric_ip_literal_is_never_handed_off(self, url: str) -> None:
        """A numeric IP literal in any encoding is never certified ``ok``.

        ``http://2130706433/``, ``http://127.1/``, ``http://0177.0.0.1/``,
        and ``http://0x7f000001/`` all resolve to 127.0.0.1; the metadata
        forms ``http://2852039166/`` and ``http://0xa9fea9fe/`` resolve to
        169.254.169.254. WHATWG clients resolve these host encodings without
        DNS, so the guard must recognize every one and classify non-global
        results ``ambiguous`` (CWE-918 literal-IP SSRF guard).
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL host is a non-global literal IP address; not a safe fetch target"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            # Dotted all-numeric hosts that every literal-IP candidate parser
            # rejects (an empty octet, an out-of-range octet): WHATWG clients
            # treat them as failed IPv4 literal attempts, so they are not
            # legitimate hostnames. Without this branch they fell through to
            # ``ok`` and were handed off as fetch targets.
            "http://169..127.1/",
            "http://1.2.3.300/",
            "http://127.300.255.127/",
            "http://255.300.1/",
            "http://1.300.3/",
        ],
    )
    async def test_malformed_dotted_numeric_host_is_never_handed_off(self, url: str) -> None:
        """A dotted all-numeric host that fails every candidate parser is ambiguous.

        ``169..127.1``, ``1.2.3.300``, ``127.300.255.127``, ``255.300.1``,
        and ``1.300.3`` are digits+dots only, but none decodes as an IP
        literal (empty or out-of-range octet), so every candidate source
        (``ipaddress``, the WHATWG-style parser, ``getaddrinfo`` with
        ``AI_NUMERICHOST``) rejects them. They are not genuine hostnames and
        must never be certified ``ok`` — only a host containing at least one
        non-digit, non-dot character may reach ``ok``.
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL host is a malformed dotted numeric literal; canonicalization is ambiguous"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            # DNS-resolvable hostnames — including nip.io-style aliases of
            # loopback/link-local/metadata IPs and "localtest.me" — are
            # certified ``ok`` by design: the classifier performs literal-only
            # checks and NO DNS resolution, so it cannot see what a host
            # resolves to at fetch time. The downstream retriever owns
            # post-resolution SSRF controls (see docs/RETRIEVAL_HANDOFF.md §5
            # and §8). This test pins the documented boundary, not a gap.
            "https://169.254.169.254.nip.io/",
            "https://127.0.0.1.nip.io/",
            "https://localtest.me/",
        ],
    )
    async def test_dns_resolvable_hostnames_stay_ok_at_literal_boundary(self, url: str) -> None:
        """Hostnames pass the literal-only check; resolution is downstream."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ok"
            assert card["retrieval"]["eligible"] is True
            assert expanded["retrieval"]["url"] == url
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            # Numeric encodings of a public address stay eligible, mirroring
            # the dotted positive control ("http://8.8.8.8/" -> ok).
            "http://134744072/",  # decimal integer -> 8.8.8.8
            "http://0x08080808/",  # hex integer -> 8.8.8.8
        ],
    )
    async def test_global_numeric_ip_literal_still_eligible(self, url: str) -> None:
        """A numeric literal that resolves to a global IP is still ok."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            assert card["retrieval"]["url_status"] == "ok"
            assert card["retrieval"]["eligible"] is True
            assert card["retrieval"]["url_reason"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize("url", ["http://100.64.0.1/", "http://100.100.100.100/"])
    async def test_cgnat_ip_host_is_never_handed_off(self, url: str) -> None:
        """RFC 6598 CGNAT space (100.64.0.0/10) is not a safe fetch target.

        ``ipaddress`` marks these addresses as neither private nor reserved,
        but ``is_global`` is False, so a global-only test is required to
        reject them alongside loopback/private/link-local/reserved space.
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL host is a non-global literal IP address; not a safe fetch target"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        "url",
        [
            "http:// example.com/",  # leading space in the authority
            "http://exa mple.com/",  # embedded space in the host
            "http://example.com /path",  # space between authority and path
        ],
    )
    async def test_ascii_whitespace_in_authority_is_never_handed_off(self, url: str) -> None:
        """ASCII whitespace in the authority is never handed off.

        The authority guard rejects control characters but previously let a
        literal space (U+0020) through; a WHATWG client rejects or
        mis-canonicalizes these URLs, so they are classified ``ambiguous``.
        """
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL authority contains a backslash, whitespace, or control character; canonicalization is ambiguous"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    async def test_userinfo_credentials_are_never_handed_off(self) -> None:
        """A URL with embedded credentials is never certified ``ok`` or handed off.

        ``****************************/`` would otherwise pass every check
        and be returned verbatim as ``retrieval.url``, persisting the
        credentials and causing downstream Basic-auth transmission. Any
        userinfo in the authority classifies the URL ``ambiguous`` with a
        null handoff URL.
        """
        url = "http://user:pass@example.com/"
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == "ambiguous"
            assert card["retrieval"]["url_reason"] == (
                "URL authority contains userinfo credentials; not handed off as a fetch target"
            )
            assert card["retrieval"]["eligible"] is False
            assert expanded["retrieval"]["url"] is None
        finally:
            set_state(None)

    async def test_ineligible_url_is_never_handed_off_as_fetch_target(self, state: McpState) -> None:
        """missing/non-HTTP/unsafe/ambiguous URLs are not exposed via handoff.url.

        SlopSearX is not an SSRF-capable proxy: the handoff record only
        carries a fetch target when the URL is classified ``ok``.
        """
        state_obj = _build_state(
            {
                "brave": _UrlEngine(
                    "brave",
                    [
                        "",
                        "mailto:someone@example.com",
                        "tel:+15551234567",
                        "file:///etc/passwd",
                        "javascript:alert(1)",
                        "/relative/path",
                        "https://internal.example\\@public.com/",
                        "http://host:99999/",
                        "http://127.0.0.1/",
                        "http://[::1]/",
                        "http://10.0.0.1/",
                        "http://169.254.169.254/latest/meta-data/",
                        "http://user:pass@example.com/",
                    ],
                )
            }
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            for card in result["results"]:
                expanded = await t.slopsearx_read_result(card["result_id"])
                assert expanded["retrieval"]["eligible"] is False
                assert expanded["retrieval"]["url"] is None
                assert expanded["retrieval"]["url_status"] in {
                    "missing",
                    "non_http",
                    "unsafe_scheme",
                    "ambiguous",
                }
                assert expanded["retrieval"]["url_reason"]
        finally:
            set_state(None)

    async def test_ok_url_handed_off_verbatim(self, state: McpState) -> None:
        """An eligible URL is handed off verbatim, never rewritten or fetched."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        expanded = await t.slopsearx_read_result(card["result_id"])
        assert expanded["retrieval"]["url"] == "https://example.com/0"
        assert expanded["url"] == "https://example.com/0"

    def test_closed_url_status_vocabulary(self) -> None:
        """The runtime vocabulary matches the documented closed token set."""
        assert set(t.RETRIEVAL_URL_STATUSES) == URL_STATUS_VOCABULARY
        assert len(t.RETRIEVAL_URL_STATUSES) == len(set(t.RETRIEVAL_URL_STATUSES))
        assert "file" in t.UNSAFE_RETRIEVAL_SCHEMES
        assert "data" in t.UNSAFE_RETRIEVAL_SCHEMES
        assert "javascript" in t.UNSAFE_RETRIEVAL_SCHEMES


# ---------------------------------------------------------------------------
# Contract fixtures: search -> handoff -> downstream capture provenance
# ---------------------------------------------------------------------------


class TestContractFixtures:
    def test_search_handoff_capture_fixture_is_internally_consistent(self) -> None:
        """The composition fixture links the capture back without parsing prose."""
        data = json.loads((FIXTURES / "search_handoff_capture.json").read_text())

        handoff = data["handoff"]
        card = data["search"]["card"]
        capture_ref = data["capture"]["handoff_ref"]

        # The handoff record references the originating result identity.
        assert handoff["result_id"] == card["result_id"]
        assert handoff["url"] == card["url"]
        assert handoff["url_status"] == "ok"
        assert handoff["eligible"] is True
        assert handoff["verified"] is False
        assert handoff["provenance"]["snapshot_cursor"] == data["search"]["cursor"]
        assert handoff["provenance"]["query_id"] == data["search"]["query_id"]
        assert handoff["provenance"]["query"] == data["search"]["query"]

        # The downstream capture references the handoff provenance directly.
        assert capture_ref["contract"] == handoff["contract"]
        assert capture_ref["result_id"] == handoff["result_id"]
        assert capture_ref["url"] == handoff["url"]
        assert capture_ref["snapshot_cursor"] == handoff["provenance"]["snapshot_cursor"]
        assert capture_ref["query_id"] == handoff["provenance"]["query_id"]

    async def test_runtime_handoff_matches_ok_fixture_shape(self) -> None:
        """A live ok handoff matches the fixture's shape and values.

        The runtime state is arranged to be fixture-consistent: the same
        engine name (``nvd``), the same query text, the same result URL at the
        same snapshot index (2), and ``SHORT_CONTENT`` so ``snippet_only`` is
        true. This asserts url/result_id/snippet_only/provenance parity, not
        just the runtime-independent constants.
        """
        fixture = json.loads((FIXTURES / "search_handoff_capture.json").read_text())["handoff"]
        nvd_url = "https://nvd.nist.gov/vuln/detail/CVE-2024-1234"
        state_obj = _build_state(
            {
                "nvd": _UrlEngine(
                    "nvd",
                    ["https://example.com/0", "https://example.com/1", nvd_url],
                    content=SHORT_CONTENT,
                )
            }
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("CVE-2024-1234 impact analysis", engines=["nvd"])
            cursor = result["meta"]["cursor"]
            assert cursor is not None

            # The nvd result is the third adapter result, i.e. snapshot index
            # 2 — the same index suffix as the fixture's "snap-...:2" id.
            index = next(i for i, card in enumerate(result["results"]) if card["url"] == nvd_url)
            assert index == 2
            card = result["results"][index]
            expanded = await t.slopsearx_read_result(card["result_id"])
            handoff = expanded["retrieval"]

            assert set(handoff) == set(fixture)
            # Runtime-independent constants match the fixture exactly.
            assert handoff["contract"] == fixture["contract"]
            assert handoff["version"] == fixture["version"]
            assert handoff["url_status"] == fixture["url_status"]
            assert handoff["url_reason"] == fixture["url_reason"]
            assert handoff["scheme"] == fixture["scheme"]
            assert handoff["eligible"] == fixture["eligible"]
            assert handoff["verified"] == fixture["verified"]
            assert handoff["verification_note"] == fixture["verification_note"]

            # Verbatim url parity: the handoff target is the raw result URL
            # from the envelope — identical to the card and to the fixture —
            # never canonicalized or rewritten.
            assert handoff["url"] == card["url"] == fixture["url"]

            # Snippet-only parity: SHORT_CONTENT stays within the snippet
            # bound, so the runtime record reports the same snippet_only as
            # the fixture (true).
            assert handoff["snippet_only"] == fixture["snippet_only"] is True

            # Result identity parity: server-issued "<cursor>:<index>" with
            # the same index suffix (":2") as the fixture.
            assert handoff["result_id"] == card["result_id"] == f"{cursor}:{index}"
            assert handoff["result_id"].rsplit(":", 1)[1] == fixture["result_id"].rsplit(":", 1)[1]

            # Provenance parity: live cursor/query id/query text map onto the
            # fixture's provenance fields; source_engines match exactly.
            assert handoff["provenance"]["snapshot_cursor"] == cursor
            assert handoff["provenance"]["query_id"] == result["meta"]["query_id"]
            assert handoff["provenance"]["query"] == fixture["provenance"]["query"] == "CVE-2024-1234 impact analysis"
            assert handoff["provenance"]["source_engines"] == fixture["provenance"]["source_engines"] == ["nvd"]
            assert set(handoff["provenance"]) == set(fixture["provenance"])
        finally:
            set_state(None)

    async def test_runtime_unsafe_handoff_matches_fixture(self) -> None:
        """An unsafe URL at runtime produces exactly the fixture classification."""
        fixture = json.loads((FIXTURES / "ineligible_url_unsafe.json").read_text())
        state_obj = _build_state({"brave": _UrlEngine("brave", ["file:///etc/passwd"], content=SHORT_CONTENT)})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("local file disclosure", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            handoff = expanded["retrieval"]

            assert handoff["contract"] == fixture["contract"]
            assert handoff["url_status"] == fixture["url_status"]
            assert handoff["url_reason"] == fixture["url_reason"]
            assert handoff["scheme"] == fixture["scheme"]
            assert handoff["eligible"] == fixture["eligible"]
            assert handoff["url"] == fixture["url"]  # null — never handed off
            assert handoff["snippet_only"] == fixture["snippet_only"]
            assert handoff["verified"] == fixture["verified"]
            assert handoff["provenance"]["query"] == fixture["provenance"]["query"]
        finally:
            set_state(None)
