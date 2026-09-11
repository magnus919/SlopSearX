"""Protected server-rendered workflow console transport."""

# ruff: noqa: E501 -- keeping security-sensitive HTML form fields together aids review.

from __future__ import annotations

import asyncio
import hashlib
import hmac
import html
import json
import os
import secrets
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Any, Callable
from urllib.parse import parse_qs, quote, urlsplit

from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse, JSONResponse, RedirectResponse, Response

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.state import McpState
from slopsearx.portal_audit import PortalAuditLogger
from slopsearx.portal_auth import (
    IdentityProvider,
    MembershipResolver,
    PortalAuthContext,
    PortalMembership,
    PortalPrincipal,
    PortalSession,
    PortalSessionStore,
    StaticMembershipResolver,
    pending_csrf,
    safe_return_to,
    valid_csrf,
)
from slopsearx.portal_oidc import OIDCIdentityProvider, OIDCSettings
from slopsearx.portal_rate_limit import PortalRateLimiter
from slopsearx.research import ResearchJobRunner
from slopsearx.research_store import ResearchJobStore
from slopsearx.retrieval_receipts import ReceiptStore
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import SavedSearchStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import KeyValueStore, SnapshotStore
from slopsearx.staged import StagedSearchStore
from slopsearx.workflow_console import WorkflowConflictError, WorkflowConsoleService, WorkflowNotFoundError

COOKIE_NAME = "__Host-slopsearx_session"
TRANSACTION_COOKIE_NAME = "__Secure-slopsearx_login"
MAX_FORM_BYTES = 16_384
_INTENT_GRANTS = {"jobs": "jobs", "security": "security", "science": "science"}


@dataclass
class WorkflowPortalRuntime:
    identity: IdentityProvider
    memberships: MembershipResolver
    sessions: PortalSessionStore
    workflows: WorkflowConsoleService
    store: KeyValueStore
    external_origin: str
    operator_actions: frozenset[str]
    mutations_enabled: frozenset[str] = frozenset()
    secure_cookie: bool = True
    audit: PortalAuditLogger | None = None
    rate_limiter: PortalRateLimiter | None = None


def _required_secret(name: str) -> bytes:
    value = os.getenv(name, "").encode()
    if len(value) < 32:
        raise ValueError(f"{name} must contain at least 32 bytes")
    return value


def _previous_secret() -> tuple[bytes | None, int]:
    value = os.getenv("SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY", "").encode()
    if not value:
        return None, 0
    if len(value) < 32:
        raise ValueError("SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY must contain at least 32 bytes")
    drain = int(os.getenv("SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS", "600"))
    if not 60 <= drain <= 600:
        raise ValueError("previous portal key drain must be between 60 and 600 seconds")
    return value, drain


def _load_directory(path: str, issuer: str) -> StaticMembershipResolver:
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError("workflow portal membership file is unavailable or invalid") from exc
    if not isinstance(payload, dict) or set(payload) != {"principals"}:
        raise ValueError("workflow portal membership file must contain only principals")
    raw_principals = payload["principals"]
    if not isinstance(raw_principals, list) or not raw_principals:
        raise ValueError("workflow portal membership principals must be a non-empty list")

    def stable_string(value: Any, field: str) -> str:
        if not isinstance(value, str) or not value or value != value.strip():
            raise ValueError(f"workflow portal {field} must be a non-empty stable string")
        return value

    def positive_revision(value: Any, field: str) -> int:
        if type(value) is not int or value < 1:
            raise ValueError(f"workflow portal {field} must be a positive integer")
        return value

    def boolean(value: Any, field: str) -> bool:
        if type(value) is not bool:
            raise ValueError(f"workflow portal {field} must be a boolean")
        return value

    principals: list[PortalPrincipal] = []
    for item in raw_principals:
        if (
            not isinstance(item, dict)
            or not {"principal_id", "issuer", "subject", "memberships"} <= set(item)
            or set(item) - {"principal_id", "issuer", "subject", "memberships", "revision", "enabled"}
        ):
            raise ValueError("workflow portal principal has an invalid schema")
        item_issuer = stable_string(item["issuer"], "principal issuer")
        if item_issuer.rstrip("/") != issuer.rstrip("/"):
            raise ValueError("workflow portal membership issuer mismatch")
        raw_memberships = item["memberships"]
        if not isinstance(raw_memberships, list) or not raw_memberships:
            raise ValueError("workflow portal memberships must be a non-empty list")
        memberships: list[PortalMembership] = []
        for member in raw_memberships:
            if (
                not isinstance(member, dict)
                or not {"tenant_id", "display_name", "grants"} <= set(member)
                or set(member) - {"tenant_id", "display_name", "grants", "revision", "enabled"}
            ):
                raise ValueError("workflow portal membership has an invalid schema")
            raw_grants = member["grants"]
            if not isinstance(raw_grants, list) or not raw_grants:
                raise ValueError("workflow portal grants must be a non-empty list")
            grants = [stable_string(grant, "grant") for grant in raw_grants]
            if len(set(grants)) != len(grants):
                raise ValueError("workflow portal grants must be unique")
            memberships.append(
                PortalMembership(
                    tenant_id=stable_string(member["tenant_id"], "tenant ID"),
                    display_name=stable_string(member["display_name"], "tenant display name"),
                    grants=frozenset(grants),
                    revision=positive_revision(member.get("revision", 1), "membership revision"),
                    enabled=boolean(member.get("enabled", True), "membership enabled"),
                )
            )
        principals.append(
            PortalPrincipal(
                principal_id=stable_string(item["principal_id"], "principal ID"),
                issuer=issuer.rstrip("/"),
                subject=stable_string(item["subject"], "subject"),
                memberships=tuple(memberships),
                revision=positive_revision(item.get("revision", 1), "principal revision"),
                enabled=boolean(item.get("enabled", True), "principal enabled"),
            )
        )
    return StaticMembershipResolver(tuple(principals))


def build_workflow_runtime_from_env(ctx: AppContext, policy: MCPPolicy) -> WorkflowPortalRuntime | None:
    """Build the production OIDC/Valkey runtime, or leave routes disabled."""
    if os.getenv("SLOPSEARX_WORKFLOW_PORTAL_ENABLED", "").strip().lower() not in {"1", "true", "yes"}:
        return None
    store = ctx.cache
    catalog = ctx.catalog
    if store is None or not store.is_connected:
        raise ValueError("workflow portal requires connected Valkey")
    if catalog is None:
        raise ValueError("workflow portal requires the shared capability catalog")
    external_origin = os.getenv("SLOPSEARX_PORTAL_EXTERNAL_ORIGIN", "").rstrip("/")
    parsed_origin = urlsplit(external_origin)
    if (
        parsed_origin.scheme != "https"
        or not parsed_origin.netloc
        or parsed_origin.path
        or parsed_origin.query
        or parsed_origin.fragment
    ):
        raise ValueError("SLOPSEARX_PORTAL_EXTERNAL_ORIGIN must be an exact HTTPS origin")
    if os.getenv("SLOPSEARX_PORTAL_TRUSTED_PROXIES", "").strip():
        raise ValueError("workflow portal trusted proxies are not supported; use direct TLS exposure")
    issuer = os.getenv("SLOPSEARX_PORTAL_OIDC_ISSUER", "").rstrip("/")
    settings = OIDCSettings(
        issuer=issuer,
        client_id=os.getenv("SLOPSEARX_PORTAL_OIDC_CLIENT_ID", ""),
        client_secret=os.getenv("SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET", ""),
        callback_url=f"{external_origin}/auth/callback",
    )
    secret = _required_secret("SLOPSEARX_PORTAL_SESSION_KEY")
    previous_secret, previous_drain = _previous_secret()
    directory = _load_directory(os.getenv("SLOPSEARX_PORTAL_MEMBERSHIPS_FILE", ""), issuer)
    actions = frozenset(
        {"workflow.read"}
        | {item.strip() for item in os.getenv("SLOPSEARX_WORKFLOW_PORTAL_ACTIONS", "").split(",") if item.strip()}
    )
    supported_mutations = frozenset(
        {
            "workflow.research.cancel",
            "workflow.research.retry",
            "workflow.saved.control",
            "workflow.saved.ack",
            "workflow.compose",
        }
        & actions
    )
    jobs = ResearchJobStore(store)
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)

    def dispatch_validator(query: Any) -> str | None:
        grant = _INTENT_GRANTS.get(query.intent)
        if query.requires_intent_grant and grant and not policy.tool_enabled(grant):
            return "current policy denies this research intent"
        if not policy.targeted_sensitive_allowed and set(query.engines) & policy.sensitive_engines:
            return "current policy denies this engine scope"
        return None

    research_runner = ResearchJobRunner(
        SearchService(ctx),
        jobs,
        snapshots,
        catalog,
        policy,
        dispatch_validator=dispatch_validator,
        lease_ttl=policy.job_lease_ttl_seconds,
        poll_interval=policy.job_poll_interval_seconds,
        max_concurrent_jobs=policy.job_max_concurrent_jobs,
    )
    staged = StagedSearchStore(store)
    saved = SavedSearchStore(
        store,
        event_capacity=policy.saved_event_capacity,
        event_retention_seconds=policy.saved_event_retention_seconds,
        event_consumers=policy.saved_event_max_consumers,
    )
    saved_runner = SavedSearchRunner(
        SearchService(ctx),
        saved,
        policy,
        policy_check=lambda _definition: "saved-search policy is not initialized",
        policy_fingerprint=lambda _definition: "",
    )
    receipts = ReceiptStore(store)
    composition_state = McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=SearchService(ctx),
        snapshots=snapshots,
        job_store=jobs,
        runner=research_runner,
        version="portal",
        staged_store=staged,
        receipt_store=receipts,
        saved_store=saved,
        saved_runner=saved_runner,
    )
    workflows = WorkflowConsoleService(
        policy=policy,
        jobs=jobs,
        research_runner=research_runner,
        staged=staged,
        saved=saved,
        cursor_key=hmac.new(secret, b"workflow-cursor", hashlib.sha256).digest(),
        composition_state=composition_state,
    )
    return WorkflowPortalRuntime(
        identity=OIDCIdentityProvider(
            settings,
            store,
            transaction_key=hmac.new(secret, b"oidc-transaction", hashlib.sha256).digest(),
            previous_transaction_key=(
                hmac.new(previous_secret, b"oidc-transaction", hashlib.sha256).digest() if previous_secret else None
            ),
            previous_key_drain_seconds=previous_drain,
        ),
        memberships=directory,
        sessions=PortalSessionStore(
            store,
            directory,
            hmac_key=secret,
            previous_hmac_key=previous_secret,
            previous_key_drain_seconds=previous_drain,
        ),
        workflows=workflows,
        store=store,
        external_origin=external_origin,
        operator_actions=actions,
        mutations_enabled=supported_mutations,
        audit=PortalAuditLogger(
            store,
            pseudonym_key=hmac.new(secret, b"portal-audit", hashlib.sha256).digest(),
        ),
        rate_limiter=PortalRateLimiter(
            store,
            key=hmac.new(secret, b"portal-rate-limit", hashlib.sha256).digest(),
        ),
    )


class MutationGuard:
    def __init__(self, store: KeyValueStore) -> None:
        self.store = store

    @staticmethod
    def _key(context: PortalAuthContext, action: str, idem: str) -> str:
        digest = hashlib.sha256(f"{context.principal_id}\0{context.tenant_id}\0{action}\0{idem}".encode()).hexdigest()
        return f"portal:mutation:v1:{digest}"

    async def claim(
        self, context: PortalAuthContext, action: str, idem: str, request_digest: str
    ) -> tuple[str, dict[str, Any] | None]:
        key = self._key(context, action, idem)
        current = await self.store.get(key)
        if current:
            if current.get("request_digest") != request_digest:
                return "conflict", None
            return ("replay" if current.get("state") == "done" else "inflight"), current
        claim_id = secrets.token_urlsafe(24)
        claim = {"request_digest": request_digest, "state": "pending", "claim_id": claim_id}
        set_nx = getattr(self.store, "set_nx", None)
        if set_nx is not None:
            created = bool(await set_nx(key, claim, 600))
        else:
            client = getattr(self.store, "_client", None)
            if client is None:
                return "unavailable", None
            created = bool(await client.set(key, json.dumps(claim), ex=600, nx=True))
        if created:
            return "claimed", claim
        current = await self.store.get(key)
        if current and current.get("request_digest") == request_digest:
            return ("replay" if current.get("state") == "done" else "inflight"), current
        return "conflict", None

    async def finish(
        self,
        context: PortalAuthContext,
        action: str,
        idem: str,
        request_digest: str,
        claim_id: str,
        result: dict[str, Any],
    ) -> bool:
        key = self._key(context, action, idem)
        completed = {
            "request_digest": request_digest,
            "state": "done",
            "claim_id": claim_id,
            "result": result,
        }
        client = getattr(self.store, "_client", None)
        if client is not None and hasattr(client, "eval"):
            script = """
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if current.state ~= 'pending' or current.claim_id ~= ARGV[1] or current.request_digest ~= ARGV[2] then
  return 0
end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
return 1
"""
            return bool(await client.eval(script, 1, key, claim_id, request_digest, "600", json.dumps(completed)))
        async with _MUTATION_LOCK:
            current = await self.store.get(key)
            if (
                not current
                or current.get("state") != "pending"
                or current.get("claim_id") != claim_id
                or current.get("request_digest") != request_digest
            ):
                return False
            await self.store.set(key, completed, 600)
            return True


_MUTATION_LOCK = asyncio.Lock()


_STYLE = """
:root{color-scheme:dark;background:#111;color:#eee;font:16px/1.5 system-ui,sans-serif}
body{margin:0}.shell{width:min(72rem,calc(100% - 2rem));margin:auto;padding:1.2rem 0 4rem}
a{color:#9ddcff}.bar{display:flex;gap:1rem;align-items:center;justify-content:space-between;flex-wrap:wrap}
.card{border:1px solid #444;border-radius:.7rem;padding:1rem;margin:1rem 0;background:#191919;overflow-wrap:anywhere}
.pill{border:1px solid #666;border-radius:99rem;padding:.15rem .55rem}.mutations{display:flex;gap:.5rem;flex-wrap:wrap}
button{font:inherit;padding:.55rem .8rem;border-radius:.4rem;border:1px solid #888;background:#292929;color:#fff}
button:focus,a:focus{outline:3px solid #ffd166;outline-offset:3px}pre{white-space:pre-wrap;overflow-wrap:anywhere}
.notice{padding:.7rem;border-left:4px solid #ffd166;background:#27220f}@media(max-width:30rem){.shell{width:calc(100% - 1rem)}}
"""

_FRAGMENT_ATTRIBUTES = {
    "a": frozenset({"href"}),
    "article": frozenset({"class"}),
    "button": frozenset({"name", "value"}),
    "div": frozenset({"class"}),
    "form": frozenset({"method", "action"}),
    "h2": frozenset(),
    "input": frozenset({"type", "name", "value", "required", "maxlength"}),
    "label": frozenset(),
    "option": frozenset(),
    "p": frozenset({"class", "role"}),
    "pre": frozenset(),
    "section": frozenset(),
    "select": frozenset({"name"}),
    "span": frozenset({"class"}),
    "strong": frozenset(),
}
_VOID_FRAGMENT_TAGS = frozenset({"input"})


class _FragmentSanitizer(HTMLParser):
    """Rebuild internal portal fragments from a small, URL-safe allowlist."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.output: list[str] = []
        self.stack: list[str] = []

    @staticmethod
    def _local_url(value: str) -> bool:
        parsed = urlsplit(value)
        return (
            not parsed.scheme
            and not parsed.netloc
            and value.startswith("/")
            and not value.startswith("//")
            and "\\" not in value
        )

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        allowed = _FRAGMENT_ATTRIBUTES.get(tag)
        names = [name for name, _value in attrs]
        if allowed is None or len(names) != len(set(names)) or any(name not in allowed for name in names):
            raise ValueError("unsafe workflow portal HTML fragment")
        rendered: list[str] = []
        for name, raw_value in attrs:
            value = raw_value or ""
            if name in {"href", "action"} and not self._local_url(value):
                raise ValueError("unsafe workflow portal URL")
            if name == "method" and value.lower() != "post":
                raise ValueError("unsafe workflow portal form method")
            rendered.append(f' {name}="{html.escape(value, quote=True)}"')
        self.output.append(f"<{tag}{''.join(rendered)}>")
        if tag not in _VOID_FRAGMENT_TAGS:
            self.stack.append(tag)

    def handle_endtag(self, tag: str) -> None:
        if not self.stack or self.stack.pop() != tag:
            raise ValueError("unbalanced workflow portal HTML fragment")
        self.output.append(f"</{tag}>")

    def handle_data(self, data: str) -> None:
        self.output.append(html.escape(data))

    def sanitized(self) -> str:
        if self.stack:
            raise ValueError("unbalanced workflow portal HTML fragment")
        return "".join(self.output)


def _sanitize_fragment(fragment: str) -> str:
    sanitizer = _FragmentSanitizer()
    sanitizer.feed(fragment)
    sanitizer.close()
    return sanitizer.sanitized()


def _page(title: str, body: str, tenant: str | None = None) -> HTMLResponse:
    tenant_markup = f'<span class="pill">{html.escape(tenant)}</span>' if tenant else ""
    safe_body = _sanitize_fragment(body)
    content = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{html.escape(title)} · SlopSearX</title><style>{_STYLE}</style></head><body><main class="shell"><header class="bar"><a href="/">SlopSearX</a>{tenant_markup}</header><h1>{html.escape(title)}</h1>{safe_body}</main></body></html>"""
    response = HTMLResponse(content)  # lgtm[py/reflective-xss] allowlisted fragment rebuilt above
    response.headers["Cache-Control"] = "no-store"
    response.headers["Content-Security-Policy"] = (
        "default-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; style-src 'unsafe-inline'"
    )
    # Same-origin form submissions need a non-opaque Origin/Referer for the
    # mutation gate; this policy still suppresses referrers to other origins.
    response.headers["Referrer-Policy"] = "same-origin"
    response.headers["X-Content-Type-Options"] = "nosniff"
    return response


def _problem(status: int, title: str) -> JSONResponse:
    response = JSONResponse({"type": "about:blank", "title": title, "status": status}, status_code=status)
    response.headers["Cache-Control"] = "no-store"
    if status == 401:
        response.headers["WWW-Authenticate"] = 'Session realm="SlopSearX workflows"'
    return response


def _workflow_location(kind: str, object_id: str) -> str:
    if kind not in {"research", "dependency_dossier", "staged_search", "saved_search"}:
        return "/workflows"
    return f"/workflows/{kind}/{quote(object_id, safe='')}"


def _machine(request: Request) -> bool:
    accept = request.headers.get("accept", "")
    return "application/json" in accept or "text/html" not in accept and accept not in {"", "*/*"}


def _request_host_allowed(request: Request, runtime: WorkflowPortalRuntime) -> bool:
    return urlsplit(str(request.url)).netloc == urlsplit(runtime.external_origin).netloc


async def _rate_allowed(
    runtime: WorkflowPortalRuntime,
    request: Request,
    bucket: str,
    identity: str,
    *,
    limit: int,
    window: int,
) -> bool:
    return runtime.rate_limiter is None or await runtime.rate_limiter.allow(
        bucket, identity, limit=limit, window=window
    )


async def _form(request: Request) -> dict[str, str]:
    if request.headers.get("content-type", "").split(";", 1)[0].strip() != "application/x-www-form-urlencoded":
        raise ValueError("content type")
    supplied_length = request.headers.get("content-length")
    if supplied_length is not None:
        try:
            length = int(supplied_length)
        except ValueError as exc:
            raise ValueError("invalid content length") from exc
        if length < 0 or length > MAX_FORM_BYTES:
            raise ValueError("form too large")
    body = await request.body()
    if len(body) > MAX_FORM_BYTES:
        raise ValueError("form too large")
    parsed = parse_qs(body.decode("utf-8"), keep_blank_values=True, max_num_fields=12)
    return {key: values[-1] for key, values in parsed.items()}


def _same_origin(request: Request, origin: str) -> bool:
    supplied = request.headers.get("origin")
    if supplied:
        return hmac.compare_digest(supplied.rstrip("/"), origin.rstrip("/"))
    referer = request.headers.get("referer", "")
    return referer == origin.rstrip("/") or referer.startswith(origin.rstrip("/") + "/")


async def _auth(
    request: Request, runtime: WorkflowPortalRuntime
) -> tuple[PortalAuthContext | None, PortalSession | None, Response | None]:
    handle = request.cookies.get(COOKIE_NAME)
    if not _request_host_allowed(request, runtime):
        return None, None, _problem(400, "Invalid request host")
    resolution = await runtime.sessions.resolve(handle, runtime.operator_actions)
    if resolution.context is not None:
        return resolution.context, resolution.session, None
    if resolution.session is not None:
        return None, resolution.session, None
    if resolution.unavailable:
        await _audit(runtime, request, "login", "denied", "store_unavailable")
    else:
        await _audit(runtime, request, "login", "denied", "authentication_required")
    if _machine(request):
        return None, None, _problem(401, "Authentication required")
    target = safe_return_to(request.url.path + (f"?{request.url.query}" if request.url.query else ""))
    return None, None, RedirectResponse(f"/auth/login?return_to={quote(target, safe='/')}", status_code=303)


def _tenant_selection_required(request: Request, session: PortalSession | None) -> Response | None:
    """Return a controlled response while an authenticated session awaits tenant choice."""
    if session is None:
        return None
    if _machine(request):
        return _problem(409, "Workspace selection required")
    return RedirectResponse("/auth/tenant", status_code=303)


def _not_found() -> HTMLResponse:
    response = _page(
        "Not found or unavailable",
        '<p>The workflow is unavailable. It may be unknown, expired, revoked, or outside your current access.</p><p><a href="/workflows">Return to workflows</a></p>',
    )
    response.status_code = 404
    return response


async def _audit(
    runtime: WorkflowPortalRuntime,
    request: Request,
    action: str,
    outcome: str,
    reason: str,
    context: PortalAuthContext | None = None,
    *,
    object_kind: str | None = None,
    object_id: str | None = None,
) -> None:
    if runtime.audit is None:
        return
    await runtime.audit.record(
        request_id=str(getattr(request.state, "request_id", "")),
        action=action,
        outcome=outcome,
        reason=reason,
        principal_id=context.principal_id if context else None,
        tenant_id=context.tenant_id if context else None,
        session_generation=context.session_generation if context else None,
        object_kind=object_kind,
        object_id=object_id,
    )


def _audit_context(
    principal: PortalPrincipal, session: PortalSession, operator_actions: frozenset[str]
) -> PortalAuthContext:
    membership = next((item for item in principal.memberships if item.tenant_id == session.active_tenant), None)
    return PortalAuthContext(
        principal_id=principal.principal_id,
        tenant_id=session.active_tenant or "",
        tenant_label=membership.display_name if membership else "",
        grants=(membership.grants & operator_actions) if membership else frozenset(),
        principal_revision=principal.revision,
        membership_revision=membership.revision if membership else 0,
        session_generation=session.generation,
        csrf_token="",
    )


def build_workflow_router(runtime_getter: Callable[[], WorkflowPortalRuntime | None]) -> APIRouter:
    router = APIRouter()

    def runtime() -> WorkflowPortalRuntime | None:
        return runtime_getter()

    @router.get("/auth/login")
    async def login(request: Request, return_to: str | None = None) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        network = request.client.host if request.client else "unknown"
        if not _request_host_allowed(request, current) or not await _rate_allowed(
            current, request, "login", network, limit=10, window=60
        ):
            return _problem(429, "Sign-in temporarily unavailable")
        started = await current.identity.begin(safe_return_to(return_to))
        response = RedirectResponse(started.location, status_code=303)
        response.set_cookie(
            TRANSACTION_COOKIE_NAME,
            started.transaction_handle,
            max_age=600,
            secure=current.secure_cookie,
            httponly=True,
            samesite="lax",
            path="/auth/callback",
        )
        return response

    @router.get("/auth/callback")
    async def callback(request: Request) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        transaction = request.cookies.get(TRANSACTION_COOKIE_NAME, "missing")
        network = request.client.host if request.client else "unknown"
        if not _request_host_allowed(request, current) or not await _rate_allowed(
            current, request, "callback", transaction + network, limit=20, window=600
        ):
            return _problem(429, "Sign-in temporarily unavailable")
        try:
            issuer, subject, return_to = await current.identity.complete(
                str(request.url), request.cookies.get(TRANSACTION_COOKIE_NAME)
            )
            principal = await current.memberships.resolve_subject(issuer, subject)
            if principal is None or not principal.enabled:
                raise PermissionError
            enabled = tuple(item for item in principal.memberships if item.enabled)
            if not enabled:
                raise PermissionError
            active = enabled[0].tenant_id if len(enabled) == 1 else None
            handle, new_session = await current.sessions.replace(request.cookies.get(COOKIE_NAME), principal, active)
        except Exception:  # noqa: BLE001 - callback errors are deliberately non-disclosing
            await _audit(current, request, "login", "denied", "callback_rejected")
            failed = _page(
                "Sign-in failed",
                '<p>We could not complete sign-in.</p><p><a href="/auth/login">Try again</a> · <a href="/">Return to search</a></p>',
            )
            failed.status_code = 401
            return failed
        redirect = RedirectResponse(return_to if active else "/auth/tenant", status_code=303)
        await _audit(
            current,
            request,
            "login",
            "succeeded",
            "ok",
            _audit_context(principal, new_session, current.operator_actions),
        )
        redirect.set_cookie(COOKIE_NAME, handle, secure=current.secure_cookie, httponly=True, samesite="lax", path="/")
        redirect.delete_cookie(
            TRANSACTION_COOKIE_NAME,
            path="/auth/callback",
            secure=current.secure_cookie,
            httponly=True,
            samesite="lax",
        )
        return redirect

    @router.get("/auth/tenant")
    async def tenant_choice(request: Request) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        _context, session, denied = await _auth(request, current)
        if denied:
            return denied
        assert session is not None
        principal = await current.memberships.resolve_principal(session.principal_id)
        if principal is None:
            return _problem(401, "Authentication required")
        choices = "".join(
            f'<button name="tenant" value="{html.escape(item.tenant_id, quote=True)}">{html.escape(item.display_name)}</button>'
            for item in principal.memberships
            if item.enabled
        )
        body = f'<p>Choose the workspace to supervise.</p><form method="post" action="/auth/tenant"><input type="hidden" name="csrf" value="{pending_csrf(session)}">{choices}</form>'
        return _page("Choose workspace", body)

    @router.post("/auth/tenant")
    async def choose_tenant(request: Request) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        if not _request_host_allowed(request, current):
            return _problem(400, "Invalid request host")
        handle = request.cookies.get(COOKIE_NAME)
        resolution = await current.sessions.resolve(handle, current.operator_actions)
        session = resolution.session
        if session is None or not _same_origin(request, current.external_origin):
            await _audit(current, request, "tenant_switch", "denied", "csrf_denied")
            return _problem(403, "Request denied")
        if not await _rate_allowed(
            current,
            request,
            "tenant_switch",
            session.principal_id + (session.active_tenant or ""),
            limit=30,
            window=60,
        ):
            return _problem(429, "Action temporarily unavailable")
        try:
            values = await _form(request)
        except (ValueError, UnicodeDecodeError):
            await _audit(current, request, "tenant_switch", "denied", "csrf_denied")
            return _problem(403, "Request denied")
        if not hmac.compare_digest(values.get("csrf", ""), pending_csrf(session)):
            await _audit(current, request, "tenant_switch", "denied", "csrf_denied")
            return _problem(403, "Request denied")
        principal = await current.memberships.resolve_principal(session.principal_id)
        if principal is None:
            await _audit(current, request, "tenant_switch", "denied", "authorization_denied")
            return _problem(403, "Request denied")
        try:
            new_handle, new_session = await current.sessions.rotate(handle or "", session, values.get("tenant", ""))
        except PermissionError:
            await _audit(current, request, "tenant_switch", "denied", "authorization_denied")
            return _problem(403, "Request denied")
        await _audit(
            current,
            request,
            "tenant_switch",
            "succeeded",
            "ok",
            _audit_context(principal, new_session, current.operator_actions),
        )
        response = RedirectResponse("/workflows", status_code=303)
        response.set_cookie(
            COOKIE_NAME, new_handle, secure=current.secure_cookie, httponly=True, samesite="lax", path="/"
        )
        return response

    @router.post("/auth/logout")
    async def logout(request: Request) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        context, session, denied = await _auth(request, current)
        if denied:
            return denied
        if selection := _tenant_selection_required(request, session if context is None else None):
            return selection
        assert context is not None
        if not await _rate_allowed(
            current,
            request,
            "logout",
            context.principal_id + context.tenant_id,
            limit=30,
            window=60,
        ):
            return _problem(429, "Action temporarily unavailable")
        try:
            values = await _form(request)
        except (ValueError, UnicodeDecodeError):
            await _audit(current, request, "logout", "denied", "csrf_denied", context)
            return _problem(403, "Request denied")
        if (
            context is None
            or not _same_origin(request, current.external_origin)
            or not valid_csrf(context, values.get("csrf"))
        ):
            await _audit(current, request, "logout", "denied", "csrf_denied", context)
            return _problem(403, "Request denied")
        await current.sessions.revoke(request.cookies.get(COOKIE_NAME, ""))
        await _audit(current, request, "logout", "succeeded", "ok", context)
        response = RedirectResponse("/", status_code=303)
        response.delete_cookie(COOKIE_NAME, path="/")
        response.headers["Clear-Site-Data"] = '"cache"'
        return response

    @router.get("/workflows")
    async def workflows(request: Request, cursor: str | None = None) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        context, session, denied = await _auth(request, current)
        if denied:
            return denied
        if selection := _tenant_selection_required(request, session if context is None else None):
            return selection
        assert context is not None
        if not await _rate_allowed(
            current,
            request,
            "read",
            context.principal_id + context.tenant_id,
            limit=120,
            window=60,
        ):
            return _problem(429, "Workflow access temporarily unavailable")
        if not context.allows("workflow.read"):
            await _audit(current, request, "workflow.read", "denied", "authorization_denied", context)
            response = _page(
                "Access unavailable",
                '<p>Your current workspace does not grant workflow access.</p><p><a href="/">Return to search</a></p>',
                context.tenant_label,
            )
            response.status_code = 403
            return response
        try:
            page = await current.workflows.list(context, cursor)
        except WorkflowNotFoundError:
            await _audit(current, request, "workflow.read", "denied", "object_unavailable", context)
            return _not_found()
        await _audit(current, request, "workflow.read", "allowed", "ok", context)
        cards = (
            "".join(
                f'<article class="card"><div class="bar"><strong>{html.escape(item["kind"].replace("_", " ").title())}</strong><span class="pill">{html.escape(item["state"])}</span></div><p>{html.escape(item["summary"])}</p><a href="{_workflow_location(str(item["kind"]), str(item["id"]))}">Inspect workflow</a></article>'
                for item in page.items
            )
            or '<p class="notice" role="status">No retained workflows are available for this workspace.</p>'
        )
        next_link = (
            f'<p><a href="/workflows?cursor={quote(page.next_cursor)}">Older workflows</a></p>'
            if page.next_cursor
            else ""
        )
        logout = f'<form method="post" action="/auth/logout"><input type="hidden" name="csrf" value="{context.csrf_token}"><button>Sign out</button></form>'
        return _page("Workflows", cards + next_link + logout, context.tenant_label)

    @router.get("/workflows/{kind}/{object_id}")
    async def workflow_detail(request: Request, kind: str, object_id: str) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        context, session, denied = await _auth(request, current)
        if denied:
            return denied
        if selection := _tenant_selection_required(request, session if context is None else None):
            return selection
        assert context is not None
        if not await _rate_allowed(
            current,
            request,
            "read",
            context.principal_id + context.tenant_id,
            limit=120,
            window=60,
        ):
            return _problem(429, "Workflow access temporarily unavailable")
        try:
            item = await current.workflows.detail(context, kind, object_id)
        except WorkflowNotFoundError:
            await _audit(
                current,
                request,
                "workflow.read",
                "denied",
                "object_unavailable",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _not_found()
        await _audit(
            current,
            request,
            "workflow.read",
            "allowed",
            "ok",
            context,
            object_kind=kind,
            object_id=object_id,
        )
        serialized = json.dumps(item.get("details", {}), indent=2, sort_keys=True, default=str)
        if len(serialized) > 65_536:
            serialized = serialized[:65_536] + "\n… bounded display truncated"
        detail = html.escape(serialized)
        forms = ""
        action_specs = []
        if kind == "research":
            action_specs = [("cancel", "Cancel queued work"), ("retry", "Retry failed work")]
        elif kind == "saved_search":
            action_specs = [
                (
                    ("resume" if item["state"] == "paused" else "pause"),
                    ("Resume" if item["state"] == "paused" else "Pause"),
                )
            ]
            batch = item.get("details", {}).get("event_batch")
            if isinstance(batch, dict) and batch.get("next_cursor") != batch.get("acknowledged_cursor"):
                action_specs.append(("ack", "Acknowledge displayed events"))
        elif kind == "staged_search":
            action_specs = [("compose", "Start research from this evidence")]
        for action, label in action_specs:
            grant = {
                "cancel": "workflow.research.cancel",
                "retry": "workflow.research.retry",
                "pause": "workflow.saved.control",
                "resume": "workflow.saved.control",
                "ack": "workflow.saved.ack",
                "compose": "workflow.compose",
            }[action]
            if grant in current.mutations_enabled and context.allows(grant):
                idem = secrets.token_urlsafe(18)
                consequence = {
                    "cancel": "Stops work that has not started; completed evidence remains available.",
                    "retry": "Runs only retryable failed or empty subquestions and uses remaining budget.",
                    "pause": "Pauses future saved-search runs and resets its comparison baseline.",
                    "resume": "Resumes future saved-search runs and resets its comparison baseline.",
                    "ack": "Advances this workspace inbox through the displayed event cursor.",
                    "compose": "Starts a new research job from this staged evidence.",
                }[action]
                extra = ""
                if action == "ack":
                    extra = f'<input type="hidden" name="cursor" value="{html.escape(str(batch["next_cursor"]), quote=True)}">'
                elif action == "compose":
                    extra = f'<label>Research question <input required maxlength="{current.workflows.policy.max_query_length}" name="question"></label><label>Strategy <select name="strategy"><option>triangulate</option><option>broad</option><option>fresh</option><option>counterevidence</option></select></label>'
                action_url = f"{_workflow_location(kind, object_id)}/{quote(action, safe='')}"
                forms += f'<form method="post" action="{action_url}"><p class="notice">{html.escape(consequence)} Submit to confirm.</p><input type="hidden" name="csrf" value="{context.csrf_token}"><input type="hidden" name="idempotency_key" value="{idem}"><input type="hidden" name="expected_revision" value="{item["revision"]}">{extra}<button>{html.escape(label)}</button></form>'
        body = f'<p><span class="pill">{html.escape(item["state"])}</span></p><p>{html.escape(item["summary"])}</p><section><h2>Evidence and lineage</h2><pre>{detail}</pre></section><div class="mutations">{forms}</div><p><a href="/workflows">Return to workflows</a></p>'
        return _page(kind.replace("_", " ").title(), body, context.tenant_label)

    @router.post("/workflows/{kind}/{object_id}/{action}")
    async def workflow_action(request: Request, kind: str, object_id: str, action: str) -> Response:
        current = runtime()
        if current is None:
            return Response(status_code=404)
        context, session, denied = await _auth(request, current)
        if denied:
            return denied
        if selection := _tenant_selection_required(request, session if context is None else None):
            return selection
        assert context is not None
        grant = {
            ("research", "cancel"): "workflow.research.cancel",
            ("research", "retry"): "workflow.research.retry",
            ("saved_search", "pause"): "workflow.saved.control",
            ("saved_search", "resume"): "workflow.saved.control",
            ("saved_search", "ack"): "workflow.saved.ack",
            ("staged_search", "compose"): "workflow.compose",
        }.get((kind, action))
        if grant is None or grant not in current.mutations_enabled or not context.allows(grant):
            if grant is not None:
                await _audit(
                    current,
                    request,
                    grant,
                    "denied",
                    "authorization_denied",
                    context,
                    object_kind=kind,
                    object_id=object_id,
                )
            return _problem(403, "Action unavailable")
        if not await _rate_allowed(
            current,
            request,
            "mutation",
            context.principal_id + context.tenant_id + grant,
            limit=30,
            window=60,
        ):
            return _problem(429, "Action temporarily unavailable")
        try:
            values = await _form(request)
            revision = int(values.get("expected_revision", ""))
        except (ValueError, UnicodeDecodeError):
            await _audit(current, request, grant, "denied", "csrf_denied", context)
            return _problem(403, "Request denied")
        if not _same_origin(request, current.external_origin) or not valid_csrf(context, values.get("csrf")):
            await _audit(
                current,
                request,
                grant,
                "denied",
                "csrf_denied",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _problem(403, "Request denied")
        idem = values.get("idempotency_key", "")
        if not 12 <= len(idem) <= 128:
            return _problem(400, "Invalid idempotency key")
        cursor = values.get("cursor", "")
        question = values.get("question", "")
        strategy = values.get("strategy", "triangulate")
        digest = hashlib.sha256(
            f"{kind}\0{object_id}\0{action}\0{revision}\0{cursor}\0{question}\0{strategy}".encode()
        ).hexdigest()
        guard = MutationGuard(current.store)
        status, prior = await guard.claim(context, grant, idem, digest)
        if status == "replay" and prior:
            return RedirectResponse("/workflows", status_code=303)
        if status != "claimed":
            await _audit(
                current,
                request,
                grant,
                "conflict",
                "duplicate",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _problem(409, "Action already submitted or changed")
        try:
            if (kind, action) == ("research", "cancel"):
                state = await current.workflows.cancel_research(context, object_id, revision)
                result = {"state": state}
            elif (kind, action) == ("research", "retry"):
                state = await current.workflows.retry_research(context, object_id, revision)
                result = {"state": state}
            elif (kind, action) in {("saved_search", "pause"), ("saved_search", "resume")}:
                updated = await current.workflows.set_saved_paused(
                    context, object_id, expected_revision=revision, paused=action == "pause"
                )
                result = {"revision": str(updated)}
            elif (kind, action) == ("saved_search", "ack"):
                acknowledged = await current.workflows.acknowledge_saved_events(
                    context, object_id, expected_revision=revision, cursor=cursor
                )
                result = {"acknowledged_cursor": acknowledged}
            else:
                job_id = await current.workflows.compose_research(
                    context,
                    object_id,
                    expected_revision=revision,
                    question=question,
                    strategy=strategy,
                    idempotency_key=hashlib.sha256(f"portal\0{context.principal_id}\0{idem}".encode()).hexdigest(),
                )
                result = {"job_id": job_id}
        except WorkflowNotFoundError:
            await _audit(
                current,
                request,
                grant,
                "denied",
                "object_unavailable",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _not_found()
        except PermissionError:
            await _audit(
                current,
                request,
                grant,
                "denied",
                "authorization_denied",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _problem(403, "Action unavailable")
        except WorkflowConflictError as exc:
            await _audit(
                current,
                request,
                grant,
                "conflict",
                "revision_conflict",
                context,
                object_kind=kind,
                object_id=object_id,
            )
            return _problem(409, f"Workflow changed; current revision is {exc.current_revision}")
        claim_id = str((prior or {}).get("claim_id", ""))
        if not await guard.finish(context, grant, idem, digest, claim_id, result):
            return _problem(409, "Action completion was superseded")
        await _audit(
            current,
            request,
            grant,
            "succeeded",
            "ok",
            context,
            object_kind=kind,
            object_id=object_id,
        )
        return RedirectResponse("/workflows", status_code=303)

    return router
