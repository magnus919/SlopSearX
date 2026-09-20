"""Optional TypeSafe Jev specialist-engine routing.

The presence of ``TYPESAFE_API_KEY`` enables this path. Policy eligibility is
decided by SlopSearX first; Jev can only add eligible specialist engines.
"""

from __future__ import annotations

import hashlib
import json
import logging
import math
import os
from dataclasses import dataclass
from typing import Any

import httpx

from slopsearx.adapter import EngineAdapter
from slopsearx.capabilities import CapabilityCatalog

logger = logging.getLogger(__name__)

MODEL = "jev-1.13.0"
CARD_VERSION = "1"
GENERAL_ENGINES = frozenset({"brave", "google", "duckduckgo"})
BROAD_SUPPORT_ENGINES = frozenset({"wikipedia", "stackexchange", "reddit", "hackernews"})

# Production routing cards are deliberately concise. Every registered engine
# must have a role; every specialist must have a purpose and use_when string.
ROUTING_CARDS: dict[str, dict[str, str]] = {
    "abuseipdb": {
        "purpose": "IP reputation and abuse reports",
        "use_when": "an IP address or abuse reputation is central",
    },
    "arxiv": {"purpose": "research preprints", "use_when": "the query seeks scientific papers or preprints"},
    "ashby": {"purpose": "jobs hosted by Ashby", "use_when": "the query seeks current job openings"},
    "censys": {
        "purpose": "internet hosts and certificates",
        "use_when": "the query targets exposed infrastructure or certificates",
    },
    "clinicaltrials": {
        "purpose": "registered clinical trials",
        "use_when": "the query seeks trials, interventions, or enrollment",
    },
    "crates": {"purpose": "Rust packages", "use_when": "the query names a Rust crate or package need"},
    "crtsh": {
        "purpose": "certificate transparency records",
        "use_when": "the query concerns domains, subdomains, or certificates",
    },
    "cve": {"purpose": "canonical CVE records", "use_when": "the query names a CVE or vulnerability identifier"},
    "dehashed": {
        "purpose": "paid breach-data lookup",
        "use_when": "authorized breach exposure lookup is explicitly requested",
    },
    "dockerhub": {"purpose": "container images", "use_when": "the query seeks a Docker image or tag"},
    "edgar": {"purpose": "US public-company filings", "use_when": "the query seeks SEC filings or company disclosures"},
    "epss": {
        "purpose": "vulnerability exploitation probability",
        "use_when": "the query seeks EPSS likelihood for a CVE",
    },
    "exploitdb": {
        "purpose": "public exploit records",
        "use_when": "the query seeks a known exploit or proof of concept",
    },
    "fred": {"purpose": "US economic time series", "use_when": "the query seeks macroeconomic data or indicators"},
    "github": {
        "purpose": "source repositories, issues, and pull requests",
        "use_when": "the query seeks code or repository artifacts",
    },
    "greenhouse": {"purpose": "jobs hosted by Greenhouse", "use_when": "the query seeks current job openings"},
    "greynoise": {
        "purpose": "internet scanner and IP intelligence",
        "use_when": "the query seeks context about an observed IP",
    },
    "hibp": {"purpose": "breach exposure status", "use_when": "authorized account or domain breach lookup is explicit"},
    "huggingface": {
        "purpose": "machine-learning datasets, models, and papers",
        "use_when": "the query seeks ML artifacts",
    },
    "intelx": {
        "purpose": "paid threat-intelligence search",
        "use_when": "authorized threat-intelligence lookup is explicit",
    },
    "internetarchive": {
        "purpose": "archived web pages and historical media",
        "use_when": "the query seeks an older or removed version",
    },
    "lever": {"purpose": "jobs hosted by Lever", "use_when": "the query seeks current job openings"},
    "mitreattack": {
        "purpose": "adversary tactics and techniques",
        "use_when": "the query seeks ATT&CK techniques, groups, or mitigations",
    },
    "musicbrainz": {
        "purpose": "music release and artist metadata",
        "use_when": "the query seeks structured music credits or releases",
    },
    "nominatim": {
        "purpose": "OpenStreetMap place geocoding",
        "use_when": "the query seeks a place, address, or coordinates",
    },
    "npm": {"purpose": "JavaScript packages", "use_when": "the query names a Node or JavaScript package need"},
    "nvd": {
        "purpose": "NIST vulnerability records",
        "use_when": "the query seeks CVE severity, affected products, or references",
    },
    "openalex": {
        "purpose": "scholarly works and authors",
        "use_when": "the query seeks research literature or citation metadata",
    },
    "openfda": {
        "purpose": "FDA drugs, devices, labels, and adverse events",
        "use_when": "the query seeks regulated medical-product data",
    },
    "openlibrary": {"purpose": "books and editions", "use_when": "the query seeks a book, author, ISBN, or edition"},
    "otx": {
        "purpose": "community threat indicators",
        "use_when": "the query seeks indicators, pulses, or threat context",
    },
    "oyez": {
        "purpose": "US Supreme Court cases and audio",
        "use_when": "the query seeks a Supreme Court case or argument",
    },
    "pubchem": {
        "purpose": "chemical compounds and bioassays",
        "use_when": "the query seeks a compound, structure, or chemical identifier",
    },
    "pubmed": {"purpose": "biomedical literature", "use_when": "the query seeks medical or life-science publications"},
    "pypi": {"purpose": "Python packages", "use_when": "the query names a Python package need"},
    "repology": {
        "purpose": "package versions across repositories",
        "use_when": "the query compares package versions or distributions",
    },
    "rubygems": {"purpose": "Ruby packages", "use_when": "the query names a Ruby gem or package need"},
    "semanticscholar": {
        "purpose": "scholarly papers and citations",
        "use_when": "the query seeks papers or citation relationships",
    },
    "shodan": {
        "purpose": "internet-connected device search",
        "use_when": "the query targets exposed services or devices",
    },
    "tmdb": {
        "purpose": "movie and television metadata",
        "use_when": "the query seeks film, series, cast, or release information",
    },
    "uniprot": {
        "purpose": "protein sequences and annotations",
        "use_when": "the query seeks a protein, gene product, or accession",
    },
    "urlhaus": {
        "purpose": "malware URL intelligence",
        "use_when": "the query concerns a suspicious URL or malware distribution",
    },
    "virustotal": {
        "purpose": "file, URL, domain, and IP reputation",
        "use_when": "the query seeks multi-scanner malware reputation",
    },
    "vulncheck": {
        "purpose": "vulnerability and exploitation intelligence",
        "use_when": "the query seeks exploitability or vulnerability enrichment",
    },
}

ROUTING_ROLES = {name: "specialist" for name in ROUTING_CARDS}
ROUTING_ROLES.update({name: "general" for name in GENERAL_ENGINES})
ROUTING_ROLES.update({name: "broad_support" for name in BROAD_SUPPORT_ENGINES})


def validate_routing_cards(engine_names: set[str]) -> None:
    """Fail when registered adapters and production routing metadata diverge."""
    missing = engine_names - set(ROUTING_ROLES)
    extra = set(ROUTING_ROLES) - engine_names
    invalid = [name for name, card in ROUTING_CARDS.items() if not card.get("purpose") or not card.get("use_when")]
    if missing or extra or invalid:
        raise ValueError(f"invalid routing cards: missing={sorted(missing)} extra={sorted(extra)} invalid={invalid}")


@dataclass(frozen=True)
class JevRoutingDecision:
    engines: list[str]
    scores: dict[str, float]
    latency_ms: float


class JevSpecialistRouter:
    """Score already-eligible specialists with one batched Jev request."""

    def __init__(self, api_key: str, threshold: float = 0.65, timeout_ms: int = 1000) -> None:
        self._api_key = api_key
        self.threshold = min(1.0, max(0.0, threshold))
        self.timeout = max(0.05, timeout_ms / 1000)

    @classmethod
    def from_environment(cls) -> JevSpecialistRouter | None:
        api_key = os.environ.get("TYPESAFE_API_KEY", "").strip()
        if not api_key:
            return None
        try:
            threshold = float(os.environ.get("JEV_SPECIALIST_THRESHOLD", "0.65"))
            timeout_ms = int(os.environ.get("JEV_TIMEOUT_MS", "1000"))
        except ValueError:
            logger.warning("Invalid Jev configuration; using threshold=0.65 timeout_ms=1000")
            threshold, timeout_ms = 0.65, 1000
        return cls(api_key, threshold, timeout_ms)

    async def route(
        self,
        query: str,
        active: dict[str, EngineAdapter],
        catalog: CapabilityCatalog | None,
        sensitive: set[str],
        cache: Any | None = None,
    ) -> JevRoutingDecision | None:
        candidates = []
        for name, engine in active.items():
            if ROUTING_ROLES.get(name) != "specialist" or name in sensitive or engine.circuit_open:
                continue
            capability = catalog.get(name) if catalog else None
            if capability and (
                not capability.enabled or (capability.auth_class == "required" and not capability.auth_configured)
            ):
                continue
            candidates.append(name)
        if not candidates:
            return JevRoutingDecision([], {}, 0.0)
        cache_key = self._decision_cache_key(query, candidates)
        if cache is not None:
            try:
                cached = await cache.get(cache_key)
                if cached is not None:
                    cached_scores = {str(name): float(score) for name, score in cached["scores"].items()}
                    engines = [str(name) for name in cached["engines"]]
                    if set(cached_scores) == set(candidates) and all(name in cached_scores for name in engines):
                        return JevRoutingDecision(engines, cached_scores, 0.0)
            except Exception:  # noqa: BLE001
                logger.warning("Jev routing decision cache unavailable; continuing without cache")
        questions = {}
        for name in sorted(candidates):
            engine, card = active[name], ROUTING_CARDS[name]
            questions[name] = {
                "type": "noul",
                "instructions": (
                    f"Specialist engine id: {name}. Display name: {engine.display_name}. Purpose: {card['purpose']}. "
                    f"Use when: {card['use_when']}. Categories: {', '.join(engine.categories)}. Would this named "
                    "specialist provide distinctive, directly useful evidence beyond ordinary general web search for "
                    "the user query? Answer no when general search is sufficient."
                ),
            }
        started = __import__("time").monotonic()
        try:
            async with httpx.AsyncClient(timeout=self.timeout) as client:
                response = await client.post(
                    "https://api.typesafe.ai/v1/systemone",
                    headers={"Authorization": f"Bearer {self._api_key}"},
                    json={"model": MODEL, "state": {"user_query": query}, "questions": questions},
                )
            response.raise_for_status()
            body = response.json()
            if body.get("model") != MODEL:
                return None
            answers = body.get("answers", {})
            scores: dict[str, float] = {}
            for name in questions:
                value = answers.get(name, {}).get("noul")
                if not isinstance(value, (int, float)) or not math.isfinite(value) or not 0 <= value <= 1:
                    return None
                scores[name] = float(value)
            engines = sorted(
                (name for name, score in scores.items() if score >= self.threshold),
                key=lambda name: (-scores[name], name),
            )
            decision = JevRoutingDecision(engines, scores, (__import__("time").monotonic() - started) * 1000)
            if cache is not None:
                try:
                    ttl = max(1, int(os.environ.get("JEV_ROUTING_CACHE_TTL_SECONDS", "3600")))
                    await cache.set(cache_key, {"engines": engines, "scores": scores}, ttl=ttl)
                except Exception:  # noqa: BLE001
                    logger.warning("Jev routing decision could not be cached")
            return decision
        except Exception as exc:  # noqa: BLE001
            logger.warning("Jev specialist routing unavailable: %s", type(exc).__name__)
            return None

    def cache_identity(self) -> str:
        raw = json.dumps({"model": MODEL, "cards": CARD_VERSION, "threshold": self.threshold}, sort_keys=True)
        return hashlib.sha256(raw.encode()).hexdigest()[:16]

    def _decision_cache_key(self, query: str, candidates: list[str]) -> str:
        raw = json.dumps(
            {"query": query.strip(), "candidates": sorted(candidates), "identity": self.cache_identity()},
            ensure_ascii=False,
            sort_keys=True,
        )
        return "jev-routing:v1:" + hashlib.sha256(raw.encode()).hexdigest()
