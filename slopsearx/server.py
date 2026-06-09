"""FastAPI HTTP server (stub for V1)."""

from __future__ import annotations

from fastapi import FastAPI

app = FastAPI(title="SlopSearX", version="0.1.0")


@app.get("/health")
async def health() -> dict:
    return {"status": "ok", "version": "0.1.0"}


@app.get("/search")
async def search(q: str = "", format: str = "json") -> dict:
    """Execute a search across all enabled engines (stub)."""
    return {"query": q, "results": [], "number_of_results": 0}
