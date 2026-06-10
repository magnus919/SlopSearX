"""
SlopSearX — Query Router.

Examines search queries and routes them to the most relevant engine
subset, improving latency, reducing API quota waste, and producing
less noisy results for agent consumers.

The router uses a configurable topic-signature mapping. First-match
wins — the first topic whose keywords appear in the query determines
the engine set. Queries matching no signature fall back to the
default engine set (``[brave, wikipedia]``).

Configured via :file:`/etc/slopsearx/config.yaml`::

    routing:
      enabled: true
      topics:
        code:
          keywords: [python, javascript, rust, typescript, golang, react, api, code, github]
          engines: [brave, github, stackexchange, wikipedia]
        science:
          keywords: [quantum, physics, biology, chemistry, "neural network", "machine learning",
                     "deep learning", paper, doi, theorem, algorithm]
          engines: [brave, arxiv, semanticscholar, openalex, wikipedia]
        news:
          keywords: [news, today, breaking, latest, update, announced, released]
          engines: [brave, hackernews, duckduckgo]
        social:
          keywords: [reddit, "hacker news", show hn, ask hn, discussion]
          engines: [brave, hackernews, reddit]
        reference:
          keywords: [documentation, docs, tutorial, guide, how to, reference, wiki, manual]
          engines: [brave, wikipedia, stackexchange]
"""

from __future__ import annotations

from typing import Any

# Default topic-to-engines mapping — operators can override via config.
# Order matters: first matching topic wins.
_DEFAULT_TOPICS: list[dict[str, Any]] = [
    {
        "name": "code",
        "keywords": [
            "python",
            "javascript",
            "typescript",
            "rust",
            "golang",
            "react",
            "api",
            "code",
            "github",
            "docker",
            "kubernetes",
            "sql",
            "npm",
            "pip",
            "cargo",
        ],
        "engines": ["brave", "github", "stackexchange", "duckduckgo", "wikipedia"],
    },
    {
        "name": "science",
        "keywords": [
            "quantum",
            "physics",
            "biology",
            "chemistry",
            "neural network",
            "machine learning",
            "deep learning",
            "paper",
            "doi",
            "theorem",
            "algorithm",
            "mathematics",
            "statistics",
        ],
        "engines": ["brave", "arxiv", "semanticscholar", "openalex", "duckduckgo", "wikipedia"],
    },
    {
        "name": "news",
        "keywords": [
            "news",
            "today",
            "breaking",
            "latest",
            "update",
            "announced",
            "released",
            "headline",
        ],
        "engines": ["brave", "hackernews", "duckduckgo"],
    },
    {
        "name": "social",
        "keywords": [
            "reddit",
            "hacker news",
            "show hn",
            "ask hn",
            "discussion",
            "forum",
        ],
        "engines": ["brave", "hackernews", "reddit", "duckduckgo"],
    },
    {
        "name": "reference",
        "keywords": [
            "documentation",
            "docs",
            "tutorial",
            "guide",
            "how to",
            "reference",
            "wiki",
            "manual",
            "definition",
        ],
        "engines": ["brave", "wikipedia", "stackexchange", "duckduckgo"],
    },
    {
        "name": "historical",
        "keywords": [
            "archive",
            "wayback",
            "historical",
            "history",
            "old",
            "vintage",
            "retro",
        ],
        "engines": ["brave", "wikipedia", "internetarchive", "duckduckgo"],
    },
]

_DEFAULT_FALLBACK = ["brave", "wikipedia"]


class QueryRouter:
    """Lightweight query classifier that routes queries to relevant engines.

    Uses first-match-wins keyword matching against configurable topic
    signatures. No ML, no LLM calls, no remote API.
    """

    def __init__(self, routing_config: dict[str, Any] | None = None) -> None:
        """Build topic list from config or defaults.

        Args:
            routing_config: Optional dict from the ``routing`` section
                of config.yaml. Expected shape::

                    {"enabled": true,
                     "topics": {"code": {"keywords": [...], "engines": [...]}, ...}}
        """
        self.enabled = True
        self.topics: list[dict[str, Any]] = list(_DEFAULT_TOPICS)
        self.fallback: list[str] = list(_DEFAULT_FALLBACK)

        if routing_config:
            self._apply_config(routing_config)

    def _apply_config(self, cfg: dict[str, Any]) -> None:
        self.enabled = cfg.get("enabled", True)

        if "fallback" in cfg and cfg["fallback"] is not None:
            self.fallback = list(cfg["fallback"])

        topics_cfg = cfg.get("topics")
        if topics_cfg is not None:
            if isinstance(topics_cfg, dict):
                # Named-topic format from config.yaml
                self.topics = []
                for _name, t in topics_cfg.items():
                    if isinstance(t, dict) and "keywords" in t and "engines" in t:
                        self.topics.append(t)
            elif isinstance(topics_cfg, list):
                # Ordered-list format
                self.topics = [t for t in topics_cfg if isinstance(t, dict)]

    def route(
        self,
        query: str,
        categories: list[str] | None = None,
    ) -> list[str] | None:
        """Determine which engines to dispatch to based on the query.

        Args:
            query: The raw search query.
            categories: Explicit category filter from the request. If
                provided (non-empty), routing is skipped — the caller
                should use category-based engine selection instead.

        Returns:
            A list of engine names to dispatch to, or ``None`` to
            indicate "use the caller's default selection logic".
        """
        if not self.enabled:
            return None

        # Skip routing when the caller already has a category filter
        if categories:
            return None

        query_lower = query.lower().strip()

        for topic in self.topics:
            keywords = topic.get("keywords", [])
            engines = topic.get("engines", [])
            if not engines:
                continue

            for kw in keywords:
                if kw in query_lower:
                    return engines  # type: ignore[no-any-return]

        # No topic matched — let the caller use its default engine set
        return None
