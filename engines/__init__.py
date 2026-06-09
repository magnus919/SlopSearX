"""Engine adapters — each file is one search engine.

Importing this module triggers @register_engine on all adapters,
populating the global engine registry.
"""

from . import (
    arxiv,
    brave,
    duckduckgo,
    github,
    google,
    hackernews,
    huggingface,
    internetarchive,
    openalex,
    semanticscholar,
    stackexchange,
    wikipedia,
)

__all__ = [
    "arxiv",
    "brave",
    "duckduckgo",
    "google",
    "github",
    "hackernews",
    "huggingface",
    "internetarchive",
    "openalex",
    "semanticscholar",
    "stackexchange",
    "wikipedia",
]
