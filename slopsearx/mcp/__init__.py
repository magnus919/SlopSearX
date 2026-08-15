"""SlopSearX MCP server package.

Run with ``python -m slopsearx.mcp`` or the ``slopsearx-mcp`` console
script. See docs/MCP_SERVER.md for installation and configuration.
"""

from slopsearx.mcp.gateway import create_gateway
from slopsearx.mcp.server import create_server, main

__all__ = ["create_gateway", "create_server", "main"]
