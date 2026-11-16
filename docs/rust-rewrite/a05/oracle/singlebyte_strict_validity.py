"""Temporary source validity-only oracle; emits no Unicode mapping values."""

import argparse
import codecs  # noqa: F401 - pinned oracle import behavior
import hashlib
import importlib
import json
import sys
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--policy", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires pinned3.14.7")
raw = a.policy.read_bytes()
if hashlib.sha256(raw).hexdigest() != "4951aac7dc41d9a92b1d6fdeaf6d1888307ff7fc84f878d9c0dbab216215fcf6":
    raise SystemExit("source policy input drift")
rows = []
for row in json.loads(raw)["canonical_codecs"]:
    if row.get("implementation_family") != "single_byte_table":
        continue
    bits = bytearray(32)
    for b in range(256):
        try:
            str(bytes([b]), row["canonical"], "strict")
            bits[b // 8] |= 1 << (b % 8)
        except UnicodeError:
            pass
    module = importlib.import_module(row["python_module"])
    rows.append(
        {
            "canonical": row["canonical"],
            "validity_bits_hex": bits.hex(),
            "module_sha256": hashlib.sha256(Path(module.__file__).read_bytes()).hexdigest(),
            "checked_inputs": 256,
        }
    )
if len(rows) != 69:
    raise SystemExit("singlebyte count drift")
a.output.write_text(json.dumps({"python": sys.version, "cases": rows}, indent=2) + "\n")
print(hashlib.sha256(a.output.read_bytes()).hexdigest())
