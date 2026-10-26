"""Black-box browser checks for the actual native T02 HTTP HTML surface.

This is a wire-only client: CI starts the Rust fixture and supplies its URL. The
module never imports the Python backend, launches the fixture, or contacts a
provider.
"""

from __future__ import annotations

import os
from typing import Any
from urllib.parse import urlsplit

import pytest

if os.environ.get("PORTAL_NATIVE_HTML_THEME_SMOKE") == "1":
    try:
        from playwright.sync_api import sync_playwright
    except ImportError as error:
        raise RuntimeError("the dedicated native browser gate requires Playwright") from error
else:
    playwright = pytest.importorskip("playwright.sync_api")
    sync_playwright = playwright.sync_playwright

DARK_BACKGROUND = "rgb(25, 26, 24)"
DARKER_BACKGROUND = "rgb(11, 12, 11)"


def _check_mode(page: Any, mode: str) -> None:
    darker = mode == "darker"
    label = "Dark mode" if darker else "Darker mode"
    accessible_name = "Use dark mode" if darker else "Use darker mode"
    background = DARKER_BACKGROUND if darker else DARK_BACKGROUND
    assert page.locator("html").get_attribute("data-theme") == mode
    button = page.get_by_role("button", name=accessible_name)
    assert button.inner_text() == label
    assert button.get_attribute("aria-pressed") == str(darker).lower()
    assert page.locator("body").evaluate("element => getComputedStyle(element).backgroundColor") == background


def _open_page(browser: Any, base_url: str, init_script: str | None = None) -> tuple[Any, Any]:
    parsed = urlsplit(base_url)
    assert parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost", "::1"}
    assert parsed.port is not None and parsed.path in {"", "/"}
    origin = f"{parsed.scheme}://{parsed.netloc}"
    context = browser.new_context(viewport={"width": 390, "height": 844})

    def restrict_requests(route: Any) -> None:
        request = urlsplit(route.request.url)
        request_origin = f"{request.scheme}://{request.netloc}"
        if request_origin == origin:
            route.continue_()
        else:
            route.abort()

    context.route("**/*", restrict_requests)
    if init_script is not None:
        context.add_init_script(init_script)
    page = context.new_page()
    page.set_default_timeout(5000)
    response = page.goto(base_url, wait_until="load")
    assert response is not None and response.status == 200
    return context, page


@pytest.mark.skipif(
    os.environ.get("PORTAL_NATIVE_HTML_THEME_SMOKE") != "1",
    reason="native HTML browser smoke runs in its dedicated CI gate",
)
def test_native_theme_storage_accessibility_and_painted_background() -> None:
    base_url = os.environ.get("PORTAL_NATIVE_HTML_BASE_URL")
    default_theme = os.environ.get("PORTAL_NATIVE_HTML_DEFAULT_THEME")
    assert base_url, "CI must point this client at the running native Rust HTTP fixture"
    assert default_theme in {"dark", "darker"}

    with sync_playwright() as api:
        browser = api.chromium.launch()
        try:
            context, page = _open_page(browser, base_url)
            try:
                assert page.locator("html").get_attribute("data-default-theme") == default_theme
                _check_mode(page, default_theme)
                next_theme = "dark" if default_theme == "darker" else "darker"
                page.get_by_role(
                    "button",
                    name="Use dark mode" if default_theme == "darker" else "Use darker mode",
                ).click()
                _check_mode(page, next_theme)
                assert page.evaluate("window.localStorage.getItem('slopsearx-theme')") == next_theme
                page.reload(wait_until="load")
                _check_mode(page, next_theme)
            finally:
                context.close()

            override = "dark" if default_theme == "darker" else "darker"
            context, page = _open_page(
                browser,
                base_url,
                f"window.localStorage.setItem('slopsearx-theme', '{override}')",
            )
            try:
                _check_mode(page, override)
            finally:
                context.close()

            context, page = _open_page(
                browser,
                base_url,
                "window.localStorage.setItem('slopsearx-theme', 'light')",
            )
            try:
                _check_mode(page, default_theme)
            finally:
                context.close()

            context, page = _open_page(
                browser,
                base_url,
                "Object.defineProperty(window, 'localStorage', "
                "{configurable: true, get() { throw new Error('blocked') }})",
            )
            try:
                _check_mode(page, default_theme)
                page.get_by_role(
                    "button",
                    name="Use dark mode" if default_theme == "darker" else "Use darker mode",
                ).click()
                _check_mode(page, next_theme)
            finally:
                context.close()
        finally:
            browser.close()
