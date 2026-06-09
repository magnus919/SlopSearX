"""Output formatters — SearXNG JSON and YAML+Markdown (stub for V1)."""

from __future__ import annotations

from typing import Any


def format_json(results: list[dict], query: str, **meta: Any) -> dict:
    """Format results as SearXNG-compatible JSON response (stub)."""
    return {
        "query": query,
        "results": results,
        "number_of_results": len(results),
        **meta,
    }


def format_yaml(results: list[dict], query: str, **meta: Any) -> str:
    """Format results as YAML+Markdown (agent-native, stub)."""
    lines = [f"# Search: {query}", f"results: {len(results)}", ""]
    for r in results:
        lines.append(f"- [{r.get('title', '')}]({r.get('url', '')})")
    return "\n".join(lines)
