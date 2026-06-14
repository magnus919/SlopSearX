"""Tests for QueryAuditLogger."""

from __future__ import annotations

import inspect
from unittest.mock import AsyncMock, MagicMock

from slopsearx.adapter import AdapterResponse, EngineStatus, SearchResult
from slopsearx.audit import QueryAuditLogger


class TestAuditDisconnected:
    """Graceful degradation when cache is unavailable."""

    async def test_record_query_no_cache_noop(self) -> None:
        """Without cache, record_query is a no-op."""
        logger = QueryAuditLogger(cache=None)
        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results={},
            latency_ms=42.0,
        )
        # No exception is success

    async def test_record_query_disconnected_cache_noop(self) -> None:
        """With disconnected cache, record_query is a no-op."""
        mock_cache = MagicMock()
        mock_cache.is_connected = False
        logger = QueryAuditLogger(cache=mock_cache)
        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results={},
            latency_ms=42.0,
        )
        # No exception is success

    async def test_record_query_no_client_noop(self) -> None:
        """With cache but no client, record_query is a no-op."""
        mock_cache = MagicMock()
        mock_cache.is_connected = True
        del mock_cache._client  # ensure attribute doesn't exist
        logger = QueryAuditLogger(cache=mock_cache)
        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results={},
            latency_ms=42.0,
        )
        # No exception is success


class TestAuditAsyncConformance:
    """All I/O methods are async def."""

    def test_record_query_is_async(self) -> None:
        assert inspect.iscoroutinefunction(QueryAuditLogger.record_query)


class TestAuditPipeline:
    """Pipeline operations and field schema."""

    def _make_engine_results(self) -> dict[str, AdapterResponse]:
        """Create realistic engine results for audit trail testing."""
        return {
            "brave": AdapterResponse(
                results=[
                    SearchResult(
                        url="https://example.com/1",
                        title="Result 1",
                        content="Content 1",
                        engine="brave",
                    ),
                ],
                status=EngineStatus.OK,
                latency_ms=120.0,
            ),
            "wikipedia": AdapterResponse(
                results=[
                    SearchResult(
                        url="https://en.wikipedia.org/wiki/Test",
                        title="Test",
                        content="Test page",
                        engine="wikipedia",
                    ),
                ],
                status=EngineStatus.OK,
                latency_ms=85.0,
            ),
            "google": AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="CAPTCHA",
                latency_ms=3000.0,
            ),
        }

    def _make_mocks(self) -> tuple[MagicMock, MagicMock, QueryAuditLogger]:
        """Create properly configured mocks for pipeline testing."""
        mock_pipe = MagicMock()
        mock_pipe.execute_command = MagicMock()
        mock_pipe.expire = MagicMock()
        mock_pipe.execute = AsyncMock()

        mock_client = MagicMock()
        mock_client.pipeline = MagicMock(return_value=mock_pipe)

        mock_cache = MagicMock()
        mock_cache.is_connected = True
        mock_cache._client = mock_client

        logger = QueryAuditLogger(cache=mock_cache)

        return mock_client, mock_pipe, logger

    async def test_pipeline_execute_is_awaited(self) -> None:
        """Pipeline execute is awaited."""
        _, mock_pipe, logger = self._make_mocks()

        await logger.record_query(
            query="test query",
            client_ip="10.0.0.1",
            engine_results=self._make_engine_results(),
            latency_ms=3200.0,
        )

        # Verify pipeline was created
        mock_pipe.execute.assert_awaited_once()

    async def test_pipeline_fields_correct(self) -> None:
        """XADD field-value pairs contain correct aggregated data."""
        _, mock_pipe, logger = self._make_mocks()

        await logger.record_query(
            query="test query",
            client_ip="10.0.0.1",
            engine_results=self._make_engine_results(),
            latency_ms=3200.0,
        )

        # Verify the XADD call with correct fields
        mock_pipe.execute_command.assert_called_once()
        args = mock_pipe.execute_command.call_args[0]
        assert args[0] == "XADD"
        stream_key = args[1]
        assert stream_key.startswith("query_audit:")
        assert args[2] == "MAXLEN"
        assert args[3] == "~"
        assert args[4] == "10000"
        assert args[5] == "*"

        # Check field-value pairs
        fields = args[6:]
        assert "query" in fields
        assert "client_ip" in fields
        assert "timestamp" in fields
        assert "engines" in fields
        assert "engines_ok" in fields
        assert "engines_error" in fields
        assert "engines_timeout" in fields
        assert "total_results" in fields
        assert "latency_ms" in fields

        # Check aggregated values
        query_idx = fields.index("query")
        assert fields[query_idx + 1] == "test query"
        ip_idx = fields.index("client_ip")
        assert fields[ip_idx + 1] == "10.0.0.1"
        ok_idx = fields.index("engines_ok")
        assert fields[ok_idx + 1] == "2"  # brave + wikipedia
        err_idx = fields.index("engines_error")
        assert fields[err_idx + 1] == "1"  # google
        timeout_idx = fields.index("engines_timeout")
        assert fields[timeout_idx + 1] == "0"
        total_idx = fields.index("total_results")
        assert fields[total_idx + 1] == "2"  # brave(1) + wikipedia(1) + google(0)

    async def test_pipeline_error_noop(self) -> None:
        """Pipeline error does not propagate."""
        _, mock_pipe, logger = self._make_mocks()
        mock_pipe.execute.side_effect = RuntimeError("Valkey error")

        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results=self._make_engine_results(),
            latency_ms=100.0,
        )
        # No exception is success

    async def test_expire_set_on_stream(self) -> None:
        """Stream key gets TTL set for 90-day expiry."""
        _, mock_pipe, logger = self._make_mocks()

        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results=self._make_engine_results(),
            latency_ms=100.0,
        )

        mock_pipe.expire.assert_called_once()
        args = mock_pipe.expire.call_args[0]
        key = args[0]
        assert key.startswith("query_audit:")
        assert args[1] == 7_776_000  # 90 days

    async def test_stream_key_includes_date(self) -> None:
        """Stream key follows the query_audit:YYYY-MM-DD format."""
        _, mock_pipe, logger = self._make_mocks()

        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results={"mock": AdapterResponse(results=[], status=EngineStatus.OK)},
            latency_ms=100.0,
        )

        args = mock_pipe.execute_command.call_args[0]
        stream_key = args[1]
        parts = stream_key.split(":")
        assert len(parts) == 2
        assert parts[0] == "query_audit"
        # Verify date format YYYY-MM-DD
        date_parts = parts[1].split("-")
        assert len(date_parts) == 3
        assert len(date_parts[0]) == 4  # year
        assert len(date_parts[1]) == 2  # month
        assert len(date_parts[2]) == 2  # day

    async def test_count_timeout_status_correctly(self) -> None:
        """TIMEOUT status is counted in engines_timeout, not engines_error."""
        _, mock_pipe, logger = self._make_mocks()

        results = {
            "brave": AdapterResponse(results=[], status=EngineStatus.TIMEOUT, error_message="timeout"),
            "wikipedia": AdapterResponse(
                results=[SearchResult(url="https://wiki", title="", content="", engine="wikipedia")],
                status=EngineStatus.OK,
            ),
        }

        await logger.record_query(
            query="test",
            client_ip="10.0.0.1",
            engine_results=results,
            latency_ms=5000.0,
        )

        args = mock_pipe.execute_command.call_args[0]
        fields = args[6:]
        timeout_idx = fields.index("engines_timeout")
        assert fields[timeout_idx + 1] == "1"
        err_idx = fields.index("engines_error")
        assert fields[err_idx + 1] == "0"
        ok_idx = fields.index("engines_ok")
        assert fields[ok_idx + 1] == "1"
