"""Deterministic browser journeys for the server-rendered portal.

The regular test suite skips this module because browser binaries are not a
Python package dependency. CI's dedicated portal-browser job installs
Chromium and sets ``PORTAL_BROWSER_SMOKE=1`` to run the same journeys.
"""

from __future__ import annotations

import os
import socket
import threading
import time
from dataclasses import dataclass, field
from typing import Any

import pytest
import uvicorn

if os.environ.get("PORTAL_BROWSER_SMOKE") != "1":
    pytest.skip("portal browser smoke runs in its dedicated CI job", allow_module_level=True)

playwright = pytest.importorskip("playwright.sync_api")

import slopsearx.server as server_mod  # noqa: E402
from slopsearx.adapter import SearchResult  # noqa: E402
from slopsearx.capabilities import MCPPolicy  # noqa: E402
from slopsearx.formatter import format_html, format_landing_page  # noqa: E402
from slopsearx.mcp.harness import InMemoryStore  # noqa: E402
from slopsearx.portal_audit import PortalAuditLogger  # noqa: E402
from slopsearx.portal_auth import (  # noqa: E402
    LoginStart,
    PortalMembership,
    PortalPrincipal,
    PortalSessionStore,
    StaticMembershipResolver,
)
from slopsearx.workflow_console import (  # noqa: E402
    WorkflowNotFoundError,
    WorkflowPage,
)
from slopsearx.workflow_portal import WorkflowPortalRuntime  # noqa: E402


def test_landing_search_and_darker_toggle() -> None:
    """A browser can find the search control and switch the persisted mode."""
    with playwright.sync_playwright() as api:
        browser = api.chromium.launch()
        page = browser.new_page(viewport={"width": 390, "height": 844})
        page.set_content(format_landing_page(default_theme="dark"))

        assert page.get_by_role("heading", name="Search SlopSearX.").is_visible()
        assert page.get_by_role("searchbox", name="Search SlopSearX").is_visible()
        toggle = page.get_by_role("button", name="Use darker mode")
        toggle.click()
        assert page.locator("html").get_attribute("data-theme") == "darker"
        assert page.get_by_role("button", name="Use dark mode").is_visible()
        page.locator("#portal-query").blur()
        page.keyboard.press("/")
        assert page.locator("#portal-query").evaluate("element => document.activeElement === element")
        browser.close()


def test_results_journey_keeps_links_and_escaped_content() -> None:
    """Result rendering exposes source state and safe navigation in a browser."""
    result = SearchResult(
        url="https://example.com/result",
        title='A <script>alert("x")</script>',
        content="A useful result",
        engine="mocktest",
        engines={"mocktest", "wikipedia"},
        category="science",
    )
    state = {
        "query": "valkey",
        "categories": "general",
        "language": "en",
        "page": 2,
        "selected_engine_count": 1,
        "responsive_engine_count": 1,
        "category_options": ["general"],
        "scope_label": "general",
    }

    with playwright.sync_playwright() as api:
        browser = api.chromium.launch()
        page = browser.new_page(viewport={"width": 1440, "height": 900})
        page.set_content(format_html([result], "valkey", portal_state=state))

        assert page.get_by_role("heading", name='A <script>alert("x")</script>').is_visible()
        assert page.get_by_text("Matched 2 sources").is_visible()
        assert page.get_by_text("Research").is_visible()
        assert page.get_by_role("article").get_by_text("Wikipedia").first.is_visible()
        assert page.get_by_role("link", name="Open JSON view ↗", exact=True).is_visible()
        assert page.get_by_role("complementary", name="Search summary").is_visible()
        assert page.get_by_role("link", name="Previous result page").get_attribute("href")
        assert page.get_by_role("link", name="Try next result page").get_attribute("href")
        assert page.get_by_role("combobox", name="Scope").is_visible()
        assert page.get_by_role("button", name="Apply filters").is_visible()
        assert page.locator("script").count() == 1
        assert page.locator(".result-link").get_attribute("href") == "https://example.com/result"
        explanation = page.get_by_role("group").filter(has_text="Why this result appeared")
        explanation.get_by_text("Why this result appeared").click()
        assert explanation.get_by_text("Cross-source presence").is_visible()
        assert explanation.get_by_text("structurally eligible").is_visible()
        page.set_viewport_size({"width": 390, "height": 844})
        assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")
        browser.close()


@dataclass
class BrowserIdentity:
    origin: str
    completed: bool = False

    async def begin(self, _return_to: str) -> LoginStart:
        return LoginStart(f"{self.origin}/auth/callback?code=good", "browser-transaction")

    async def complete(self, request_url: str, transaction_handle: str | None) -> tuple[str, str, str]:
        assert request_url == f"{self.origin}/auth/callback?code=good"
        assert transaction_handle == "browser-transaction"
        self.completed = True
        return "https://id.example", "browser-subject", "/workflows"


@dataclass
class BrowserWorkflows:
    calls: int = 0
    policy: MCPPolicy = field(default_factory=MCPPolicy)

    async def list(self, _context: Any, _cursor: str | None) -> WorkflowPage:
        return WorkflowPage(
            [
                {
                    "kind": "research",
                    "id": "job-1",
                    "state": "partial",
                    "created_at": 1.0,
                    "expires_at": 2.0,
                    "revision": 3,
                    "summary": '<script>alert("stored")</script>',
                }
            ],
            None,
        )

    async def detail(self, _context: Any, kind: str, object_id: str) -> dict[str, Any]:
        if object_id == "foreign":
            raise WorkflowNotFoundError
        return {
            "kind": kind,
            "id": object_id,
            "state": "partial",
            "revision": 3,
            "summary": "Two subquestions",
            "details": {"lineage": [{"unsafe": "<img src=x>"}]},
        }

    async def cancel_research(self, _context: Any, _object_id: str, revision: int) -> str:
        assert revision == 3
        self.calls += 1
        return "cancelled"


def test_workflow_console_real_server_browser_journey() -> None:
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    port = listener.getsockname()[1]
    origin = f"http://localhost:{port}"
    now = [1000.0]
    principal = PortalPrincipal(
        "browser-principal",
        "https://id.example",
        "browser-subject",
        (
            PortalMembership("tenant-a", "Tenant A", frozenset({"workflow.read", "workflow.research.cancel"})),
            PortalMembership("tenant-b", "Tenant B", frozenset({"workflow.read", "workflow.research.cancel"})),
        ),
    )
    directory = StaticMembershipResolver((principal,))
    store = InMemoryStore()
    counter = iter(range(100))
    sessions = PortalSessionStore(
        store,
        directory,
        hmac_key=b"b" * 32,
        clock=lambda: now[0],
        idle_seconds=60,
        absolute_seconds=120,
        token_source=lambda _size: f"browser-token-{next(counter):03d}-abcdefghijklmnopqrstuvwxyz",
    )
    identity = BrowserIdentity(origin)
    workflows = BrowserWorkflows()
    server_mod.configure_workflow_portal(
        WorkflowPortalRuntime(
            identity=identity,
            memberships=directory,
            sessions=sessions,
            workflows=workflows,  # type: ignore[arg-type]
            store=store,
            external_origin=origin,
            operator_actions=frozenset({"workflow.read", "workflow.research.cancel"}),
            mutations_enabled=frozenset({"workflow.research.cancel"}),
            secure_cookie=True,
            audit=PortalAuditLogger(store, pseudonym_key=b"a" * 32),
        )
    )
    server = uvicorn.Server(uvicorn.Config(server_mod.app, host="127.0.0.1", port=port, log_level="error"))
    thread = threading.Thread(target=lambda: server.run(sockets=[listener]), daemon=True)
    thread.start()
    try:
        deadline = time.monotonic() + 10
        while not server.started and time.monotonic() < deadline:
            time.sleep(0.01)
        assert server.started
        with playwright.sync_playwright() as api:
            browser = api.chromium.launch()
            page = browser.new_page(viewport={"width": 320, "height": 640})
            page.goto(f"{origin}/workflows")
            assert page.get_by_role("heading", name="Choose workspace").is_visible()
            assert identity.completed
            session_cookie = next(
                cookie for cookie in page.context.cookies() if cookie["name"] == "__Host-slopsearx_session"
            )
            assert session_cookie["secure"] and session_cookie["path"] == "/"
            assert all(cookie["name"] != "__Secure-slopsearx_login" for cookie in page.context.cookies())
            page.get_by_role("button", name="Tenant B").click()
            assert page.get_by_role("heading", name="Workflows").is_visible()
            assert page.get_by_text("Tenant B").is_visible()
            assert page.locator("script").count() == 0
            assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")
            page.keyboard.press("Tab")
            assert page.get_by_role("link", name="SlopSearX").evaluate("el => document.activeElement === el")
            page.get_by_role("link", name="Inspect workflow").click()
            assert page.locator(".mutations .notice").count() == 1
            assert page.get_by_text("Submit to confirm.").is_visible()
            assert "<img src=x>" not in page.content()
            csrf_status = page.evaluate(
                """async () => {
                    const form = document.querySelector('form[action$="/cancel"]');
                    const body = new URLSearchParams(new FormData(form));
                    body.set('csrf', 'bad');
                    return (await fetch(form.action, {method: 'POST', body})).status;
                }"""
            )
            assert csrf_status == 403 and workflows.calls == 0
            response = page.goto(f"{origin}/workflows/research/foreign")
            assert response is not None and response.status == 404
            assert page.get_by_role("heading", name="Not found or unavailable").is_visible()
            page.goto(f"{origin}/workflows/research/job-1")
            now[0] = 1061.0
            page.get_by_role("button", name="Cancel queued work").click()
            assert page.get_by_role("heading", name="Choose workspace").is_visible()
            assert workflows.calls == 0
            browser.close()
    finally:
        server.should_exit = True
        thread.join(timeout=10)
        listener.close()
        server_mod.configure_workflow_portal(None)
