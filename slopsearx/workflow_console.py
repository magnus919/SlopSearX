"""Transport-independent workflow explorer and guarded browser mutations."""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import time
from dataclasses import dataclass
from typing import Any

from slopsearx.artifacts import artifact_ref
from slopsearx.capabilities import MCPPolicy, engine_policy_rejection
from slopsearx.mcp.composition import resolve_source
from slopsearx.mcp.dependency_tools import slopsearx_get_dependency_dossier
from slopsearx.mcp.receipt_tools import slopsearx_export_research_manifest
from slopsearx.mcp.state import McpState, state_scope, tenant_scope
from slopsearx.mcp.tools import (
    slopsearx_cancel_job,
    slopsearx_pause_saved_search,
    slopsearx_read_saved_search_reports,
    slopsearx_retry_research,
    slopsearx_start_research,
)
from slopsearx.portal_auth import PortalAuthContext
from slopsearx.research import ResearchJobRunner
from slopsearx.research_store import JOB_RETENTION_SECONDS, ResearchJobStore
from slopsearx.saved_events import public_event
from slopsearx.saved_models import SavedDefinition
from slopsearx.saved_store import OutboxCapacityError, SavedSearchStore
from slopsearx.staged import StagedSearchStore

READ_ACTION = "workflow.read"
CANCEL_ACTION = "workflow.research.cancel"
RETRY_ACTION = "workflow.research.retry"
SAVED_CONTROL_ACTION = "workflow.saved.control"
SAVED_ACK_ACTION = "workflow.saved.ack"
COMPOSE_ACTION = "workflow.compose"
_INTENT_GRANTS = {"jobs": "jobs", "security": "security", "science": "science"}
_MAX_OVERVIEW_SCAN = 1000


@dataclass(frozen=True)
class WorkflowPage:
    items: list[dict[str, Any]]
    next_cursor: str | None


class WorkflowNotFoundError(LookupError):
    """Uniform object-level denial, including policy and tenant mismatch."""


class WorkflowConflictError(RuntimeError):
    def __init__(self, current_revision: int | None = None) -> None:
        super().__init__("workflow changed")
        self.current_revision = current_revision


class WorkflowConsoleService:
    """Shared application boundary used by the server-rendered console."""

    def __init__(
        self,
        *,
        policy: MCPPolicy,
        jobs: ResearchJobStore,
        research_runner: ResearchJobRunner,
        staged: StagedSearchStore,
        saved: SavedSearchStore,
        cursor_key: bytes,
        composition_state: McpState | None = None,
    ) -> None:
        self.policy = policy
        self.jobs = jobs
        self.research_runner = research_runner
        self.staged = staged
        self.saved = saved
        self._cursor_key = cursor_key
        self.composition_state = composition_state

    @staticmethod
    def _consumer_id(context: PortalAuthContext) -> str:
        return "portal-" + hashlib.sha256(context.principal_id.encode()).hexdigest()[:24]

    def _encode_cursor(self, context: PortalAuthContext, marker: tuple[float, str, str]) -> str:
        payload = json.dumps([context.tenant_id, marker[0], marker[1], marker[2]], separators=(",", ":")).encode()
        signature = hmac.new(self._cursor_key, payload, hashlib.sha256).digest()[:16]
        return base64.urlsafe_b64encode(payload + signature).decode().rstrip("=")

    def _decode_cursor(self, context: PortalAuthContext, cursor: str | None) -> tuple[float, str, str] | None:
        if not cursor:
            return None
        try:
            raw = base64.urlsafe_b64decode(cursor + "=" * (-len(cursor) % 4))
            payload, signature = raw[:-16], raw[-16:]
            expected = hmac.new(self._cursor_key, payload, hashlib.sha256).digest()[:16]
            tenant, created_at, object_id, kind = json.loads(payload)
            if not hmac.compare_digest(signature, expected) or tenant != context.tenant_id:
                raise ValueError
            return float(created_at), str(object_id), str(kind)
        except (ValueError, TypeError, json.JSONDecodeError) as exc:
            raise WorkflowNotFoundError from exc

    def _policy_allows(self, kind: str) -> bool:
        grants = {
            "research": "research",
            "dependency_dossier": "dependency_dossier",
            "staged_search": "staged_search",
            "saved_search": "saved_searches",
        }
        grant = grants.get(kind)
        return bool(grant and self.policy.tool_enabled(grant))

    def _engines_allowed(self, engines: list[str]) -> bool:
        if self.composition_state is not None:
            return engine_policy_rejection(self.composition_state.catalog, self.policy, engines) is None
        return self.policy.targeted_sensitive_allowed or not (set(engines) & self.policy.sensitive_engines)

    def _job_allowed(self, job: Any, kind: str) -> bool:
        if not self._policy_allows(kind):
            return False
        if kind == "dependency_dossier" and any(
            not self.policy.tool_enabled(grant) for grant in ("dependency_dossier", "research", "security")
        ):
            return False
        for query in job.queries:
            if not self._engines_allowed(list(query.engines)):
                return False
            grant = _INTENT_GRANTS.get(query.intent)
            if query.requires_intent_grant and grant and not self.policy.tool_enabled(grant):
                return False
        return True

    @staticmethod
    def _job_item(job: Any) -> dict[str, Any]:
        kind = "dependency_dossier" if job.workflow.get("kind") == "dependency_dossier" else "research"
        revision_payload = [
            job.state,
            job.cancel_requested,
            [
                [query.index, query.state, [[attempt.attempt_id, attempt.state] for attempt in query.attempts]]
                for query in job.queries
            ],
        ]
        revision = int(
            hashlib.sha256(json.dumps(revision_payload, separators=(",", ":")).encode()).hexdigest()[:12], 16
        )
        return {
            "kind": kind,
            "id": job.job_id,
            "state": job.state,
            "created_at": job.created_at,
            "expires_at": job.created_at + JOB_RETENTION_SECONDS,
            "revision": revision,
            "summary": f"{len(job.queries)} subquestions",
        }

    @staticmethod
    def _staged_engines(record: dict[str, Any]) -> list[str]:
        return [
            str(engine)
            for stage in record.get("stages") or []
            for engine in (stage.get("scope") or {}).get("selected_engines") or []
        ]

    @staticmethod
    def _staged_item(record: dict[str, Any]) -> dict[str, Any]:
        return {
            "kind": "staged_search",
            "id": str(record["operation_id"]),
            "state": str(record.get("state", "unknown")),
            "created_at": float(record.get("accepted_at", 0)),
            "expires_at": float(record.get("expires_at", 0)),
            "revision": len(record.get("retry_keys") or []) + len(record.get("stages") or []),
            "summary": f"{len(record.get('stages') or [])} stages",
        }

    @staticmethod
    def _saved_item(definition: SavedDefinition) -> dict[str, Any]:
        return {
            "kind": "saved_search",
            "id": definition.search_id,
            "state": "paused" if definition.paused else "active",
            "created_at": definition.created_at,
            "expires_at": definition.expires_at,
            "revision": definition.revision,
            "summary": f"Every {definition.interval_seconds} seconds",
        }

    async def list(self, context: PortalAuthContext, cursor: str | None, limit: int = 20) -> WorkflowPage:
        if not context.allows(READ_ACTION) or not 1 <= limit <= 50:
            raise PermissionError
        marker = self._decode_cursor(context, cursor)
        candidates: list[dict[str, Any]] = []
        if self.policy.tool_enabled("research") or self.policy.tool_enabled("dependency_dossier"):
            before: tuple[float, str] | None = None
            scanned = 0
            while scanned < _MAX_OVERVIEW_SCAN:
                page_limit = min(200, _MAX_OVERVIEW_SCAN - scanned)
                jobs = await self.jobs.for_tenant(context.tenant_id).list_recent(before=before, limit=page_limit)
                if not jobs:
                    break
                scanned += len(jobs)
                candidates.extend(
                    self._job_item(job) for job in jobs if self._job_allowed(job, str(self._job_item(job)["kind"]))
                )
                if len(jobs) < page_limit:
                    break
                before = (float(jobs[-1].created_at), str(jobs[-1].job_id))
        if self.policy.tool_enabled("staged_search"):
            scanned = 0
            before = None
            while scanned < _MAX_OVERVIEW_SCAN:
                page_limit = min(50, _MAX_OVERVIEW_SCAN - scanned)
                records = await self.staged.list_recent(context.tenant_id, before=before, limit=page_limit)
                if not records:
                    break
                scanned += len(records)
                allowed_records = [record for record in records if self._engines_allowed(self._staged_engines(record))]
                candidates.extend(self._staged_item(record) for record in allowed_records)
                if len(records) < page_limit:
                    break
                before = (float(records[-1].get("accepted_at", 0)), str(records[-1].get("operation_id", "")))
        if self.policy.tool_enabled("saved_searches"):
            scanned = 0
            before = None
            while scanned < _MAX_OVERVIEW_SCAN:
                page_limit = min(200, _MAX_OVERVIEW_SCAN - scanned)
                definitions = await self.saved.for_tenant(context.tenant_id).list_recent(
                    before=before, limit=page_limit
                )
                if not definitions:
                    break
                scanned += len(definitions)
                allowed_definitions = [item for item in definitions if self._engines_allowed(item.engines)]
                candidates.extend(self._saved_item(item) for item in allowed_definitions)
                if len(definitions) < page_limit:
                    break
                before = (float(definitions[-1].created_at), str(definitions[-1].search_id))
        candidates.sort(key=lambda item: (item["created_at"], item["id"], item["kind"]), reverse=True)
        if marker is not None:
            candidates = [item for item in candidates if (item["created_at"], item["id"], item["kind"]) < marker]
        page = candidates[:limit]
        next_cursor = None
        if len(candidates) > limit and page:
            last = page[-1]
            next_cursor = self._encode_cursor(context, (float(last["created_at"]), str(last["id"]), str(last["kind"])))
        return WorkflowPage(page, next_cursor)

    async def detail(self, context: PortalAuthContext, kind: str, object_id: str) -> dict[str, Any]:
        if not context.allows(READ_ACTION) or not self._policy_allows(kind):
            raise WorkflowNotFoundError
        if kind in {"research", "dependency_dossier"}:
            job = await self.jobs.for_tenant(context.tenant_id).load(object_id)
            actual_kind = (
                "dependency_dossier" if job and job.workflow.get("kind") == "dependency_dossier" else "research"
            )
            if job is None or actual_kind != kind or not self._job_allowed(job, kind):
                raise WorkflowNotFoundError
            item = self._job_item(job)
            item["details"] = {
                "budget": {"limits": dict(job.budget_limits), "used": dict(job.budget_used)},
                "stop_reason": job.stop_reason,
                "subquestions": [
                    {
                        "index": query.index,
                        "state": query.state,
                        "attempts": [{"state": attempt.state, "cursor": attempt.cursor} for attempt in query.attempts],
                    }
                    for query in job.queries
                ],
                "lineage": list(job.workflow.get("lineage") or []),
            }
            source_redacted = False
            if self.composition_state is not None:
                composition = job.workflow.get("composition")
                source = composition.get("source") if isinstance(composition, dict) else None
                source_artifact = source.get("artifact") if isinstance(source, dict) else None
                if source_artifact is not None:
                    with state_scope(self.composition_state), tenant_scope(context.tenant_id):
                        resolved_source = await resolve_source(source_artifact, kind)
                    if isinstance(resolved_source, dict):
                        error = resolved_source.get("error", {})
                        item["details"]["source_status"] = str(error.get("code", "unavailable"))
                        source_redacted = True
                        item["details"]["lineage"] = []
            if (
                kind == "research"
                and self.policy.tool_enabled("retrieval_receipts")
                and self.composition_state is not None
                and not source_redacted
            ):
                with state_scope(self.composition_state), tenant_scope(context.tenant_id):
                    item["details"]["research_manifest"] = await slopsearx_export_research_manifest(
                        source=artifact_ref("research_job", object_id), max_nodes=50
                    )
            if kind == "dependency_dossier" and self.composition_state is not None:
                with state_scope(self.composition_state), tenant_scope(context.tenant_id):
                    dossier = await slopsearx_get_dependency_dossier(
                        object_id, max_results=min(20, self.policy.max_results)
                    )
                if "error" in dossier:
                    raise WorkflowNotFoundError
                if source_redacted:
                    dossier.pop("source", None)
                    dossier["lineage"] = []
                    dossier["source_status"] = item["details"].get("source_status", "source_unavailable")
                item["details"] = dossier
            return item
        if kind == "staged_search":
            read = await self.staged.read(context.tenant_id, object_id)
            if read.record is None:
                raise WorkflowNotFoundError
            stage_engines = self._staged_engines(read.record)
            if not self._engines_allowed(stage_engines):
                raise WorkflowNotFoundError
            item = self._staged_item(read.record)
            item["details"] = {
                "budget": dict(read.record.get("budget") or {}),
                "stop_reason": read.record.get("stop_reason"),
                "stages": list(read.record.get("stages") or []),
                "lineage": list(read.record.get("lineage") or []),
            }
            return item
        if kind == "saved_search":
            definition = await self.saved.for_tenant(context.tenant_id).load(object_id)
            if definition is None or not self._engines_allowed(definition.engines):
                raise WorkflowNotFoundError
            item = self._saved_item(definition)
            saved_store = self.saved.for_tenant(context.tenant_id)
            if self.composition_state is None:
                raise WorkflowNotFoundError
            with state_scope(self.composition_state), tenant_scope(context.tenant_id):
                report_result = await slopsearx_read_saved_search_reports(
                    object_id, limit=min(20, self.policy.saved_max_reports)
                )
            if "error" in report_result:
                raise WorkflowNotFoundError
            reports = list(report_result["reports"])
            event_batch = None
            if self.policy.tool_enabled("saved_search_events"):
                raw_batch = await saved_store.read_events(
                    self._consumer_id(context), cursor=None, limit=min(100, self.policy.saved_event_capacity)
                )
                events = []
                for event_cursor, event in raw_batch.pop("events"):
                    engines = event.get("_policy_engines")
                    redacted = not isinstance(engines, list) or not self._engines_allowed(
                        [str(engine) for engine in engines]
                    )
                    events.append(public_event(event, event_cursor, redacted=redacted))
                event_batch = {
                    **raw_batch,
                    "scope": "tenant_inbox",
                    "events": events,
                    "returned": len(events),
                }
            item["details"] = {
                "next_due": definition.next_due,
                "reports": reports,
                "lineage": [report.get("lineage") for report in reports if report.get("lineage")],
                "event_batch": event_batch,
            }
            return item
        raise WorkflowNotFoundError

    async def cancel_research(self, context: PortalAuthContext, object_id: str, expected_revision: int) -> str:
        if (
            not context.allows(CANCEL_ACTION)
            or not self.policy.tool_enabled("research")
            or self.composition_state is None
        ):
            raise PermissionError
        detail = await self.detail(context, "research", object_id)
        if detail["revision"] != expected_revision:
            raise WorkflowConflictError(detail["revision"])
        with state_scope(self.composition_state), tenant_scope(context.tenant_id):
            result = await slopsearx_cancel_job(object_id)
        if "error" in result:
            raise WorkflowNotFoundError
        return str(result["state"])

    async def retry_research(self, context: PortalAuthContext, object_id: str, expected_revision: int) -> str:
        if (
            not context.allows(RETRY_ACTION)
            or not self.policy.tool_enabled("research")
            or self.composition_state is None
        ):
            raise PermissionError
        detail = await self.detail(context, "research", object_id)
        if detail["revision"] != expected_revision:
            raise WorkflowConflictError(detail["revision"])
        with state_scope(self.composition_state), tenant_scope(context.tenant_id):
            result = await slopsearx_retry_research(object_id)
        if "error" in result:
            code = str(result["error"].get("code", ""))
            if code in {"invalid_job_id", "policy_rejected", "tool_disabled"}:
                raise WorkflowNotFoundError
            raise WorkflowConflictError(expected_revision)
        return str(result["state"])

    async def set_saved_paused(
        self,
        context: PortalAuthContext,
        object_id: str,
        *,
        expected_revision: int,
        paused: bool,
    ) -> int:
        if (
            not context.allows(SAVED_CONTROL_ACTION)
            or not self.policy.tool_enabled("saved_searches")
            or self.composition_state is None
        ):
            raise PermissionError
        store = self.saved.for_tenant(context.tenant_id)
        definition = await store.load(object_id)
        if definition is None or not self._engines_allowed(definition.engines):
            raise WorkflowNotFoundError
        if definition.revision != expected_revision:
            raise WorkflowConflictError(definition.revision)
        with state_scope(self.composition_state), tenant_scope(context.tenant_id):
            result = await slopsearx_pause_saved_search(object_id, expected_revision, paused)
        if "error" in result:
            error = result["error"]
            code = str(error.get("code", ""))
            if code in {"invalid_search_id", "policy_rejected", "tool_disabled"}:
                raise WorkflowNotFoundError
            raise WorkflowConflictError(error.get("current_revision"))
        return int(result["revision"])

    async def acknowledge_saved_events(
        self, context: PortalAuthContext, object_id: str, *, expected_revision: int, cursor: str
    ) -> str:
        if not context.allows(SAVED_ACK_ACTION) or not self.policy.tool_enabled("saved_search_events"):
            raise PermissionError
        detail = await self.detail(context, "saved_search", object_id)
        if detail["revision"] != expected_revision:
            raise WorkflowConflictError(detail["revision"])
        try:
            return await self.saved.for_tenant(context.tenant_id).acknowledge_event(
                self._consumer_id(context), cursor, now=time.time()
            )
        except (LookupError, OutboxCapacityError, ValueError) as exc:
            raise WorkflowConflictError(expected_revision) from exc

    async def compose_research(
        self,
        context: PortalAuthContext,
        object_id: str,
        *,
        expected_revision: int,
        question: str,
        strategy: str,
        idempotency_key: str,
    ) -> str:
        if (
            not context.allows(COMPOSE_ACTION)
            or not self.policy.tool_enabled("research")
            or self.composition_state is None
        ):
            raise PermissionError
        detail = await self.detail(context, "staged_search", object_id)
        if detail["revision"] != expected_revision:
            raise WorkflowConflictError(detail["revision"])
        with state_scope(self.composition_state), tenant_scope(context.tenant_id):
            result = await slopsearx_start_research(
                question=question,
                strategy=strategy,
                idempotency_key=idempotency_key,
                source=artifact_ref("staged_search", object_id),
            )
        error = result.get("error")
        if isinstance(error, dict):
            if error.get("code") in {"source_not_found", "source_expired", "source_policy_denied"}:
                raise WorkflowNotFoundError
            raise WorkflowConflictError(expected_revision)
        job_id = result.get("job_id")
        if not isinstance(job_id, str):
            raise WorkflowConflictError(expected_revision)
        return job_id
