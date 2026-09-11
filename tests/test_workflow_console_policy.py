from __future__ import annotations

from types import SimpleNamespace
from typing import Any

import pytest

import slopsearx.workflow_console as console_mod
from slopsearx.capabilities import MCPPolicy
from slopsearx.portal_auth import PortalAuthContext
from slopsearx.research import ResearchJob, ResearchQuery
from slopsearx.saved_models import SavedDefinition
from slopsearx.workflow_console import WorkflowConsoleService, WorkflowNotFoundError


def context(*grants: str) -> PortalAuthContext:
    return PortalAuthContext("principal", "tenant", "Tenant", frozenset(grants), 1, 1, 1, "csrf")


def saved(search_id: str, created_at: float, engines: list[str]) -> SavedDefinition:
    return SavedDefinition(
        search_id=search_id,
        tenant="tenant",
        query="q",
        engines=engines,
        interval_seconds=60,
        retention_seconds=300,
        max_results=10,
        max_reports=5,
        created_at=created_at,
        expires_at=created_at + 10_000,
        next_due=created_at + 60,
    )


class EmptyJobs:
    def for_tenant(self, _tenant: str) -> EmptyJobs:
        return self

    async def list_recent(self, **_kwargs: Any) -> list[Any]:
        return []

    async def load(self, _object_id: str) -> Any:
        return None


class FakeStaged:
    def __init__(self, records: list[dict[str, Any]]) -> None:
        self.records = records

    async def list_recent(
        self, _tenant: str, *, before: tuple[float, str] | None = None, limit: int = 20
    ) -> list[dict[str, Any]]:
        rows = self.records
        if before is not None:
            rows = [row for row in rows if (float(row["accepted_at"]), str(row["operation_id"])) < before]
        return rows[:limit]

    async def read(self, _tenant: str, object_id: str) -> Any:
        record = next((row for row in self.records if row["operation_id"] == object_id), None)
        return type("Read", (), {"record": record})()


class FakeSaved:
    def __init__(self, definitions: list[SavedDefinition]) -> None:
        self.definitions = definitions

    def for_tenant(self, _tenant: str) -> FakeSaved:
        return self

    async def list_recent(
        self, *, before: tuple[float, str] | None = None, limit: int = 20, **_kwargs: Any
    ) -> list[SavedDefinition]:
        rows = self.definitions
        if before is not None:
            rows = [row for row in rows if (row.created_at, row.search_id) < before]
        return rows[:limit]

    async def load(self, object_id: str) -> SavedDefinition | None:
        return next((row for row in self.definitions if row.search_id == object_id), None)


def service(
    policy: MCPPolicy, staged: FakeStaged, saved_store: FakeSaved, *, shared_state: Any = None
) -> WorkflowConsoleService:
    return WorkflowConsoleService(
        policy=policy,
        jobs=EmptyJobs(),  # type: ignore[arg-type]
        research_runner=object(),  # type: ignore[arg-type]
        staged=staged,  # type: ignore[arg-type]
        saved=saved_store,  # type: ignore[arg-type]
        cursor_key=b"c" * 32,
        composition_state=shared_state,
    )


def shared_state() -> Any:
    class Catalog:
        def known_names(self) -> set[str]:
            return {"wikipedia", "secret"}

        def get(self, _name: str) -> Any:
            return SimpleNamespace(enabled=True)

    return SimpleNamespace(catalog=Catalog())


async def test_overview_fills_around_revoked_rows_and_cursor_disambiguates_kind() -> None:
    policy = MCPPolicy(
        enabled_tools={"staged_search": True, "saved_searches": True},
        sensitive_engines={"secret"},
    )
    definitions = [saved(f"revoked-{index:03d}", 2000 - index, ["secret"]) for index in range(205)]
    definitions.extend([saved("same", 900, ["wikipedia"]), saved("older", 899, ["wikipedia"])])
    staged = FakeStaged(
        [
            {
                "operation_id": "same",
                "accepted_at": 900,
                "expires_at": 2000,
                "state": "succeeded",
                "stages": [{"scope": {"selected_engines": ["wikipedia"]}}],
            },
            *[
                {
                    "operation_id": f"revoked-stage-{index:03d}",
                    "accepted_at": 1900 - index,
                    "expires_at": 3000,
                    "state": "succeeded",
                    "stages": [{"scope": {"selected_engines": ["secret"]}}],
                }
                for index in range(50)
            ],
        ]
    )
    staged.records.sort(key=lambda row: (float(row["accepted_at"]), str(row["operation_id"])), reverse=True)
    console = service(policy, staged, FakeSaved(definitions))
    first = await console.list(context("workflow.read"), None, limit=1)
    second = await console.list(context("workflow.read"), first.next_cursor, limit=1)
    third = await console.list(context("workflow.read"), second.next_cursor, limit=1)
    assert [(first.items[0]["kind"], first.items[0]["id"]), (second.items[0]["kind"], second.items[0]["id"])] == [
        ("staged_search", "same"),
        ("saved_search", "same"),
    ]
    assert third.items[0]["id"] == "older"
    assert all("revoked" not in item["id"] for page in (first, second, third) for item in page.items)


async def test_saved_control_rechecks_policy_then_uses_authoritative_function(monkeypatch: pytest.MonkeyPatch) -> None:
    policy = MCPPolicy(enabled_tools={"saved_searches": True}, sensitive_engines=set())
    definition = saved("saved", 1000, ["secret"])
    console = service(policy, FakeStaged([]), FakeSaved([definition]), shared_state=shared_state())
    calls: list[tuple[str, int, bool]] = []

    async def pause(search_id: str, expected_revision: int, paused: bool) -> dict[str, Any]:
        calls.append((search_id, expected_revision, paused))
        return {"search_id": search_id, "revision": 2}

    monkeypatch.setattr(console_mod, "slopsearx_pause_saved_search", pause)
    assert (
        await console.set_saved_paused(context("workflow.saved.control"), "saved", expected_revision=1, paused=True)
        == 2
    )
    assert calls == [("saved", 1, True)]
    policy.sensitive_engines.add("secret")
    with pytest.raises(WorkflowNotFoundError):
        await console.set_saved_paused(context("workflow.saved.control"), "saved", expected_revision=1, paused=False)
    assert len(calls) == 1


async def test_composed_research_redacts_policy_revoked_source_and_manifest(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    policy = MCPPolicy(
        enabled_tools={"research": True, "retrieval_receipts": True},
        sensitive_engines={"secret"},
    )
    job = ResearchJob(
        "job",
        "question",
        "broad",
        queries=[ResearchQuery(0, "q", "web", ["wikipedia"])],
        workflow={
            "composition": {
                "source": {
                    "artifact": {"kind": "staged_search", "id": "revoked-source-id"},
                    "status": "live",
                    "result_ids": [],
                }
            },
            "lineage": [
                {
                    "from": {"kind": "research_job", "id": "job"},
                    "relation": "derived_from",
                    "to": {"kind": "staged_search", "id": "revoked-source-id"},
                }
            ],
        },
    )
    jobs = EmptyJobs()

    async def load(_object_id: str) -> ResearchJob:
        return job

    jobs.load = load  # type: ignore[method-assign]
    console = service(policy, FakeStaged([]), FakeSaved([]), shared_state=shared_state())
    console.jobs = jobs  # type: ignore[assignment]

    async def denied(_source: Any, _destination: str) -> dict[str, Any]:
        return {"error": {"code": "source_policy_denied"}}

    async def manifest(**_kwargs: Any) -> dict[str, Any]:
        raise AssertionError("manifest must not expose a policy-revoked source")

    monkeypatch.setattr(console_mod, "resolve_source", denied)
    monkeypatch.setattr(console_mod, "slopsearx_export_research_manifest", manifest)
    detail = await console.detail(context("workflow.read"), "research", "job")
    assert detail["details"]["lineage"] == []
    assert "revoked-source-id" not in str(detail)


async def test_dossier_and_saved_reports_use_authoritative_redacted_readers(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    policy = MCPPolicy(
        enabled_tools={
            "research": True,
            "dependency_dossier": True,
            "security": True,
            "saved_searches": True,
        }
    )
    job = ResearchJob("dossier", "question", "broad", workflow={"kind": "dependency_dossier"})
    jobs = EmptyJobs()

    async def load(_object_id: str) -> ResearchJob:
        return job

    jobs.load = load  # type: ignore[method-assign]
    definitions = FakeSaved([saved("saved", 1000, ["wikipedia"])])
    console = service(policy, FakeStaged([]), definitions, shared_state=shared_state())
    console.jobs = jobs  # type: ignore[assignment]

    async def dossier(_job_id: str, max_results: int | None = None) -> dict[str, Any]:
        assert max_results == 20
        return {
            "sections": {"advisory_leads": {"state": "available"}},
            "resolved_package_identity": {"status": "resolved"},
            "suggested_followup_searches": ["verify"],
            "limitations": ["bounded evidence"],
        }

    async def denied_reports(_search_id: str, limit: int = 20) -> dict[str, Any]:
        assert limit == 20
        return {"error": {"code": "policy_rejected"}}

    monkeypatch.setattr(console_mod, "slopsearx_get_dependency_dossier", dossier)
    monkeypatch.setattr(console_mod, "slopsearx_read_saved_search_reports", denied_reports)
    detail = await console.detail(context("workflow.read"), "dependency_dossier", "dossier")
    assert "resolved_package_identity" in detail["details"]
    with pytest.raises(WorkflowNotFoundError):
        await console.detail(context("workflow.read"), "saved_search", "saved")
