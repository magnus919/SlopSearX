"""Temporary G08 prompt raw-number boundary reference; no providers or shared store."""

import asyncio
import importlib.util
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
if len(sys.argv) != 2 or Path(sys.argv[1]).exists():
    raise RuntimeError("requires a new observation output")
spec = importlib.util.spec_from_file_location("t06_capture", Path(__file__).with_name("capture.py"))
capture = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = capture
spec.loader.exec_module(capture)


async def main():
    from slopsearx.mcp.harness import build_fixture_context, create_fixture_server

    context = build_fixture_context()

    async def factory():
        return context

    rows = []
    cases = [
        ("infinite_required", '{"question":1e400}'),
        ("infinite_extra", '{"question":"caller","extra":1e400}'),
        ("huge_required", '{"question":' + "9" * 400 + "}"),
        ("huge_extra", '{"question":"caller","extra":' + "9" * 400 + "}"),
        ("arguments_null", "null"),
        ("arguments_scalar", "1"),
        ("arguments_array", "[]"),
    ]
    async with capture.WireClient(create_fixture_server(state_factory=factory)) as client:
        for i, (case, arguments) in enumerate(cases, 100):
            raw = (
                '{"jsonrpc":"2.0","id":'
                + str(i)
                + ',"method":"prompts/get","params":{"name":"research_with_source_coverage","arguments":'
                + arguments
                + "}}"
            )
            response = await client.http.post("/mcp", content=raw, headers={"content-type": "application/json"})
            body = response.text
            if response.headers.get("content-type", "").startswith("text/event-stream"):
                body = [line[5:].strip() for line in body.splitlines() if line.startswith("data:")][-1]
            rows.append({"case": case, "raw_request": raw, "status": response.status_code, "body": body})
    if any(e.calls for e in context.active_engines.values()):
        raise RuntimeError("prompt characterization dispatched an engine")
    with Path(sys.argv[1]).open("x") as output:
        output.write(
            json.dumps(
                {"baseline": capture.BASELINE, "versions": capture.VERSIONS, "cases": rows, "dispatches": 0},
                ensure_ascii=False,
                indent=2,
            )
            + "\n"
        )


if __name__ == "__main__":
    asyncio.run(main())
