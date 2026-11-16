"""Narrow source Unicode16 codec-label alphanumeric classifier, no runtime backend."""

import argparse
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--unicode-data", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
raw = a.unicode_data.read_bytes()
PIN = "ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f"
if hashlib.sha256(raw).hexdigest() != PIN:
    raise SystemExit("Unicode16 source input drift")
ranges = []
pending = None
for line in raw.decode().splitlines():
    f = line.split(";")
    c = int(f[0], 16)
    if f[1].endswith(", First>"):
        pending = (c, f[2])
        continue
    if f[1].endswith(", Last>"):
        start, category = pending
        pending = None
        if category[0] in "LN":
            ranges.append([start, c + 1])
    elif f[2][0] in "LN":
        ranges.append([c, c + 1])
merged = []
for start, end in ranges:
    if merged and merged[-1][1] == start:
        merged[-1][1] = end
    else:
        merged.append([start, end])
data = (
    json.dumps({"unicode_version": "16.0.0", "source_sha256": PIN, "ranges_half_open": merged}, indent=2) + "\n"
).encode()
a.output.write_bytes(data)
print(json.dumps({"ranges": len(merged), "sha256": hashlib.sha256(data).hexdigest()}))
