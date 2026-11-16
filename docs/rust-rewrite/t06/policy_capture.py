"""Manual temporary policy/availability oracle, guarded fake-state loopback only. G08."""
import asyncio
import importlib.util
import json
import os
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("t06_capture", Path(__file__).with_name("capture.py"))
capture = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = capture
spec.loader.exec_module(capture)


async def main():
    from slopsearx.mcp.harness import build_fixture_context, create_fixture_server, fixture_config
    from slopsearx.config import EngineEntry

    rows = []
    for allowed, disabled in [(False, False), (True, False), (False, True), (True, True)]:
        os.environ["MCP_TARGETED_SENSITIVE_ALLOWED"] = "1" if allowed else "0"
        context = build_fixture_context()
        config = fixture_config()
        if disabled:
            config.engines["hibp"] = EngineEntry(enabled=False)

        async def factory():
            return context

        async with capture.WireClient(create_fixture_server(state_factory=factory, config=config)) as client:
            for engines in [["wikipedia", "hibp"], ["hibp"], ["unknown", "hibp"]]:
                before = {k: e.calls for k, e in context.active_engines.items()}
                result = await client.call_tool_mcp("slopsearx_search", {"query": "scope boundary", "engines": engines})
                rows.append({"sensitive_allowed": allowed, "configured_disabled": disabled, "actual_active_engines": list(context.active_engines), "arguments": {"query": "scope boundary", "engines": engines}, "result": result, "dispatch_delta": {k: e.calls - before[k] for k, e in context.active_engines.items()}})
    Path(sys.argv[1]).write_text(json.dumps({"baseline": capture.BASELINE, "versions": capture.VERSIONS, "cases": rows}, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    asyncio.run(main())
