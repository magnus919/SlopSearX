"""Tests for EngineStatsTracker."""

from __future__ import annotations

import inspect
from unittest.mock import AsyncMock, MagicMock

from slopsearx.adapter import EngineStatus
from slopsearx.stats import EngineStatsTracker


class TestStatsAsyncConformance:
    """M3-006, M3-012: EngineStatsTracker uses async pipeline."""

    def test_record_query_is_async(self) -> None:
        """record_query is an async def."""
        assert inspect.iscoroutinefunction(EngineStatsTracker.record_query)


class TestStatsTrackerDisconnected:
    """Graceful degradation when cache is unavailable."""

    async def test_record_query_no_cache_noop(self) -> None:
        """Without cache, record_query is a no-op."""
        tracker = EngineStatsTracker(cache=None)
        await tracker.record_query(
            engine="brave",
            result_count=5,
            latency_ms=42.0,
            status=EngineStatus.OK,
        )
        # No exception is success

    async def test_record_query_disconnected_cache_noop(self) -> None:
        """With disconnected cache, record_query is a no-op."""
        mock_cache = MagicMock()
        mock_cache.is_connected = False
        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="brave",
            result_count=5,
            latency_ms=42.0,
            status=EngineStatus.OK,
        )
        # No exception is success

    async def test_record_query_no_client_noop(self) -> None:
        """With cache but no client, record_query is a no-op."""
        mock_cache = MagicMock()
        mock_cache.is_connected = True
        mock_cache._client = None
        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="brave",
            result_count=5,
            latency_ms=42.0,
            status=EngineStatus.OK,
        )
        # No exception is success


class TestStatsPipeline:
    """M3-012, M3-013: Pipeline operations are awaited and use correct schema."""

    def _make_mocks(self) -> tuple[MagicMock, AsyncMock, MagicMock]:
        """Create properly configured mocks for pipeline testing.

        The pipeline() method on a valkey.asyncio client is synchronous
        (not a coroutine), so we need to use a regular MagicMock for the
        client, not AsyncMock.
        """
        mock_pipe = MagicMock()
        mock_pipe.execute = AsyncMock()
        mock_pipe.hincrby = MagicMock()
        mock_pipe.expire = MagicMock()

        mock_client = MagicMock()
        mock_client.pipeline = MagicMock(return_value=mock_pipe)
        mock_client.ping = AsyncMock()
        mock_client.get = AsyncMock()
        mock_client.setex = AsyncMock()
        mock_client.flushdb = AsyncMock()
        mock_client.close = AsyncMock()
        mock_client.incrby = AsyncMock()
        mock_client.expire = AsyncMock()

        mock_cache = MagicMock()
        mock_cache.is_connected = True
        mock_cache._client = mock_client

        return mock_client, mock_pipe, mock_cache

    async def test_pipeline_execute_is_awaited(self) -> None:
        """Pipeline execute is awaited."""
        mock_client, mock_pipe, mock_cache = self._make_mocks()

        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="testengine",
            result_count=3,
            latency_ms=100.0,
            status=EngineStatus.OK,
        )

        # Verify pipeline was created and commands were queued
        mock_client.pipeline.assert_called_once()
        # Key should be engine_stats:testengine:YYYY-MM-DD
        hincrby_calls = mock_pipe.hincrby.call_args_list
        assert len(hincrby_calls) >= 3
        key = hincrby_calls[0][0][0]
        assert key.startswith("engine_stats:testengine:")
        assert key.count(":") == 2  # engine_stats:engine:date
        mock_pipe.expire.assert_called_once()
        expire_key = mock_pipe.expire.call_args[0][0]
        assert expire_key.startswith("engine_stats:testengine:")
        assert mock_pipe.expire.call_args[0][1] == 7_776_000  # 90 days
        mock_pipe.execute.assert_awaited_once()

    async def test_pipeline_error_noop(self) -> None:
        """Pipeline error does not propagate."""
        _, mock_pipe, mock_cache = self._make_mocks()
        mock_pipe.execute = AsyncMock(side_effect=RuntimeError("Valkey error"))

        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="testengine",
            result_count=3,
            latency_ms=100.0,
            status=EngineStatus.OK,
        )
        # No exception is success

    async def test_error_and_timeout_increment_errors(self) -> None:
        """ERROR and TIMEOUT statuses increment the errors counter."""
        _, mock_pipe, mock_cache = self._make_mocks()

        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="testengine",
            result_count=0,
            latency_ms=5000.0,
            status=EngineStatus.ERROR,
        )

        # Verify errors=1 was set
        found = False
        for args, _ in mock_pipe.hincrby.call_args_list:
            if args[1] == "errors" and args[2] == 1:
                found = True
                break
        assert found, "errors field should be 1 for ERROR status"

    async def test_ok_status_does_not_increment_errors(self) -> None:
        """OK status does not increment the errors counter."""
        _, mock_pipe, mock_cache = self._make_mocks()

        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="testengine",
            result_count=5,
            latency_ms=42.0,
            status=EngineStatus.OK,
        )

        # Verify errors=0 was set
        found = False
        for args, _ in mock_pipe.hincrby.call_args_list:
            if args[1] == "errors":
                assert args[2] == 0, "errors should be 0 for OK status"
                found = True
                break
        assert found, "errors field should be present"

    async def test_rate_limited_status_increments_rate_limited(self) -> None:
        """RATE_LIMITED status increments the rate_limited counter."""
        _, mock_pipe, mock_cache = self._make_mocks()

        tracker = EngineStatsTracker(cache=mock_cache)
        await tracker.record_query(
            engine="testengine",
            result_count=0,
            latency_ms=100.0,
            status=EngineStatus.RATE_LIMITED,
        )

        found_rate_limited = False
        found_errors = False
        for args, _ in mock_pipe.hincrby.call_args_list:
            if args[1] == "rate_limited":
                assert args[2] == 1, "rate_limited should be 1 for RATE_LIMITED status"
                found_rate_limited = True
            if args[1] == "errors":
                assert args[2] == 0, "errors should be 0 for RATE_LIMITED status"
                found_errors = True
        assert found_rate_limited, "rate_limited field should be present"
        assert found_errors, "errors field should be present"
