"""Engine adapters — each file is one search engine.

Importing this module triggers @register_engine on all adapters,
populating the global engine registry.
"""

from . import brave, duckduckgo, google, wikipedia

__all__ = ["brave", "duckduckgo", "google", "wikipedia"]
