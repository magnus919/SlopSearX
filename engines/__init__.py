"""Engine adapters — each file is one search engine.

Importing this module triggers @register_engine on all adapters,
populating the global engine registry.
"""

from . import (
    arxiv,
    brave,
    cve,
    duckduckgo,
    github,
    google,
    hackernews,
    huggingface,
    internetarchive,
    nvd,
    openalex,
    reddit,
    semanticscholar,
    stackexchange,
    wikipedia,
)

__all__ = [
    "arxiv",
    "brave",
    "cve",
    "duckduckgo",
    "google",
    "github",
    "hackernews",
    "huggingface",
    "internetarchive",
    "nvd",
    "openalex",
    "reddit",
    "semanticscholar",
    "stackexchange",
    "wikipedia",
]
