"""Actual pinned codec observations for every generated name and range control."""

import argparse
import json
import re
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--names", type=Path, required=True)
p.add_argument("--ucd", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
names = re.findall(r'\(\s*"([^"]+)",\s*\d+\s*\)', args.names.read_text(), re.M)
assert len(names) == 51662
for line in (args.ucd / "NamedSequences.txt").read_text().splitlines():
    if line and not line.startswith("#"):
        names.append(line.split(";")[0].strip())
names += [
    "HANGUL SYLLABLE GAX",
    "HANGUL SYLLABLE ",
    "CJK UNIFIED IDEOGRAPH-04E00",
    "CJK UNIFIED IDEOGRAPH-4e00",
    "CJK UNIFIED IDEOGRAPH-4E00 ",
    "SNOWMAN\0",
    " SNOWMAN",
    "SNOWMAN ",
]
for lo, hi, prefix in [
    (0x3400, 0x4DBF, "CJK UNIFIED IDEOGRAPH-"),
    (0x4E00, 0x9FFF, "CJK UNIFIED IDEOGRAPH-"),
    (0x17000, 0x187F7, "TANGUT IDEOGRAPH-"),
    (0x18D00, 0x18D08, "TANGUT IDEOGRAPH-"),
    (0x20000, 0x2A6DF, "CJK UNIFIED IDEOGRAPH-"),
    (0x2A700, 0x2B739, "CJK UNIFIED IDEOGRAPH-"),
    (0x2B740, 0x2B81D, "CJK UNIFIED IDEOGRAPH-"),
    (0x2B820, 0x2CEA1, "CJK UNIFIED IDEOGRAPH-"),
    (0x2CEB0, 0x2EBE0, "CJK UNIFIED IDEOGRAPH-"),
    (0x2EBF0, 0x2EE5D, "CJK UNIFIED IDEOGRAPH-"),
    (0x30000, 0x3134A, "CJK UNIFIED IDEOGRAPH-"),
    (0x31350, 0x323AF, "CJK UNIFIED IDEOGRAPH-"),
]:
    for cp in [lo - 1, lo, hi, hi + 1]:
        names.append(prefix + f"{cp:04X}")
# Lowercase controls exercise the actual codec, not a normalized expectation.
names = list(dict.fromkeys(names + [n.lower() for n in names]))
rows = []
for name in names:
    body = b"\\N{" + name.encode("utf8") + b"}"
    rows.append([name, list(map(ord, body.decode("unicode_escape", "replace")))])
args.output.write_text(json.dumps(rows, ensure_ascii=True, separators=(",", ":")) + "\n")
print(len(rows))
