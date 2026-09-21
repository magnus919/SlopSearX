"""Internet Archive adapter — web archives, books, audio, video.

Free, public API. No auth required.
Wayback CDX API for web archive queries.
General advancedsearch for books/media queries.
"""

from __future__ import annotations

import re
import time
import urllib.parse
from typing import Any

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)

_AVAILABILITY_URL = "https://archive.org/wayback/available"
_SNAPSHOT_PATH_RE = re.compile(r"^/web/(\d{14})/(https?://.+)$")


def _is_domain_query(query: str) -> bool:
    """Detect if query looks like a domain name (for Wayback routing)."""
    return bool(re.match(r"^[\w.-]+\.[a-z]{2,}$", query.strip(), re.IGNORECASE))


@register_engine
class InternetArchiveAdapter(EngineAdapter):
    """Internet Archive search — Wayback Machine, books, media.

    Routes domain-looking queries to the Wayback CDX API.
    Routes everything else to the general archive advancedsearch.
    """

    name = "internetarchive"
    display_name = "Internet Archive"
    env_prefix = "ENGINE_INTERNETARCHIVE"
    engine_type = "api"
    categories = ["reference", "web:archive", "historical"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "blocked", "error", "timeout")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:

        if _is_domain_query(query):
            return await self._search_wayback(query, params)

        return await self._search_archive(query, params)

    async def _search_wayback(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Search CDX, with one bounded Availability API fallback.

        CDX remains authoritative for a history.  The Availability API is only
        consulted after CDX fails and can return one validated ``closest``
        snapshot; it must never be presented as a CDX history.
        """
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)
        domain = query.strip()
        total_timeout = max(0.001, timeout_ms / 1000)
        deadline = time.monotonic() + total_timeout
        # Reserve half of the aggregate budget for the one-request fallback;
        # an exhausted CDX call must not consume the entire search deadline.
        cdx_timeout = max(0.001, total_timeout / 2)

        url = (
            f"https://web.archive.org/cdx/search/cdx"
            f"?url={urllib.parse.quote(domain)}"
            f"&output=json"
            f"&limit={max_results}"
            f"&filter=statuscode:200"
            f"&collapse=timestamp:6"  # one snapshot per month
        )

        try:
            async with self.http_client(timeout=min(cdx_timeout, max(0.001, deadline - time.monotonic()))) as client:
                cdx_response = await client.get(url)
                if cdx_response.status_code != 200:
                    cdx_failure = _http_failure(cdx_response, "CDX")
                else:
                    raw = cdx_response.json()
                    if not isinstance(raw, list) or (raw and not isinstance(raw[0], list)):
                        cdx_failure = AdapterResponse(
                            results=[],
                            status=EngineStatus.ERROR,
                            error_message="CDX returned malformed JSON",
                        )
                    else:
                        # CDX returns: [urlkey, timestamp, original, mimetype, ...]
                        results = []
                        for row in raw[1:]:  # skip header row
                            if not isinstance(row, list) or len(row) < 3:
                                continue
                            ts = str(row[1])
                            original_url = str(row[2])
                            if not re.fullmatch(r"\d{14}", ts) or not original_url:
                                continue
                            mime = row[3] if len(row) > 3 else ""
                            wayback_url = f"https://web.archive.org/web/{ts}/{original_url}"
                            results.append(
                                SearchResult(
                                    url=wayback_url,
                                    title=f"{domain} — {ts[:4]}-{ts[4:6]}-{ts[6:8]} ({mime})",
                                    content=f"Archived snapshot of {original_url} from {ts[:4]}-{ts[4:6]}-{ts[6:8]}",
                                    engine=self.name,
                                    score=0.5,
                                    published_date=f"{ts[:4]}-{ts[4:6]}-{ts[6:8]}",
                                )
                            )
                        return AdapterResponse(results=results, status=EngineStatus.OK)
        except httpx.TimeoutException as exc:
            cdx_failure = AdapterResponse(
                results=[], status=EngineStatus.TIMEOUT, error_message=f"CDX request timed out: {exc}"
            )
        except Exception as exc:  # noqa: BLE001 - adapter contract is never-raises
            cdx_failure = AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=f"CDX request failed: {exc}"
            )

        # The fallback is deliberately one request and one closest snapshot.
        availability_url = f"{_AVAILABILITY_URL}?url={urllib.parse.quote(domain)}"
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message="CDX failed and the aggregate Wayback deadline expired before fallback",
            )
        try:
            async with self.http_client(timeout=remaining) as client:
                availability_response = await client.get(availability_url)
                if availability_response.status_code != 200:
                    return _http_failure(availability_response, "Availability API")
                data = availability_response.json()
        except httpx.TimeoutException as exc:
            return AdapterResponse(
                results=[], status=EngineStatus.TIMEOUT, error_message=f"Availability API timed out: {exc}"
            )
        except Exception as exc:  # noqa: BLE001 - adapter contract is never-raises
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message=f"Availability API failed: {exc}"
            )

        if not isinstance(data, dict):
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message="Availability API returned malformed JSON"
            )
        archived_snapshots = data.get("archived_snapshots")
        if not isinstance(archived_snapshots, dict):
            return AdapterResponse(
                results=[], status=EngineStatus.ERROR, error_message="Availability API returned malformed JSON"
            )
        closest = archived_snapshots.get("closest")
        if not isinstance(closest, dict):
            return AdapterResponse(results=[], status=EngineStatus.OK, error_message="No archived snapshot available")
        if closest.get("available") is not True:
            return AdapterResponse(results=[], status=EngineStatus.OK, error_message="No archived snapshot available")
        if str(closest.get("status")) != "200":
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Availability API returned an unusable snapshot status",
            )

        snapshot = _validated_snapshot(domain, closest)
        if snapshot is None:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Availability API returned an untrusted snapshot URL",
            )
        return AdapterResponse(
            results=[snapshot],
            status=EngineStatus.OK,
            error_message=(
                f"CDX unavailable ({cdx_failure.status.value}); returned one closest snapshot "
                "from the Availability API, not a CDX history"
            ),
        )

    async def _search_archive(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        """Search general Internet Archive catalog."""
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)
        base_url = cfg.get("base_url", "https://archive.org")

        url = f"{base_url}/advancedsearch.php?q={urllib.parse.quote(query)}&output=json&rows={max_results}"

        try:
            async with self.http_client(timeout=timeout_ms / 1000) as client:
                resp = await client.get(url)
                resp.raise_for_status()
                data = resp.json()
        except Exception as exc:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
            )

        docs = data.get("response", {}).get("docs", [])
        results = []
        for doc in docs[:max_results]:
            identifier = doc.get("identifier", "")
            results.append(
                SearchResult(
                    url=f"https://archive.org/details/{identifier}" if identifier else "",
                    title=doc.get("title", ""),
                    content=(doc.get("description") or "")[:500],
                    engine=self.name,
                    score=float(doc.get("downloads", 0)) / 10000.0,
                    published_date=doc.get("date"),
                )
            )

        return AdapterResponse(results=results, status=EngineStatus.OK)


def _http_failure(response: httpx.Response, source: str) -> AdapterResponse:
    """Classify an upstream HTTP failure without exposing response content."""
    if response.status_code == 429:
        status = EngineStatus.RATE_LIMITED
    elif response.status_code in (401, 403):
        status = EngineStatus.BLOCKED
    elif response.status_code == 408:
        status = EngineStatus.TIMEOUT
    else:
        status = EngineStatus.ERROR
    return AdapterResponse(results=[], status=status, error_message=f"{source} returned HTTP {response.status_code}")


def _validated_snapshot(domain: str, closest: dict[str, Any]) -> SearchResult | None:
    """Accept only the official Wayback snapshot identified by the response."""
    raw_url = closest.get("url")
    timestamp = str(closest.get("timestamp", ""))
    if not isinstance(raw_url, str) or not re.fullmatch(r"\d{14}", timestamp):
        return None
    try:
        parsed = urllib.parse.urlparse(raw_url)
        match = _SNAPSHOT_PATH_RE.fullmatch(parsed.path)
        original = urllib.parse.urlparse(match.group(2)) if match else None
        requested_host = domain.strip().lower().rstrip(".")
        if (
            parsed.scheme not in {"http", "https"}
            or parsed.hostname != "web.archive.org"
            or parsed.port is not None
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
            or match is None
            or match.group(1) != timestamp
            or original is None
            or original.scheme not in {"http", "https"}
            or original.hostname != requested_host
        ):
            return None
        normalized_url = urllib.parse.urlunparse(parsed._replace(scheme="https"))
        date = f"{timestamp[:4]}-{timestamp[4:6]}-{timestamp[6:8]}"
        return SearchResult(
            url=normalized_url,
            title=f"{domain} — closest archived snapshot ({date})",
            content=(
                f"One closest archived snapshot of {original.geturl()} from {date}; "
                "the Availability API does not provide a full CDX history."
            ),
            engine="internetarchive",
            score=0.5,
            published_date=date,
        )
    except (TypeError, ValueError):
        return None
