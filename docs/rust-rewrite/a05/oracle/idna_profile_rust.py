"""Format approved immutable profile data as native static Rust; offline tooling."""

import argparse
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--input", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
raw = a.input.read_bytes()
if hashlib.sha256(raw).hexdigest() != "5d6d1c88e4ccf80e10da18bb60d01d7d12616f9eb8c96100b255f2360c10bd8d":
    raise SystemExit("approved profile data drift")
x = json.loads(raw)
out = [
    (
        "// Source permissions: retained original notices; see provenance"
        "/idna/manifest.json.\n// Derived profile data: exact sources and "
        "original Unicode3.2/RFC notices are\n// retained under provenance"
        "/idna/. Unicode16 portions retain LICENSE-UNICODE.\n// See proven"
        "ance/idna/manifest.json; no Python runtime is used.\n"
    )
]
for key, name in [("b2", "B2"), ("compatibility_decomposition", "DECOMP")]:
    flat = []
    index = []
    for c, seq in x[key]:
        if not 0 <= c <= 0x10FFFF or not seq or any(not 0 <= n <= 0x10FFFF for n in seq):
            raise SystemExit("invalid profile mapping")
        index.append((c, len(flat), len(seq)))
        flat += seq
    out.append(
        f"pub static {name}_INDEX: &[(u32,usize,usize)] = &[\n"
        + "".join(f"({c},{start},{count}),\n" for c, start, count in index)
        + "];\n"
    )
    out.append(
        f"pub static {name}_DATA: &[u32] = &[\n"
        + "".join(",".join(map(str, flat[i : i + 16])) + ",\n" for i in range(0, len(flat), 16))
        + "];\n"
    )
for key, name, typ in [
    ("combining_classes", "CCC", "(u32,u8)"),
    ("canonical_composition", "COMPOSE", "(u32,u32,u32)"),
    ("profile_ranges", "FLAGS", "(u32,u32,u8)"),
]:
    out.append(
        f"pub static {name}: &[{typ}] = &[\n" + "".join("(" + ",".join(map(str, r)) + "),\n" for r in x[key]) + "];\n"
    )
a.output.write_text("".join(out))
