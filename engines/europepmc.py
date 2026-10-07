"""Europe PMC metadata search; opt-in, free public REST API.

Only lite bibliographic records are retrieved; no abstracts/full text/annotations.
Docs: https://europepmc.org/RestfulWebService
"""

from __future__ import annotations

import time
from datetime import date as calendar_date
from typing import Any
from urllib.parse import quote

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine
from slopsearx.payload import DOMAIN_SCIENCE, build_payload


@register_engine
class EuropePMCAdapter(EngineAdapter):
    """Biomedical publications and preprints from Europe PMC / EMBL-EBI."""

    name = "europepmc"
    display_name = "Europe PMC (EMBL-EBI)"
    env_prefix = "ENGINE_EUROPEPMC"
    engine_type = "api"
    categories = ["science", "reference", "medical", "health"]
    supported_result_types = ("text", "structured")
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"
    # Cursor pagination is deliberately not mapped onto stateless page numbers.
    # Publication dates are exposed, but no filter enforcement is claimed.

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        started = time.monotonic()
        try:
            if early := await self._check_rate_limit():
                return early
            limit = min(1000, max(1, int(self.config.get("max_results", 10))))
            timeout = self.config.get("timeout_ms", 5000) / 1000
            base = self.config.get("base_url", "https://www.ebi.ac.uk/europepmc/webservices/rest")
            async with self.http_client(timeout=timeout) as client:
                response = await client.get(
                    f"{base.rstrip('/')}/search",
                    params={"query": query, "format": "json", "resultType": "lite", "pageSize": limit},
                )
                response.raise_for_status()
                data = response.json()
            if not isinstance(data, dict) or not isinstance(data.get("resultList"), dict):
                raise ValueError("Europe PMC response missing resultList")
            records = data["resultList"].get("result")
            if not isinstance(records, list):
                raise ValueError("Europe PMC results must be a list")
            results: list[SearchResult] = []
            for record in records[:limit]:
                if not isinstance(record, dict):
                    raise ValueError("Europe PMC record must be an object")
                source, identifier, title = (record.get(k) for k in ("source", "id", "title"))
                if not isinstance(source, str) or not isinstance(identifier, str) or not isinstance(title, str):
                    continue
                if not source or not identifier or not title:
                    continue
                pmid = identifier if source == "MED" and identifier.isdigit() else None
                # Preserve provider identity/attribution: URL dedup currently
                # retains the first snippet and would erase our attribution
                # when a canonical PubMed URL was already present.
                url = f"https://europepmc.org/article/{quote(source, safe='')}/{quote(identifier, safe='')}"
                date = record.get("firstPublicationDate")
                try:
                    date = calendar_date.fromisoformat(date).isoformat() if isinstance(date, str) else None
                except ValueError:
                    date = None
                content = " — ".join(
                    v
                    for v in [
                        "Source: Europe PMC / EMBL-EBI (https://europepmc.org)",
                        "Preprint (not peer reviewed)" if source == "PPR" else None,
                        _text(record, "journalTitle")[:150],
                        _text(record, "authorString")[:200],
                    ]
                    if isinstance(v, str) and v
                )
                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=content,
                        engine=self.name,
                        score=1 / (len(results) + 1),
                        published_date=date,
                        payload=build_payload(
                            DOMAIN_SCIENCE,
                            "publication",
                            {
                                "publication_id": identifier,
                                "source": source,
                                "pmid": pmid,
                                "pmcid": record.get("pmcid"),
                                "doi": record.get("doi"),
                                "journal": record.get("journalTitle"),
                                "is_open_access": record.get("isOpenAccess"),
                                "in_epmc": record.get("inEPMC"),
                            },
                            engine=self.name,
                        ),
                    )
                )
            return AdapterResponse(
                results=results, status=EngineStatus.OK, latency_ms=(time.monotonic() - started) * 1000
            )
        except httpx.TimeoutException as exc:
            status, error = EngineStatus.TIMEOUT, str(exc)
        except httpx.HTTPStatusError as exc:
            status = EngineStatus.RATE_LIMITED if exc.response.status_code == 429 else EngineStatus.ERROR
            error = str(exc)
        except Exception as exc:
            status, error = EngineStatus.ERROR, str(exc)
        return AdapterResponse(
            results=[], status=status, error_message=error, latency_ms=(time.monotonic() - started) * 1000
        )


def _text(record: dict[str, Any], key: str) -> str:
    value = record.get(key)
    return value if isinstance(value, str) else ""
