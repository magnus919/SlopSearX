"""Tests for the snapshot store (opaque cursors over captured evidence)."""

from __future__ import annotations

from typing import Any

from slopsearx.adapter import SearchResult
from slopsearx.service import ScopeDecision
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}
        self.set_ttls: list[int] = []

    async def get(self, key: str) -> dict[str, Any] | None:
        if not self.is_connected:
            return None
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        if self.is_connected:
            self._data[key] = value
            self.set_ttls.append(ttl)


def _results(count: int = 3) -> list[SearchResult]:
    return [
        SearchResult(
            url=f"https://example.com/{i}",
            title=f"Title {i}",
            content=f"Content {i}.",
            engine="brave",
            engines={"brave", "wikipedia"} if i == 0 else {"brave"},
            score=2.0 if i == 0 else 1.0,
            position=i + 1,
            category="general",
            published_date="2026-01-01",
            tier=1,
        )
        for i in range(count)
    ]


class TestSnapshotStore:
    async def test_create_and_get_round_trip(self) -> None:
        store = _FakeStore()
        snapshots = SnapshotStore(store, tenant="t1", ttl_seconds=120)

        snapshot_id = await snapshots.create("query", "ssx-abc", _results(), ScopeDecision(selected_engines=["brave"]))

        assert snapshot_id is not None
        snapshot = await snapshots.get(snapshot_id)
        assert snapshot is not None
        assert snapshot.query == "query"
        assert snapshot.query_id == "ssx-abc"
        assert snapshot.total == 3
        assert snapshot.tenant == "t1"
        assert snapshot.results[0].engines == {"brave", "wikipedia"}
        assert snapshot.results[0].published_date == "2026-01-01"
        assert store.set_ttls == [120]

    async def test_ttl_applied(self) -> None:
        store = _FakeStore()
        snapshots = SnapshotStore(store, ttl_seconds=3600)
        await snapshots.create("q", "ssx-1", _results(1), ScopeDecision())
        assert store.set_ttls == [3600]

    async def test_unknown_snapshot_returns_none(self) -> None:
        store = _FakeStore()
        snapshots = SnapshotStore(store)
        assert await snapshots.get("snap-missing") is None

    async def test_unavailable_store_returns_none(self) -> None:
        store = _FakeStore(connected=False)
        snapshots = SnapshotStore(store)
        assert await snapshots.create("q", "ssx-1", _results(), ScopeDecision()) is None
        assert await snapshots.get("snap-anything") is None

    async def test_tenant_boundary(self) -> None:
        store = _FakeStore()
        store_a = SnapshotStore(store, tenant="a")
        store_b = SnapshotStore(store, tenant="b")

        snapshot_id = await store_a.create("q", "ssx-1", _results(1), ScopeDecision())

        # Tenant B must not see tenant A's snapshot
        assert await store_b.get(snapshot_id) is None

    async def test_result_id_format(self) -> None:
        snapshots = SnapshotStore(_FakeStore())
        assert snapshots.result_id("snap-abc", 3) == "snap-abc:3"

    async def test_empty_results_snapshot(self) -> None:
        store = _FakeStore()
        snapshots = SnapshotStore(store)
        snapshot_id = await snapshots.create("q", "ssx-1", [], ScopeDecision())
        snapshot = await snapshots.get(snapshot_id)
        assert snapshot is not None
        assert snapshot.total == 0
        assert snapshot.results == []
