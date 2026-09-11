from __future__ import annotations

import hashlib
import hmac
from collections.abc import Generator
from dataclasses import dataclass, field
from typing import Any
from urllib.parse import urlsplit

import pytest
from fastapi.testclient import TestClient

import slopsearx.server as server_mod
from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_audit import PortalAuditLogger
from slopsearx.portal_auth import (
    LoginStart,
    PortalMembership,
    PortalPrincipal,
    PortalSessionStore,
    StaticMembershipResolver,
)
from slopsearx.workflow_console import WorkflowPage
from slopsearx.workflow_portal import WorkflowPortalRuntime, _page, _workflow_location


class FakeIdentity:
    async def begin(self, return_to: str) -> LoginStart:
        del return_to
        return LoginStart("https://id.example/authorize?state=fixed", "transaction")

    async def complete(self, request_url: str, transaction_handle: str | None) -> tuple[str, str, str]:
        assert "code=good" in request_url
        assert transaction_handle == "transaction"
        return "https://id.example", "subject", "/workflows"


@dataclass
class RecordingRateLimiter:
    calls: list[tuple[str, str]] = field(default_factory=list)

    async def allow(self, bucket: str, identity: str, *, limit: int, window: int) -> bool:
        del limit, window
        self.calls.append((bucket, identity))
        return True


@dataclass
class FakeWorkflows:
    calls: int = 0
    ack_calls: int = 0
    compose_calls: int = 0
    policy: MCPPolicy = field(default_factory=MCPPolicy)
    sessions: PortalSessionStore | None = None
    store: InMemoryStore | None = None
    rate_calls: list[tuple[str, str]] = field(default_factory=list)
    deny_mutation: bool = False

    async def list(self, context: Any, cursor: str | None) -> WorkflowPage:
        del context, cursor
        return WorkflowPage(
            [
                {
                    "kind": "research",
                    "id": "job-1",
                    "state": "partial",
                    "created_at": 1.0,
                    "expires_at": 2.0,
                    "revision": 3,
                    "summary": '<script>alert("x")</script>',
                }
            ],
            None,
        )

    async def detail(self, context: Any, kind: str, object_id: str) -> dict[str, Any]:
        del context
        if object_id == "foreign":
            from slopsearx.workflow_console import WorkflowNotFoundError

            raise WorkflowNotFoundError
        details: dict[str, Any] = {"lineage": [{"relation": "derived_from", "unsafe": "<img src=x>"}]}
        state = "partial"
        if kind == "saved_search":
            state = "active"
            details["event_batch"] = {
                "scope": "tenant_inbox",
                "next_cursor": "1000-0",
                "acknowledged_cursor": "0-0",
                "events": [],
            }
        return {
            "kind": kind,
            "id": object_id,
            "state": state,
            "revision": 3,
            "summary": "Two subquestions",
            "details": details,
        }

    async def cancel_research(self, context: Any, object_id: str, revision: int) -> str:
        del context, object_id
        if self.deny_mutation:
            raise PermissionError
        if revision != 3:
            from slopsearx.workflow_console import WorkflowConflictError

            raise WorkflowConflictError(3)
        self.calls += 1
        return "cancelled"

    async def acknowledge_saved_events(
        self, context: Any, object_id: str, *, expected_revision: int, cursor: str
    ) -> str:
        del context, object_id, expected_revision
        assert cursor == "1000-0"
        self.ack_calls += 1
        return cursor

    async def compose_research(
        self,
        context: Any,
        object_id: str,
        *,
        expected_revision: int,
        question: str,
        strategy: str,
        idempotency_key: str,
    ) -> str:
        del context, object_id, expected_revision, idempotency_key
        assert question == "What follows?"
        assert strategy == "fresh"
        self.compose_calls += 1
        return "job-composed"


@pytest.fixture
def portal() -> Generator[tuple[TestClient, FakeWorkflows, PortalPrincipal], None, None]:
    principal = PortalPrincipal(
        principal_id="principal",
        issuer="https://id.example",
        subject="subject",
        memberships=(
            PortalMembership(
                "tenant-a",
                "Tenant <A>",
                frozenset(
                    {
                        "workflow.read",
                        "workflow.research.cancel",
                        "workflow.saved.ack",
                        "workflow.compose",
                    }
                ),
            ),
        ),
    )
    directory = StaticMembershipResolver((principal,))
    store = InMemoryStore()
    counter = iter(range(100))
    sessions = PortalSessionStore(
        store,
        directory,
        hmac_key=b"z" * 32,
        token_source=lambda _size: f"deterministic-token-{next(counter):03d}-abcdefghijklmnopqrstuvwxyz",
    )
    workflows = FakeWorkflows()
    workflows.sessions = sessions
    workflows.store = store

    limiter = RecordingRateLimiter(workflows.rate_calls)

    runtime = WorkflowPortalRuntime(
        identity=FakeIdentity(),
        memberships=directory,
        sessions=sessions,
        workflows=workflows,  # type: ignore[arg-type]
        store=store,
        external_origin="https://testserver",
        operator_actions=frozenset(
            {"workflow.read", "workflow.research.cancel", "workflow.saved.ack", "workflow.compose"}
        ),
        mutations_enabled=frozenset({"workflow.research.cancel", "workflow.saved.ack", "workflow.compose"}),
        secure_cookie=True,
        audit=PortalAuditLogger(store, pseudonym_key=b"a" * 32),
        rate_limiter=limiter,  # type: ignore[arg-type]
    )
    server_mod.configure_workflow_portal(runtime)
    with TestClient(server_mod.app, base_url="https://testserver") as client:
        yield client, workflows, principal
    server_mod.configure_workflow_portal(None)


def sign_in(client: TestClient) -> None:
    started = client.get("/auth/login?return_to=/workflows", follow_redirects=False)
    assert started.status_code == 303
    assert started.headers["location"].startswith("https://id.example/authorize")
    transaction_cookie = started.headers["set-cookie"]
    assert "__Secure-slopsearx_login=" in transaction_cookie
    assert "Path=/auth/callback" in transaction_cookie
    assert "Secure" in transaction_cookie
    assert "Domain=" not in transaction_cookie
    response = client.get("/auth/callback?code=good", follow_redirects=False)
    assert response.status_code == 303
    assert "Secure" in response.headers["set-cookie"]


def test_protected_negotiation_and_escaped_console(portal: tuple[TestClient, FakeWorkflows, PortalPrincipal]) -> None:
    client, _workflows, _principal = portal
    signed_out = client.get("/workflows", follow_redirects=False)
    assert signed_out.status_code == 303
    assert signed_out.headers["location"].startswith("/auth/login?return_to=/workflows")
    machine = client.get("/workflows", headers={"accept": "application/json"})
    assert machine.status_code == 401
    assert "WWW-Authenticate" in machine.headers

    sign_in(client)
    response = client.get("/workflows")
    assert response.status_code == 200
    assert response.headers["cache-control"] == "no-store"
    assert "Tenant &lt;A&gt;" in response.text
    assert "&lt;script&gt;" in response.text
    assert "<script>alert" not in response.text
    detail = client.get("/workflows/research/job-1")
    assert "&lt;img src=x&gt;" in detail.text
    assert "Submit to confirm" in detail.text


def test_mutation_requires_origin_csrf_and_is_idempotent(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, workflows, _principal = portal
    sign_in(client)
    detail = client.get("/workflows/research/job-1")
    from lxml import html

    document = html.fromstring(detail.text)
    form = document.cssselect('form[action$="/cancel"]')[0]
    values = {element.name: element.value for element in form.cssselect("input")}
    url = form.attrib["action"]

    oversized = client.post(
        url,
        content=b"x" * 20_000,
        headers={
            "origin": "https://testserver",
            "content-type": "application/x-www-form-urlencoded",
            "content-length": "1",
        },
    )
    assert oversized.status_code == 403
    assert workflows.calls == 0
    assert client.post(url, data=values).status_code == 403
    headers = {"origin": "https://testserver"}
    assert client.post(url, data={**values, "csrf": "bad"}, headers=headers).status_code == 403
    first = client.post(url, data=values, headers=headers, follow_redirects=False)
    assert first.status_code == 303
    replay = client.post(url, data=values, headers=headers, follow_redirects=False)
    assert replay.status_code == 303
    assert workflows.calls == 1

    stale = client.post(
        url,
        data={**values, "idempotency_key": "different-key-123", "expected_revision": "2"},
        headers=headers,
    )
    assert stale.status_code == 409


def test_policy_revoked_mutation_is_safe_denial_with_audit(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, workflows, _principal = portal
    assert workflows.store is not None
    sign_in(client)
    from lxml import html

    document = html.fromstring(client.get("/workflows/research/job-1").text)
    form = document.cssselect('form[action$="/cancel"]')[0]
    values = {element.name: element.value for element in form.cssselect("input")}
    workflows.deny_mutation = True
    response = client.post(form.attrib["action"], data=values, headers={"origin": "https://testserver"})
    assert response.status_code == 403
    assert response.json()["title"] == "Action unavailable"
    event = next(
        entry
        for key, value in workflows.store._data.items()
        if key.startswith("portal:audit:v1:")
        for entry in value["entries"]
        if entry["action"] == "workflow.research.cancel" and entry["reason"] == "authorization_denied"
    )
    assert event["outcome"] == "denied"


def test_page_rebuilds_only_allowlisted_markup_and_redirects_stay_local(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    unsafe_fragments = (
        '<img src=x onerror="alert(1)">',
        '<p onclick="alert(1)">unsafe</p>',
        "<p><strong>unbalanced</p></strong>",
        '<a href="//evil.example">leave</a>',
        '<a href="/\\evil.example">leave</a>',
        '<a href="/one" href="/two">duplicate</a>',
    )
    for fragment in unsafe_fragments:
        with pytest.raises(ValueError):
            _page("unsafe", fragment)
    escaped = _page('<script>alert("title")</script>', "<p>Safe</p>", '<svg onload="alert(1)">')
    rendered = escaped.body.decode()
    assert "<script>" not in rendered and "<svg" not in rendered

    malicious_id = "@evil.example?next=//evil.example\r\nLocation: https://evil.example"
    location = _workflow_location("research", malicious_id)
    assert location.startswith("/workflows/research/")
    assert urlsplit(location).netloc == "" and "\r" not in location and "\n" not in location

    client, _workflows, _principal = portal
    sign_in(client)
    detail = client.get("/workflows/research/job-1")
    from lxml import html

    form = html.fromstring(detail.text).cssselect('form[action$="/cancel"]')[0]
    values = {element.name: element.value for element in form.cssselect("input")}
    response = client.post(
        form.attrib["action"],
        data=values,
        headers={"origin": "https://testserver"},
        follow_redirects=False,
    )
    assert response.status_code == 303
    assert response.headers["location"] == "/workflows"


async def test_successful_reauthentication_invalidates_presented_session_and_audits_identity(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, workflows, _principal = portal
    assert workflows.sessions is not None and workflows.store is not None
    sign_in(client)
    old_handle = client.cookies.get("__Host-slopsearx_session")
    assert old_handle
    sign_in(client)
    new_handle = client.cookies.get("__Host-slopsearx_session")
    assert new_handle and new_handle != old_handle
    assert (await workflows.sessions.resolve(old_handle, frozenset({"workflow.read"}))).context is None
    assert (await workflows.sessions.resolve(new_handle, frozenset({"workflow.read"}))).context is not None
    entries = [
        entry
        for key, value in workflows.store._data.items()
        if key.startswith("portal:audit:v1:")
        for entry in value["entries"]
        if entry["action"] == "login" and entry["outcome"] == "succeeded"
    ]
    assert entries
    none_hash = hmac.new(b"a" * 32, b"none", hashlib.sha256).hexdigest()[:24]
    assert entries[-1]["principal"] != none_hash
    assert entries[-1]["tenant"] != none_hash


def test_logout_uses_bounded_server_identity_rate_limit(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, workflows, _principal = portal
    sign_in(client)
    from lxml import html

    document = html.fromstring(client.get("/workflows").text)
    form = document.cssselect('form[action="/auth/logout"]')[0]
    values = {element.name: element.value for element in form.cssselect("input")}
    response = client.post(
        "/auth/logout",
        data=values,
        headers={"origin": "https://testserver"},
        follow_redirects=False,
    )
    assert response.status_code == 303
    assert ("logout", "principaltenant-a") in workflows.rate_calls


def test_saved_ack_and_composition_have_dedicated_guarded_routes(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, workflows, _principal = portal
    sign_in(client)
    from lxml import html

    headers = {"origin": "https://testserver"}
    saved = html.fromstring(client.get("/workflows/saved_search/saved-1").text)
    ack = saved.cssselect('form[action$="/ack"]')[0]
    ack_values = {element.name: element.value for element in ack.cssselect("input")}
    ack_response = client.post(ack.attrib["action"], data=ack_values, headers=headers, follow_redirects=False)
    assert ack_response.status_code == 303
    assert workflows.ack_calls == 1

    staged = html.fromstring(client.get("/workflows/staged_search/staged-1").text)
    compose = staged.cssselect('form[action$="/compose"]')[0]
    compose_values = {element.name: element.value for element in compose.cssselect("input")}
    compose_values.update(question="What follows?", strategy="fresh")
    compose_response = client.post(
        compose.attrib["action"], data=compose_values, headers=headers, follow_redirects=False
    )
    assert compose_response.status_code == 303
    assert workflows.compose_calls == 1


def test_invalid_workflow_cookie_does_not_change_public_portal(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, _workflows, _principal = portal
    without_cookie = client.get("/").text
    with_cookie = client.get("/", headers={"cookie": "__Host-slopsearx_session=invalid"}).text
    assert without_cookie == with_cookie


def test_forbidden_object_uses_same_non_disclosing_response(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    client, _workflows, _principal = portal
    sign_in(client)
    unknown = client.get("/workflows/research/foreign")
    assert unknown.status_code == 404
    assert "unknown, expired, revoked" in unknown.text


def test_two_tenant_chooser_rotation_and_expiry_during_form() -> None:
    now = [1000.0]
    memberships = (
        PortalMembership("tenant-a", "Tenant A", frozenset({"workflow.read", "workflow.research.cancel"})),
        PortalMembership("tenant-b", "Tenant B", frozenset({"workflow.read", "workflow.research.cancel"})),
    )
    principal = PortalPrincipal(
        principal_id="multi-principal",
        issuer="https://id.example",
        subject="subject",
        memberships=memberships,
    )
    directory = StaticMembershipResolver((principal,))
    store = InMemoryStore()
    counter = iter(range(100))
    sessions = PortalSessionStore(
        store,
        directory,
        hmac_key=b"m" * 32,
        clock=lambda: now[0],
        idle_seconds=60,
        absolute_seconds=120,
        token_source=lambda _size: f"multi-token-{next(counter):03d}-abcdefghijklmnopqrstuvwxyz",
    )
    workflows = FakeWorkflows()
    limiter = RecordingRateLimiter()
    runtime = WorkflowPortalRuntime(
        identity=FakeIdentity(),
        memberships=directory,
        sessions=sessions,
        workflows=workflows,  # type: ignore[arg-type]
        store=store,
        external_origin="https://testserver",
        operator_actions=frozenset({"workflow.read", "workflow.research.cancel"}),
        mutations_enabled=frozenset({"workflow.research.cancel"}),
        audit=PortalAuditLogger(store, pseudonym_key=b"b" * 32),
        rate_limiter=limiter,  # type: ignore[arg-type]
    )
    server_mod.configure_workflow_portal(runtime)
    try:
        with TestClient(server_mod.app, base_url="https://testserver") as client:
            sign_in(client)
            for method, path in (
                ("get", "/workflows/research/job-1"),
                ("post", "/workflows/research/job-1/cancel"),
                ("post", "/auth/logout"),
            ):
                pending = getattr(client, method)(path, follow_redirects=False)
                assert pending.status_code == 303
                assert pending.headers["location"] == "/auth/tenant"
            machine_pending = client.get(
                "/workflows/research/job-1", headers={"accept": "application/json"}, follow_redirects=False
            )
            assert machine_pending.status_code == 409
            assert machine_pending.json()["title"] == "Workspace selection required"
            assert workflows.calls == 0
            chooser = client.get("/auth/tenant")
            assert "Tenant A" in chooser.text and "Tenant B" in chooser.text
            from lxml import html

            document = html.fromstring(chooser.text)
            csrf = document.cssselect('input[name="csrf"]')[0].value
            headers = {"origin": "https://testserver"}
            spoofed = client.post(
                "/auth/tenant",
                data={"csrf": csrf, "tenant": "tenant-a"},
                headers={"host": "evil.example", "origin": "https://evil.example"},
            )
            assert spoofed.status_code == 400
            rejected = client.post("/auth/tenant", data={"csrf": csrf, "tenant": "foreign"}, headers=headers)
            assert rejected.status_code == 403
            selected = client.post(
                "/auth/tenant",
                data={"csrf": csrf, "tenant": "tenant-b"},
                headers=headers,
                follow_redirects=False,
            )
            assert selected.status_code == 303
            assert ("tenant_switch", "multi-principal") in limiter.calls
            tenant_event = next(
                entry
                for key, value in store._data.items()
                if key.startswith("portal:audit:v1:")
                for entry in value["entries"]
                if entry["action"] == "tenant_switch" and entry["outcome"] == "succeeded"
            )
            none_hash = hmac.new(b"b" * 32, b"none", hashlib.sha256).hexdigest()[:24]
            assert tenant_event["principal"] != none_hash
            assert tenant_event["tenant"] != none_hash
            assert tenant_event["session_generation"] != none_hash
            assert "Tenant B" in client.get("/workflows").text

            detail = html.fromstring(client.get("/workflows/research/job-1").text)
            form = detail.cssselect('form[action$="/cancel"]')[0]
            values = {element.name: element.value for element in form.cssselect("input")}
            now[0] = 1121
            expired = client.post(form.attrib["action"], data=values, headers=headers, follow_redirects=False)
            assert expired.status_code == 303
            assert expired.headers["location"].startswith("/auth/login")
            assert workflows.calls == 0
    finally:
        server_mod.configure_workflow_portal(None)


def test_authenticated_workflow_route_is_keyboard_usable_at_narrow_width(
    portal: tuple[TestClient, FakeWorkflows, PortalPrincipal],
) -> None:
    playwright = pytest.importorskip("playwright.sync_api")
    client, _workflows, _principal = portal
    sign_in(client)
    markup = client.get("/workflows/research/job-1").text
    with playwright.sync_playwright() as api:
        browser = api.chromium.launch()
        page = browser.new_page(viewport={"width": 320, "height": 640})
        page.set_content(markup)
        assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")
        assert page.get_by_text("Submit to confirm.").count() == 2
        assert page.locator("script").count() == 0
        page.keyboard.press("Tab")
        assert page.get_by_role("link", name="SlopSearX").evaluate("el => document.activeElement === el")
        page.keyboard.press("Tab")
        assert page.get_by_role("button", name="Cancel queued work").evaluate("el => document.activeElement === el")
        browser.close()
