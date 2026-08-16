"""Runtime capability catalog, intent profiles, and MCP policy model.

Generates the engine capability catalog from the **live registry and
effective configuration** (never from README prose), defines the
declarative intent profiles that the MCP search tools resolve against,
and loads the MCP policy (tool grants, bounds, sensitive engines) from
configuration.

Everything in this module is read-only introspection: it never issues
requests and never returns secret material (API keys are only
represented as the boolean ``auth_configured``).
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from slopsearx.adapter import (
    FAILURE_CLASS_TOKENS,
    SUPPORTED_FILTER_KEYS,
    SUPPORTED_RESULT_TYPES,
    EngineAdapter,
    list_engines,
)
from slopsearx.config import CONFIG_FILE_PATH, Config, EngineEntry, load_config

# ---------------------------------------------------------------------------
# Auth requirement classes
# ---------------------------------------------------------------------------

AUTH_NONE = "none"
AUTH_OPTIONAL = "optional"
AUTH_REQUIRED = "required"
AUTH_UNKNOWN = "unknown"

# Engines that must never be reached by generic category or unscoped
# routing. They are only reachable through an explicit ``engines`` list
# or a policy grant (see the MCP policy model). Defaults follow the
# safety boundaries in docs/MCP_SERVER_DESIGN.md §5. The canonical
# definition lives here; the search service imports it.
DEFAULT_SENSITIVE_ENGINES: frozenset[str] = frozenset({"hibp", "dehashed"})

# Engines whose adapters fail closed at search time without configured
# credentials. Kept as a curated set because adapters do not yet expose
# a standardized requirement marker; operators can override via the
# ``mcp.required_key_engines`` config list.
REQUIRED_KEY_ENGINES: frozenset[str] = frozenset(
    {
        "abuseipdb",
        "brave",
        "censys",
        "dehashed",
        "fred",
        "hibp",
        "intelx",
        "otx",
        "shodan",
        "tmdb",
        "virustotal",
        "vulncheck",
    }
)


# ---------------------------------------------------------------------------
# Capability model
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class EngineCapability:
    """Public, redacted metadata for one engine."""

    name: str
    display_name: str
    engine_type: str  # api | scrape | structured
    categories: list[str]
    enabled: bool
    auth_class: str  # none | optional | required | unknown
    auth_configured: bool
    scope_hints: list[str]
    caveats: list[str]
    # Feature matrix (design §4.6): the full capability surface.
    sensitive: bool = False  # reaching this engine requires the sensitive-engine grant
    supported_filters: dict[str, bool] = field(default_factory=lambda: {key: False for key in SUPPORTED_FILTER_KEYS})
    supported_result_types: list[str] = field(default_factory=lambda: ["text"])
    failure_classes: list[str] = field(
        default_factory=lambda: ["rate_limited", "blocked", "error", "timeout", "auth_required", "unavailable"]
    )
    cost_class: str = ""  # coarse operator hint; "" = unknown (emitted as null)
    last_known_status: str = "unknown"  # ok|rate_limited|blocked|error|timeout|unknown
    last_known_status_at: str | None = None  # ISO freshness marker, or None when unknown

    @property
    def subcategories(self) -> list[str]:
        """Namespace-prefixed categories (e.g. ``github:code``)."""
        return [cat for cat in self.categories if ":" in cat]


class CapabilityCatalog:
    """Engine catalog generated from the runtime registry and config.

    Includes disabled engines so operators/agents can see what exists
    but is turned off; ``include_disabled`` controls that in callers.
    """

    def __init__(
        self,
        config: Config | None = None,
        adapters: dict[str, EngineAdapter] | None = None,
        required_key_engines: set[str] | frozenset[str] | None = None,
        engine_caveats: dict[str, list[str]] | None = None,
        sensitive_engines: set[str] | frozenset[str] | None = None,
    ) -> None:
        self._config = config or load_config()
        self._adapters = adapters or {}
        self._required_key = (
            set(required_key_engines) if required_key_engines is not None else set(REQUIRED_KEY_ENGINES)
        )
        self._sensitive = set(sensitive_engines) if sensitive_engines is not None else set(DEFAULT_SENSITIVE_ENGINES)
        self._caveats = dict(engine_caveats or _DEFAULT_ENGINE_CAVEATS)
        self._by_name = self._build()

    def _build(self) -> dict[str, EngineCapability]:
        out: dict[str, EngineCapability] = {}
        registry = list_engines()
        for name, cls in registry.items():
            entry = self._config.engines.get(name)
            adapter = self._adapters.get(name)
            # Matches discover_engines(): engines without an explicit config
            # entry are enabled (empty config defaults to enabled=True).
            enabled = entry.enabled if entry else True
            if adapter is not None:
                categories = list(adapter.categories)
            else:
                categories = list(cls.categories)
            auth_class, auth_configured = _auth_class_for(name, entry, self._required_key)
            out[name] = EngineCapability(
                name=name,
                display_name=cls.display_name,
                engine_type=cls.engine_type,
                categories=categories,
                enabled=enabled,
                auth_class=auth_class,
                auth_configured=auth_configured,
                scope_hints=_scope_hints(categories),
                caveats=list(self._caveats.get(name, [])),
                sensitive=name in self._sensitive
                or bool(getattr(adapter if adapter is not None else cls, "sensitive", False)),
                supported_filters=_normalize_supported_filters(cls, adapter),
                supported_result_types=_normalize_result_types(cls, adapter),
                failure_classes=_normalize_failure_classes(cls, adapter),
                cost_class=str(getattr(adapter if adapter is not None else cls, "cost_class", "") or ""),
                last_known_status="unknown",
                last_known_status_at=None,
            )
        return out

    def all(self) -> list[EngineCapability]:
        """All engines in registry order."""
        return list(self._by_name.values())

    def enabled(self) -> list[EngineCapability]:
        """Only engines enabled by the effective configuration."""
        return [cap for cap in self._by_name.values() if cap.enabled]

    def get(self, name: str) -> EngineCapability | None:
        """Fetch one engine's capability, or None for unknown names."""
        return self._by_name.get(name)

    def engines_for_categories(self, categories: list[str]) -> list[str]:
        """Engine names matching any of the given categories (OR)."""
        wanted = set(categories)
        return [
            cap.name for cap in self._by_name.values() if cap.enabled and any(cat in wanted for cat in cap.categories)
        ]

    def known_names(self) -> set[str]:
        """All registered engine names (enabled or not)."""
        return set(self._by_name)

    def families(self) -> dict[str, list[str]]:
        """Capability families → engine names, for discovery UIs."""
        result: dict[str, list[str]] = {}
        for cap in self._by_name.values():
            for cat in cap.categories:
                if ":" in cat:
                    continue  # subcategories belong to the parent family
                result.setdefault(cat, []).append(cap.name)
        for names in result.values():
            names.sort()
        return result


# ---------------------------------------------------------------------------
# Intent profiles
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class IntentProfile:
    """Declarative intent → category/engine profile."""

    intent: str
    description: str
    categories: list[str] = field(default_factory=list)
    engines: list[str] = field(default_factory=list)  # explicit override
    sensitive: bool = False  # contains sensitive engines → needs a grant


INTENT_PROFILES: dict[str, IntentProfile] = {
    "web": IntentProfile(
        intent="web",
        description="General-purpose web search across broad engines.",
        categories=["general"],
    ),
    "news": IntentProfile(
        intent="news",
        description="Recent news and headlines.",
        categories=["news"],
    ),
    "science": IntentProfile(
        intent="science",
        description="Papers, scholarly indexes, and scientific references.",
        categories=["science"],
    ),
    "reference": IntentProfile(
        intent="reference",
        description="Encyclopedic and reference material.",
        categories=["reference"],
    ),
    "code": IntentProfile(
        intent="code",
        description="Source code, issues, and package registries.",
        engines=["github", "stackexchange", "pypi", "npm", "crates", "rubygems", "dockerhub", "repology"],
    ),
    "social": IntentProfile(
        intent="social",
        description="Discussion forums and social sources.",
        engines=["reddit", "hackernews", "brave", "duckduckgo"],
    ),
    "historical": IntentProfile(
        intent="historical",
        description="Archives and historical material.",
        categories=["historical"],
        engines=["wikipedia", "internetarchive"],
    ),
    "jobs": IntentProfile(
        intent="jobs",
        description="Job postings on ATS boards and general boards.",
        engines=["greenhouse", "ashby", "lever", "brave", "duckduckgo"],
    ),
    "security": IntentProfile(
        intent="security",
        description="Vulnerabilities, threat intelligence, and exposures.",
        engines=[
            "cve",
            "nvd",
            "shodan",
            "censys",
            "crtsh",
            "otx",
            "abuseipdb",
            "virustotal",
            "urlhaus",
            "epss",
            "vulncheck",
            "intelx",
            "exploitdb",
            "mitreattack",
            "greynoise",
            "hibp",
            "dehashed",
        ],
        sensitive=True,
    ),
    "medical": IntentProfile(
        intent="medical",
        description="Clinical trials, biomedical literature, and health data.",
        engines=["pubmed", "clinicaltrials", "openfda", "pubchem", "uniprot"],
    ),
    "finance": IntentProfile(
        intent="finance",
        description="Economic data and SEC filings.",
        categories=["finance"],
    ),
    "packages": IntentProfile(
        intent="packages",
        description="Software package registries.",
        categories=["packages"],
    ),
    "media": IntentProfile(
        intent="media",
        description="Music, movies, and books.",
        categories=["music", "movies", "entertainment", "books"],
    ),
    "legal": IntentProfile(
        intent="legal",
        description="Court cases and legal references.",
        categories=["legal"],
    ),
    "geography": IntentProfile(
        intent="geography",
        description="Geocoding and place lookup.",
        categories=["geography"],
    ),
}


def resolve_intent(
    intent: str,
    catalog: CapabilityCatalog,
) -> tuple[list[str], list[str]]:
    """Resolve an intent to concrete engine names.

    Returns ``(engines, warnings)``. Category-based profiles are
    resolved against the live catalog; explicit-engine profiles are
    validated against the registry. Unknown intents return
    ``([], ["unknown intent ..."])``.
    """
    profile = INTENT_PROFILES.get(intent)
    if profile is None:
        return [], [f"unknown intent '{intent}'; valid intents: {', '.join(sorted(INTENT_PROFILES))}"]
    if profile.engines:
        known = catalog.known_names()
        engines = [name for name in profile.engines if name in known]
        missing = [name for name in profile.engines if name not in known]
        warnings = [f"intent '{intent}' references unknown engines: {', '.join(sorted(missing))}"] if missing else []
        return engines, warnings
    return catalog.engines_for_categories(profile.categories), []


def validate_intent_profiles(catalog: CapabilityCatalog) -> list[str]:
    """Validate every intent profile against the registry.

    Returns a list of problems (empty when healthy). Called at MCP
    server startup so misconfigured profiles fail loudly.
    """
    problems: list[str] = []
    for intent, profile in INTENT_PROFILES.items():
        if profile.engines:
            missing = [name for name in profile.engines if name not in catalog.known_names()]
            if missing:
                problems.append(f"intent '{intent}' references unknown engines: {', '.join(sorted(missing))}")
        else:
            matched = catalog.engines_for_categories(profile.categories)
            if not matched:
                problems.append(f"intent '{intent}' has categories matching no enabled engines: {profile.categories}")
    return problems


# ---------------------------------------------------------------------------
# MCP policy
# ---------------------------------------------------------------------------


@dataclass
class MCPPolicy:
    """Operator policy for the MCP surface. Secure by default."""

    # Specialist tool grants — all disabled until explicitly enabled.
    enabled_tools: dict[str, bool] = field(
        default_factory=lambda: {"jobs": False, "security": False, "science": False, "research": False}
    )
    sensitive_engines: set[str] = field(default_factory=lambda: set(DEFAULT_SENSITIVE_ENGINES))
    required_key_engines: set[str] = field(default_factory=lambda: set(REQUIRED_KEY_ENGINES))
    max_query_length: int = 500
    max_results: int = 50
    snapshot_ttl_seconds: int = 3600
    job_max_queries: int = 20
    job_max_engines_per_query: int = 10
    job_max_results: int = 500
    job_default_deadline_seconds: int = 600
    # Empty token = authentication disabled (stdio transport is trusted by
    # process-launch boundary; HTTP transport requires a token).
    auth_token: str = ""
    # Explicit engine selection is an advanced operation (design §5):
    # reaching sensitive engines via slopsearx_search_targeted requires
    # an explicit operator grant.
    targeted_sensitive_allowed: bool = False
    # OAuth 2.1 authorization-server mode (alternative to auth_token).
    # When enabled, the server speaks standard MCP OAuth with dynamic
    # client registration; the issuer URL must be externally reachable.
    oauth_enabled: bool = False
    oauth_issuer_url: str = ""
    oauth_service_documentation_url: str = ""
    oauth_access_token_ttl_seconds: int = 3600
    oauth_refresh_token_ttl_seconds: int = 2_592_000  # 30 days

    def tool_enabled(self, tool: str) -> bool:
        """Whether a specialist tool is granted."""
        return bool(self.enabled_tools.get(tool, False))

    def validate(self, catalog: CapabilityCatalog) -> list[str]:
        """Validate grants and engine lists against the registry."""
        problems: list[str] = []
        for engine in sorted(self.sensitive_engines):
            if engine not in catalog.known_names():
                problems.append(f"mcp.sensitive_engines references unknown engine '{engine}'")
        for engine in sorted(self.required_key_engines):
            if engine not in catalog.known_names():
                problems.append(f"mcp.required_key_engines references unknown engine '{engine}'")
        if self.oauth_enabled and not self.oauth_issuer_url:
            problems.append("mcp.oauth.enabled requires mcp.oauth.issuer_url (or MCP_OAUTH_ISSUER_URL)")
        return problems


def load_mcp_policy(
    config: Config | None = None,
    config_path: str | Path | None = None,
) -> MCPPolicy:
    """Load the MCP policy from the ``mcp:`` YAML section plus ``MCP_*`` env vars.

    The policy is intentionally separate from the HTTP service config:
    the MCP surface has its own grants, bounds, and auth. Environment
    variables (``MCP_GRANT_JOBS=1``, ``MCP_MAX_RESULTS=100``, ...) always
    win over the YAML section.
    """
    del config  # section is read directly from the YAML file
    policy = MCPPolicy()
    path = Path(config_path) if config_path else Path(CONFIG_FILE_PATH)
    if path.exists():
        try:
            with open(path) as f:
                data = yaml.safe_load(f) or {}
        except yaml.YAMLError:
            data = {}
        mcp_cfg = data.get("mcp") or {}
        if isinstance(mcp_cfg, dict):
            _apply_mcp_section(policy, mcp_cfg)
    _apply_mcp_env(policy)
    return policy


def _apply_mcp_section(policy: MCPPolicy, section: dict[str, Any]) -> None:
    """Apply the ``mcp:`` YAML section to a policy."""
    grants = section.get("enabled_tools")
    if isinstance(grants, dict):
        for name, enabled in grants.items():
            if isinstance(enabled, bool) and name in policy.enabled_tools:
                policy.enabled_tools[name] = enabled
    for key in ("sensitive_engines", "required_key_engines"):
        values = section.get(key)
        if isinstance(values, list):
            setattr(policy, key, {str(v) for v in values if isinstance(v, str)})
    for key, default in (
        ("max_query_length", 500),
        ("max_results", 50),
        ("snapshot_ttl_seconds", 3600),
        ("job_max_queries", 20),
        ("job_max_engines_per_query", 10),
        ("job_max_results", 500),
        ("job_default_deadline_seconds", 600),
    ):
        value = section.get(key)
        if isinstance(value, int) and value > 0:
            setattr(policy, key, value)
    token = section.get("auth_token")
    if isinstance(token, str):
        policy.auth_token = token
    targeted = section.get("targeted_sensitive_allowed")
    if isinstance(targeted, bool):
        policy.targeted_sensitive_allowed = targeted

    oauth = section.get("oauth")
    if isinstance(oauth, dict):
        enabled = oauth.get("enabled")
        if isinstance(enabled, bool):
            policy.oauth_enabled = enabled
        issuer = oauth.get("issuer_url")
        if isinstance(issuer, str):
            policy.oauth_issuer_url = issuer.strip()
        docs_url = oauth.get("service_documentation_url")
        if isinstance(docs_url, str):
            policy.oauth_service_documentation_url = docs_url.strip()
        for key, attr in (
            ("access_token_ttl_seconds", "oauth_access_token_ttl_seconds"),
            ("refresh_token_ttl_seconds", "oauth_refresh_token_ttl_seconds"),
        ):
            value = oauth.get(key)
            if isinstance(value, int) and value > 0:
                setattr(policy, attr, value)


def _apply_mcp_env(policy: MCPPolicy) -> None:
    """Apply ``MCP_*`` environment overrides (always win)."""
    grant_map = {
        "MCP_GRANT_JOBS": "jobs",
        "MCP_GRANT_SECURITY": "security",
        "MCP_GRANT_SCIENCE": "science",
        "MCP_GRANT_RESEARCH": "research",
    }
    for env_var, tool in grant_map.items():
        value = os.environ.get(env_var, "").strip().lower()
        if value in ("true", "1", "yes"):
            policy.enabled_tools[tool] = True
        elif value in ("false", "0", "no"):
            policy.enabled_tools[tool] = False

    int_map = {
        "MCP_MAX_QUERY_LENGTH": "max_query_length",
        "MCP_MAX_RESULTS": "max_results",
        "MCP_SNAPSHOT_TTL_SECONDS": "snapshot_ttl_seconds",
        "MCP_JOB_MAX_QUERIES": "job_max_queries",
        "MCP_JOB_MAX_ENGINES_PER_QUERY": "job_max_engines_per_query",
        "MCP_JOB_MAX_RESULTS": "job_max_results",
        "MCP_JOB_DEFAULT_DEADLINE_SECONDS": "job_default_deadline_seconds",
    }
    for env_var, attr in int_map.items():
        raw = os.environ.get(env_var, "").strip()
        if raw:
            try:
                parsed = int(raw)
            except ValueError:
                continue
            if parsed > 0:
                setattr(policy, attr, parsed)

    token = os.environ.get("MCP_AUTH_TOKEN")
    if token is not None:
        policy.auth_token = token

    targeted = os.environ.get("MCP_TARGETED_SENSITIVE_ALLOWED", "").strip().lower()
    if targeted in ("true", "1", "yes"):
        policy.targeted_sensitive_allowed = True
    elif targeted in ("false", "0", "no"):
        policy.targeted_sensitive_allowed = False

    sensitive = os.environ.get("MCP_SENSITIVE_ENGINES")
    if sensitive:
        policy.sensitive_engines = {name.strip() for name in sensitive.split(",") if name.strip()}

    # OAuth mode
    oauth_enabled = os.environ.get("MCP_OAUTH_ENABLED", "").strip().lower()
    if oauth_enabled in ("true", "1", "yes"):
        policy.oauth_enabled = True
    elif oauth_enabled in ("false", "0", "no"):
        policy.oauth_enabled = False
    issuer = os.environ.get("MCP_OAUTH_ISSUER_URL")
    if issuer is not None:
        policy.oauth_issuer_url = issuer.strip()
    docs_url = os.environ.get("MCP_OAUTH_SERVICE_DOCUMENTATION_URL")
    if docs_url is not None:
        policy.oauth_service_documentation_url = docs_url.strip()
    for env_var, attr in (
        ("MCP_OAUTH_ACCESS_TOKEN_TTL_SECONDS", "oauth_access_token_ttl_seconds"),
        ("MCP_OAUTH_REFRESH_TOKEN_TTL_SECONDS", "oauth_refresh_token_ttl_seconds"),
    ):
        raw = os.environ.get(env_var, "").strip()
        if raw:
            try:
                parsed = int(raw)
            except ValueError:
                continue
            if parsed > 0:
                setattr(policy, attr, parsed)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _auth_class_for(
    name: str,
    entry: EngineEntry | None,
    required_key_engines: set[str],
) -> tuple[str, bool]:
    """Derive (auth_class, auth_configured) from config state."""
    has_key = bool(entry and entry.api_key)
    if name in required_key_engines:
        return AUTH_REQUIRED, has_key
    if has_key:
        return AUTH_OPTIONAL, True
    if entry is None:
        return AUTH_UNKNOWN, False
    return AUTH_NONE, False


def _normalize_supported_filters(
    cls: type[EngineAdapter],
    adapter: EngineAdapter | None,
) -> dict[str, bool]:
    """Normalize an adapter's declared supported_filters to all filter keys.

    Every entry must carry boolean members for ``language``, ``time_range``,
    ``safesearch``, and ``pagination`` (VAL-CAP-002), so undeclared keys
    default to ``False``.
    """
    declared = getattr(adapter if adapter is not None else cls, "supported_filters", None) or {}
    return {key: bool(declared.get(key)) for key in SUPPORTED_FILTER_KEYS}


def _normalize_result_types(
    cls: type[EngineAdapter],
    adapter: EngineAdapter | None,
) -> list[str]:
    """Normalize declared supported_result_types to the stable vocabulary."""
    declared = getattr(adapter if adapter is not None else cls, "supported_result_types", None) or ("text",)
    values = [value for value in declared if value in SUPPORTED_RESULT_TYPES]
    return values or ["text"]


def _normalize_failure_classes(
    cls: type[EngineAdapter],
    adapter: EngineAdapter | None,
) -> list[str]:
    """Normalize declared failure_classes to the stable machine-readable set."""
    declared = getattr(adapter if adapter is not None else cls, "failure_classes", None) or ()
    values = [value for value in declared if value in FAILURE_CLASS_TOKENS]
    return values or ["error"]


def _scope_hints(categories: list[str]) -> list[str]:
    """Scope hints derived from an engine's categories."""
    hints = ["explicit"]
    if "general" in categories:
        hints.append("unscoped")
    return hints


_DEFAULT_ENGINE_CAVEATS: dict[str, list[str]] = {
    "github": ["pagination hardcoded to page 1"],
    "brave": ["safesearch parameter not enforced"],
    "google": ["scrape adapter — subject to blocking and CAPTCHAs"],
    "duckduckgo": ["scrape adapter — subject to blocking and CAPTCHAs"],
    "greenhouse": ["requires a company name in the query; returns no full job descriptions"],
    "ashby": ["requires a company name in the query; returns no full job descriptions"],
    "lever": ["requires a company name in the query; returns no full job descriptions"],
    "internetarchive": ["opt-in engine, disabled by default"],
}
