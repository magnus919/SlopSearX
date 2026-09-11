"""Deterministic browser journeys for the server-rendered portal.

The regular test suite skips this module because browser binaries are not a
Python package dependency. CI's dedicated portal-browser job installs
Chromium and sets ``PORTAL_BROWSER_SMOKE=1`` to run the same journeys.
"""

from __future__ import annotations

import os

import pytest

if os.environ.get("PORTAL_BROWSER_SMOKE") != "1":
    pytest.skip("portal browser smoke runs in its dedicated CI job", allow_module_level=True)

playwright = pytest.importorskip("playwright.sync_api")

from slopsearx.adapter import SearchResult  # noqa: E402
from slopsearx.formatter import format_html, format_landing_page  # noqa: E402


def test_landing_search_and_darker_toggle() -> None:
    """A browser can find the search control and switch the persisted mode."""
    with playwright.sync_playwright() as api:
        browser = api.chromium.launch()
        page = browser.new_page(viewport={"width": 390, "height": 844})
        page.set_content(format_landing_page(default_theme="dark"))

        assert page.get_by_role("heading", name="Search SlopSearX.").is_visible()
        assert page.get_by_role("searchbox", name="Search SlopSearX").is_visible()
        toggle = page.get_by_role("button", name="Darker · off")
        toggle.click()
        assert page.locator("html").get_attribute("data-theme") == "darker"
        assert page.get_by_role("button", name="Darker · on").is_visible()
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
        assert page.get_by_role("article").get_by_text("Wikipedia").is_visible()
        assert page.get_by_role("link", name="JSON view ↗").is_visible()
        assert page.get_by_role("complementary", name="Search summary").is_visible()
        assert page.get_by_role("link", name="← Previous").get_attribute("href")
        assert page.get_by_role("link", name="Next →").get_attribute("href")
        page.get_by_text("Scope and filters").click()
        assert page.get_by_role("combobox", name="Scope").is_visible()
        assert page.get_by_role("button", name="Apply filters").is_visible()
        assert page.locator("script").count() == 1
        assert page.locator(".result-link").get_attribute("href") == "https://example.com/result"
        browser.close()
