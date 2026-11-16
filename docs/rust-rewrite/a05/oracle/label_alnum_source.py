"""Actual pinned CPython full-range classifier oracle; no native mapping data."""

import argparse
import hashlib
import json
import sys
import zlib
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--ranges", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires source3.14.7")
x = json.loads(a.ranges.read_bytes())
ranges = x["ranges_half_open"]
index = 0
bits = bytearray(0x110000 // 8)
differences = []
for c in range(0x110000):
    while index < len(ranges) and c >= ranges[index][1]:
        index += 1
    native = index < len(ranges) and ranges[index][0] <= c < ranges[index][1]
    actual = chr(c).isalnum()
    if actual:
        bits[c // 8] |= 1 << (c % 8)
    if native != actual:
        differences.append(c)
if differences:
    raise SystemExit("source classifier mismatch " + str(differences[:20]))
a.output.write_bytes(zlib.compress(bits, 9))
print(
    json.dumps(
        {
            "points": 0x110000,
            "differences": len(differences),
            "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest(),
        }
    )
)
