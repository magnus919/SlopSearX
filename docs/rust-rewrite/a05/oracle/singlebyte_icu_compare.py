"""Complete comparison verifier, separate from independent native derivation."""

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--native", type=Path, required=True)
parser.add_argument("--runtime-rust", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
source = json.loads(args.source.read_text())
if source["implementation"] != "CPython" or not source["python"].startswith("3.14.7 "):
    raise SystemExit("source identity mismatch")
rows = source["cases"]
if len(rows) != 2 or {row["canonical"] for row in rows} != {"koi8-t", "hp-roman8"}:
    raise SystemExit("source case inventory mismatch")
rust = args.runtime_rust.read_text()
results = []
for row in rows:
    if row["body_hex"] != bytes(range(256)).hex() or row["errors"] != "replace" or row["exception_metadata"]:
        raise SystemExit("source input/exception metadata mismatch")
    raw = (args.native / (row["canonical"] + ".single.bin")).read_bytes()
    if len(raw) != 1024:
        raise SystemExit("native table size mismatch")
    points = list(struct.unpack(">256I", raw))
    if points != row["decoded_codepoints"]:
        raise SystemExit("complete source/native mismatch: " + row["canonical"])
    arm = re.search(r'"' + re.escape(row["canonical"]) + r'" => Some\(&([^)]*)\)', rust)
    if arm is None:
        raise SystemExit("native table registration missing")
    table = re.search(r"static " + arm.group(1) + r": \[u32; 256\] = \[(.*?)\];", rust, re.S)
    if table is None:
        raise SystemExit("runtime table missing")
    runtime = [int(value, 16) for value in re.findall(r"0x[0-9a-f]+", table.group(1))]
    if points != runtime:
        raise SystemExit("complete current runtime/native mismatch: " + row["canonical"])
    results.append(
        dict(
            canonical=row["canonical"],
            complete_points_compared=256,
            current_runtime_table=arm.group(1),
            native_sha256=hashlib.sha256(raw).hexdigest(),
            source_equal=True,
            existing_donor_equal=True,
        )
    )
args.output.write_text(json.dumps(results, indent=2) + "\n")
