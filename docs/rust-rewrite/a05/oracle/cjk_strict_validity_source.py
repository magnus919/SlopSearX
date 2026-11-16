"""Actual source strict validity only; never emits HKSCS Unicode values."""

import argparse
import hashlib
import json
import sys
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires3.14.7")
names = [
    "big5",
    "big5hkscs",
    "cp932",
    "cp949",
    "cp950",
    "euc_jis_2004",
    "euc_jisx0213",
    "euc_jp",
    "euc_kr",
    "gb18030",
    "gb2312",
    "gbk",
    "johab",
    "shift_jis",
    "shift_jis_2004",
    "shift_jisx0213",
]
rows = []
for name in names:
    masks = []
    counts = []
    for width in [1, 2]:
        size = 256**width
        bits = bytearray(size // 8)
        valid = 0
        for n in range(size):
            try:
                str(n.to_bytes(width, "big"), name, "strict")
                bits[n // 8] |= 1 << (n % 8)
                valid += 1
            except UnicodeError:
                pass
        masks.append(bits)
        counts.append(valid)
    a.output.mkdir(exist_ok=True)
    out = a.output / (name + ".valid.bin")
    out.write_bytes(b"".join(masks))
    rows.append(
        {
            "canonical": name,
            "inputs": [256, 65536],
            "valid_counts": counts,
            "sha256": hashlib.sha256(out.read_bytes()).hexdigest(),
        }
    )
(a.output / "manifest.json").write_text(
    json.dumps(
        {"python": sys.version, "cases": rows, "contents": "validity booleans only; no Unicode mapping values"},
        indent=2,
    )
    + "\n"
)
print(json.dumps({"codecs": len(rows), "inputs": len(rows) * (256 + 65536)}))
