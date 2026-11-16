"""Manual Unicode16 routing source oracle; retired G08, never gate/runtime."""

import hashlib
import json
import pathlib
import re
import sys
import unicodedata

if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("A09 oracle requires Python 3.14.7 and Unicode 16.0.0, including optimized mode")
ranges = []
for p in range(0x110000):
    if re.fullmatch(r"\w", chr(p)):
        if ranges and ranges[-1][1] + 1 == p:
            ranges[-1][1] = p
        else:
            ranges.append([p, p])
p = pathlib.Path(__file__).resolve().parents[1] / "word16.json"
p.write_text(json.dumps(ranges, separators=(",", ":")) + "\n")
print(len(ranges), hashlib.sha256(p.read_bytes()).hexdigest())
