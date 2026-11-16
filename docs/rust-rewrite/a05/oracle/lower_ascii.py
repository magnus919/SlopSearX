"""Pinned source lowercase preimages and substring controls; G08 temporary."""

import argparse
import json
import random
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
preimages = []
empty = []
for point in range(128, 0x110000):
    lower = chr(point).lower()
    if not lower:
        empty.append(point)
    if any(ord(c) < 128 for c in lower):
        preimages.append(dict(input=point, lower_codepoints=list(map(ord, lower))))
rng = random.Random(925)
rows = []
for text in ["", "İ", "K", "AAABaAAB", "DucKducKgo", "verİfy", "a\ud800a", "a\x00a", "😀challenge"]:
    for needle in ["", "i", "k", "aaab", "duckduckgo", "verify", "challenge"]:
        rows.append(dict(points=list(map(ord, text)), needle=needle, expected=needle in text.lower()))
for _ in range(2048):
    text = "".join(
        chr(rng.choice([0x130, 0x212A, 0xD800, 0, 0x10000] + list(range(32, 128)))) for _ in range(rng.randrange(128))
    )
    needle = rng.choice(["aa", "k", "i", "challenge", "verify"])
    rows.append(dict(points=list(map(ord, text)), needle=needle, expected=needle in text.lower()))
args.output.write_text(
    json.dumps(
        dict(exhaustive_nonascii_preimages=preimages, empty_lower_preimages=empty, cases=rows),
        ensure_ascii=True,
        indent=2,
    )
    + "\n"
)
