"""PubChem adapter — chemical compound search.

Free, public REST API. No auth required.
Docs: https://pubchem.ncbi.nlm.nih.gov/docs/pug-rest
"""

from __future__ import annotations

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


@register_engine
class PubChemAdapter(EngineAdapter):
    """PubChem compound search."""

    name = "pubchem"
    display_name = "PubChem"
    env_prefix = "ENGINE_PUBCHEM"
    engine_type = "api"
    categories = ["science", "reference", "chemistry", "medical"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://pubchem.ncbi.nlm.nih.gov/rest/pug")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)

        start_time = time.monotonic()

        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0) as client:
                # Search by name/query
                url = f"{base_url}/compound/name/{urllib.parse.quote(query)}/cids/JSON"
                resp = await client.get(url)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 404:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)
                resp.raise_for_status()
                data = resp.json()

                id_list = data.get("IdentifierList", {}).get("CID", [])
                if not id_list:
                    return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=latency)

                cids = id_list[:max_results]

                # Get compound details
                detail_url = f"{base_url}/compound/cid/{','.join(str(c) for c in cids)}/JSON"
                detail_resp = await client.get(detail_url)
                detail_resp.raise_for_status()
                detail_data = detail_resp.json()

                results = []
                pc_compounds = detail_data.get("PC_Compounds", [])
                for idx, compound in enumerate(pc_compounds[:max_results]):
                    cid = compound.get("id", {}).get("id", {}).get("cid", "")
                    props = compound.get("props", [])

                    name = ""
                    formula = ""
                    mass = ""

                    for prop in props:
                        urn = prop.get("urn", {})
                        label = urn.get("label", "")
                        name_val = urn.get("name", "")
                        value = prop.get("value", {})

                        if label == "IUPAC Name" and name_val == "Preferred":
                            sval = value.get("sval", "")
                            if sval:
                                name = sval
                        elif label == "Molecular Formula":
                            sval = value.get("sval", "")
                            if sval:
                                formula = sval
                        elif label == "Molecular Weight":
                            fval = value.get("fval")
                            if fval:
                                mass = f"{fval:.2f}"
                    # Fallback name from synonyms search
                    if not name:
                        name = f"Compound CID {cid}"

                    content_parts = []
                    if formula:
                        content_parts.append(f"Formula: {formula}")
                    if mass:
                        content_parts.append(f"MW: {mass}")
                    content = " | ".join(content_parts) if content_parts else "Chemical compound"

                    results.append(
                        SearchResult(
                            url=f"https://pubchem.ncbi.nlm.nih.gov/compound/{cid}",
                            title=name,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=1.0,
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
