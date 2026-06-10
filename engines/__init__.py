"""Engine adapters — each file is one search engine.

Importing this module triggers @register_engine on all adapters,
populating the global engine registry.
"""

from . import (
    arxiv,
    brave,
    crtsh,
    cve,
    duckduckgo,
    epss,
    exploitdb,
    github,
    google,
    greynoise,
    hackernews,
    huggingface,
    internetarchive,
    mitreattack,
    nvd,
    openalex,
    reddit,
    semanticscholar,
    urlhaus,
    wikipedia,
)

__all__ = [
    "arxiv",
    "brave",
    "crtsh",
    "cve",
    "duckduckgo",
    "epss",
    "exploitdb",
    "google",
    "github",
    "greynoise",
    "hackernews",
    "huggingface",
    "internetarchive",
    "mitreattack",
    "nvd",
    "openalex",
    "reddit",
    "semanticscholar",
    "urlhaus",
    "wikipedia",
]
