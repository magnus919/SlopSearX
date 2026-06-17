"""
SlopSearX — Proxy rotation for scrape adapters.

Manages a pool of proxies, cycles through them round-robin, and
tracks failures for cooloff. Designed for DuckDuckGo and Google
scrape adapters that would otherwise be quickly blocked by CAPTCHA
and rate-limiting when connecting from a single IP.
"""

from __future__ import annotations

import itertools
import logging
import time
from typing import Any

logger = logging.getLogger(__name__)

_DEFAULT_COOLOFF_SECONDS = 120


class ProxyPool:
    """Round-robin proxy pool with failure tracking.

    Usage::

        pool = ProxyPool(["http://proxy1:8080", "http://proxy2:8080"])
        proxy = pool.get_proxy()  # returns e.g. {"all://": "http://proxy1:8080"}
        # ... make request ...
        pool.report_failure(proxy)   # marks proxy for cooloff
        pool.report_success(proxy)   # resets failure count
    """

    def __init__(
        self,
        proxies: list[str] | None = None,
        scrape_proxy_url: str = "",
        cooloff_seconds: int = _DEFAULT_COOLOFF_SECONDS,
    ) -> None:
        self._cooloff_seconds = cooloff_seconds
        self._scrape_proxy_url = scrape_proxy_url

        # Internal state
        self._proxies: list[str] = list(proxies) if proxies else []
        self._failures: dict[str, int] = {}  # proxy -> consecutive failures
        self._cooloff_until: dict[str, float] = {}  # proxy -> timestamp
        self._iterator: Any | None = itertools.cycle(self._proxies) if self._proxies else None

    def get_proxy(self) -> dict[str, str] | None:
        """Return the next healthy proxy, or ``None`` if all are exhausted.

        Returns an httpx-compatible proxy dict: ``{"all://": "http://..."}``.
        """
        # If there's a dynamic proxy endpoint, delegate (no local rotation)
        if self._scrape_proxy_url:
            return {"all://": self._scrape_proxy_url}

        if not self._proxies or self._iterator is None:
            return None

        # Try up to N rotations to find a healthy proxy
        for _ in range(len(self._proxies)):
            proxy = next(self._iterator)

            # Check cooloff
            cooloff_until = self._cooloff_until.get(proxy, 0)
            if time.monotonic() < cooloff_until:
                continue

            return {"all://": proxy}

        # All proxies on cooloff — try the next one anyway (fail open)
        proxy = next(self._iterator)
        self._cooloff_until.pop(proxy, None)
        logger.warning("All proxies on cooloff, returning %s anyway", proxy)
        return {"all://": proxy}

    def report_failure(self, proxy_dict: dict[str, str] | None) -> None:
        """Mark a proxy as having failed (CAPTCHA, 429, 403).

        After ``cooloff_seconds``, the proxy may be tried again.
        After 3 consecutive failures, the cooloff duration is tripled.
        """
        if not proxy_dict:
            return
        proxy = self._extract_url(proxy_dict)
        if not proxy or proxy not in self._proxies:
            return

        count = self._failures.get(proxy, 0) + 1
        self._failures[proxy] = count

        # Escalating cooloff: base * (1 if <3 failures else 3)
        multiplier = 1 if count < 3 else 3
        duration = self._cooloff_seconds * multiplier
        self._cooloff_until[proxy] = time.monotonic() + duration

        logger.debug("Proxy %s failed (%dx), cooloff %ds", proxy, count, duration)

    def report_success(self, proxy_dict: dict[str, str] | None) -> None:
        """Reset failure count for a successful proxy request."""
        if not proxy_dict:
            return
        proxy = self._extract_url(proxy_dict)
        if not proxy:
            return
        self._failures.pop(proxy, None)
        self._cooloff_until.pop(proxy, None)

    @property
    def available_count(self) -> int:
        """Number of proxies not currently on cooloff."""
        now = time.monotonic()
        return sum(1 for p in self._proxies if self._cooloff_until.get(p, 0) <= now)

    @property
    def total_count(self) -> int:
        """Total number of proxies in the pool."""
        return len(self._proxies)

    @staticmethod
    def _extract_url(proxy_dict: dict[str, str]) -> str:
        """Extract the actual proxy URL from an httpx proxy dict.

        Handles ``{"all://": "http://proxy:8080"}`` or any scheme key.
        """
        for val in proxy_dict.values():
            if val:
                return val
        return ""

    @classmethod
    def from_config(cls, config: dict[str, Any]) -> ProxyPool | None:
        """Create a ProxyPool from an engine's config dict.

        Looks for ``proxy_pool`` (list of URLs) and ``scrape_proxy_url``
        (dynamic endpoint). Returns ``None`` if neither is configured.
        """
        proxies: list[str] = config.get("proxy_pool", []) or []
        scrape_url: str = config.get("scrape_proxy_url", "") or ""
        cooloff: int = config.get("proxy_cooloff_seconds", _DEFAULT_COOLOFF_SECONDS)

        if not proxies and not scrape_url:
            return None

        return cls(
            proxies=proxies if proxies else None,
            scrape_proxy_url=scrape_url,
            cooloff_seconds=cooloff,
        )
