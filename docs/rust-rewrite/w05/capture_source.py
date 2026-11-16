"""Manual W05 original-source oracle; inventoried for G08 retirement, never native runtime."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import socket
import sys
import unicodedata

# Fail closed before any backend import, including under python -O.
parser = argparse.ArgumentParser()
parser.add_argument("--allow-original-oracle", action="store_true")
parser.add_argument("--output", type=pathlib.Path, required=True)
args = parser.parse_args()
if not args.allow_original_oracle:
    raise RuntimeError("explicit temporary original oracle authorization required")
if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("pinned Python3.14.7 Unicode16 required")
if args.output.exists():
    raise RuntimeError("capture output exists; evidence is immutable")


def denied(*_args, **_kwargs):
    raise RuntimeError("W05 pure oracle forbids every network connection")


socket.socket.connect = denied
socket.socket.connect_ex = denied
socket.create_connection = denied
root = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(root))
from slopsearx import saved_events as original  # noqa: E402
from slopsearx.saved_models import SavedDefinition  # noqa: E402

original.generate_event_id = lambda: "event-0123456789abcdefghijklmn"
raw = (pathlib.Path(__file__).parent / "inputs.json").read_bytes()
inputs = json.loads(raw)
definition = SavedDefinition(**inputs["definition"])
rows = []
for item in inputs["reports"]:
    report = {"run_id": "run-α", "finished_at": 1060.5, **{k: v for k, v in item.items() if k != "name"}}
    event = original.event_for_report(definition, report)
    event["_unknown_internal"] = "private"
    rows.append(
        {
            "name": item["name"],
            "report": report,
            "stored": event,
            "public": original.public_event(event, "1060500-0"),
            "redacted": original.public_event(event, "1060500-0", redacted=True),
        }
    )
for kind in ["definition_paused", "definition_expired"]:
    event = original.definition_event(definition, kind, 1600.0)
    rows.append(
        {
            "name": kind,
            "stored": event,
            "public": original.public_event(event, "1600000-0"),
            "redacted": original.public_event(event, "1600000-0", redacted=True),
        }
    )
validation = []
for name, values, function in [
    ("consumer", inputs["consumers"], original.validate_consumer_id),
    ("cursor", inputs["cursors"], original.validate_cursor),
]:
    for value in values:
        value = value.replace("a{129}", "a" * 129).replace("1{65}", "1" * 65)
        try:
            output = function(value)
        except ValueError:
            output = "invalid_input"
        validation.append({"kind": name, "input": value, "output": output})
paths = [
    "slopsearx/saved_events.py",
    "slopsearx/saved_store.py",
    "slopsearx/saved_runner.py",
    "slopsearx/mcp/tools.py",
    "tests/test_saved_events.py",
    "tests/test_saved_events_valkey.py",
    "tests/test_saved_events_transport.py",
    "tests/test_saved_event_metrics.py",
]
result = {
    "baseline": "93db215de1c5b59ffdcac035a79ffcbd8e806762",
    "inputs_sha256": hashlib.sha256(raw).hexdigest(),
    "sources": {path: hashlib.sha256((root / path).read_bytes()).hexdigest() for path in paths},
    "normalization": "opaque event identity fixed to event-0123456789abcdefghijklmn; no other output normalization",
    "cases": rows,
    "validation": validation,
}
with args.output.open("x", encoding="utf-8") as output_file:
    json.dump(result, output_file, ensure_ascii=True, indent=2)
    output_file.write("\n")
