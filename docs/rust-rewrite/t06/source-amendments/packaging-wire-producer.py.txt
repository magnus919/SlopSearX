"""Finite write-once W07 Python wire reference; temporary G08 evidence only."""

import argparse
import ast
import asyncio
import hashlib
import importlib.metadata
import importlib.util
import json
import os
import socket
import subprocess
import sys
from pathlib import Path

NAMES = [
    "slopsearx_submit_retrieval_receipt",
    "slopsearx_read_retrieval_receipts",
    "slopsearx_export_research_manifest",
    "slopsearx_get_artifact_lineage",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(value, message):
    if not value:
        raise RuntimeError(message)


async def capture(root, bindings, guard):
    from slopsearx.mcp.harness import FakeEngineSpec, build_fixture_context, create_fixture_server

    # Reuse only the frozen raw HTTP client class, never run the historical producer.
    old = root / "docs/rust-rewrite/t06/capture.py"
    module = ast.parse(old.read_text())
    client = next(row for row in module.body if isinstance(row, ast.ClassDef) and row.name == "WireClient")
    namespace = {"socket": socket, "asyncio": asyncio, "guard": guard, "json": json}
    exec(compile(ast.Module(body=[client], type_ignores=[]), str(old), "exec"), namespace)
    context = build_fixture_context([FakeEngineSpec(name="wikipedia", count=1)])

    async def factory():
        return context

    cases = []
    seeds = [
        ("submit_missing", NAMES[0], {}),
        (
            "submit_bad_retriever_type",
            NAMES[0],
            {"result_id": "missing:0", "retriever": True, "idempotency_key": "once", "status": "failed"},
        ),
        ("read_bool_limit", NAMES[1], {"result_id": "missing:0", "limit": True}),
        ("read_string_limit", NAMES[1], {"result_id": "missing:0", "limit": "1"}),
        ("read_zero_limit", NAMES[1], {"result_id": "missing:0", "limit": 0}),
        ("read_large_limit", NAMES[1], {"result_id": "missing:0", "limit": 21}),
        ("read_malformed_handle", NAMES[1], {"result_id": "bad"}),
        ("manifest_bool_depth", NAMES[2], {"max_depth": True}),
        ("manifest_nested_bad_result", NAMES[2], {"result_ids": [True]}),
        ("manifest_nested_bad_artifact", NAMES[2], {"artifacts": [True]}),
        ("manifest_conflict", NAMES[2], {"result_ids": ["missing:0"], "source": {}}),
        (
            "lineage_bool_depth",
            NAMES[3],
            {
                "artifact": {"contract": "slopsearx.artifact_ref", "version": 1, "kind": "snapshot", "id": "missing"},
                "max_depth": True,
            },
        ),
        ("lineage_malformed_artifact", NAMES[3], {"artifact": {"kind": "bad", "id": "missing"}}),
        (
            "lineage_invalid_direction",
            NAMES[3],
            {
                "artifact": {"contract": "slopsearx.artifact_ref", "version": 1, "kind": "snapshot", "id": "missing"},
                "direction": "invalid",
            },
        ),
        (
            "lineage_large_depth",
            NAMES[3],
            {
                "artifact": {"contract": "slopsearx.artifact_ref", "version": 1, "kind": "snapshot", "id": "missing"},
                "max_depth": 6,
            },
        ),
        (
            "lineage_zero_nodes",
            NAMES[3],
            {
                "artifact": {"contract": "slopsearx.artifact_ref", "version": 1, "kind": "snapshot", "id": "missing"},
                "max_nodes": 0,
            },
        ),
    ]
    seeds.extend(
        [
            ("t06_search_missing", "slopsearx_search", {}),
            ("t06_search_null_query", "slopsearx_search", {"query": None}),
            ("t06_search_unexpected", "slopsearx_search", {"query": "q", "unexpected": 1}),
            ("t06_read_entities_bool_version", "slopsearx_read_entities", {"cursor": "missing", "version": True}),
            ("t06_read_results_bad_cursor_type", "slopsearx_read_results", {"cursor": True}),
            ("t06_read_result_bad_id_type", "slopsearx_read_result", {"result_id": True}),
        ]
    )
    async with namespace["WireClient"](create_fixture_server(state_factory=factory)) as wire:
        full_catalog = await wire.list_tools()
        tools = [row for row in full_catalog if row["name"] in NAMES]
        require([row["name"] for row in tools] == NAMES, "incomplete/changed four-tool order")
        for identity, name, arguments in seeds:
            before = {key: value.calls for key, value in context.active_engines.items()}
            result = await wire.call_tool_mcp(name, arguments)
            after = {key: value.calls for key, value in context.active_engines.items()}
            require(before == after, "wire reference dispatched an engine")
            cases.append({"id": identity, "name": name, "arguments": arguments, "result": result, "dispatch_delta": 0})
    return {"tools": tools, "full_catalog": full_catalog, "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--packaging-version", choices=("26.2", "26.3"), default="26.2")
    args = parser.parse_args()
    # Refuse the destination before metadata, backend or guard imports.
    require(not os.path.lexists(args.output), "refusing existing observation")
    require(args.output.parent.is_dir(), "output parent missing")
    sys.dont_write_bytecode = True
    require(sys.version_info[:3] == (3, 14, 7), "requires exact CPython3.14.7")
    versions = {key: importlib.metadata.version(key) for key in ("fastmcp", "mcp", "pydantic", "packaging")}
    require(
        versions == {"fastmcp": "4.0.11", "mcp": "2.3.0", "pydantic": "2.13.4", "packaging": args.packaging_version},
        "wrong pinned SDK profile",
    )
    root = Path(__file__).resolve().parents[3]
    binding_path = Path(__file__).with_name("wire-reference-bindings.json")
    bindings = json.loads(binding_path.read_text())
    require(digest(Path(__file__).resolve()) == bindings["producer_sha256"], "producer drift")
    git_env = {key: value for key, value in os.environ.items() if key in ("PATH", "TMPDIR")}
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, env=git_env, text=True).strip()
    require(head == bindings["baseline"], "wrong pinned source revision")
    for path, expected in bindings["sources"].items():
        require(digest(root / path) == expected, "source binding drift: " + path)
    provenance = {
        "executable": str(Path(sys.executable).resolve()),
        "executable_sha256": digest(Path(sys.executable).resolve()),
        "prefix": sys.prefix,
        "version": sys.version,
    }
    sdk_files = {}
    for key in ("fastmcp", "mcp", "pydantic", "packaging"):
        spec = importlib.util.find_spec(key)
        require(spec is not None and spec.origin is not None, "SDK module missing")
        sdk_files[key] = {"path": str(Path(spec.origin).resolve()), "sha256": digest(Path(spec.origin).resolve())}
    spec = importlib.util.spec_from_file_location(
        "w07_oracle_guard", root / "docs/rust-rewrite/replay/oracle/export.py"
    )
    guard = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = guard
    spec.loader.exec_module(guard)
    guard.sanitize_environment(candidate=False)
    guard.assert_no_shared_store_url()
    guard.install_loopback_network_guard()
    os.environ["MCP_GRANT_RETRIEVAL_RECEIPTS"] = "1"
    sys.path.insert(0, str(root))
    result = asyncio.run(asyncio.wait_for(capture(root, bindings, guard), 60))
    packet = {
        "schema_version": 1,
        "plan_id": "W07",
        "kind": "measured_python_wire",
        "baseline": head,
        "versions": versions,
        "interpreter": provenance,
        "sdk_files": sdk_files,
        "producer_sha256": bindings["producer_sha256"],
        "bindings_sha256": digest(binding_path),
        "sources": bindings["sources"],
        "provider_calls": 0,
        "guard": "sanitized environment; no shared store; only explicitly admitted loopback ports; fake engines",
        "normalization": "none",
        **result,
    }
    raw = (json.dumps(packet, indent=2, ensure_ascii=False) + "\n").encode()
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as target:
        target.write(raw)
        target.flush()
        os.fsync(target.fileno())


if __name__ == "__main__":
    main()
