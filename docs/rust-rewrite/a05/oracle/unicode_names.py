import argparse  # noqa: I001 - pinned oracle import behavior
import json
import hashlib  # noqa: F401 - pinned oracle import behavior
import re
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--ucd", type=Path, required=True)
p.add_argument("--cpython", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
names = {}
for line in (args.ucd / "UnicodeData.txt").read_text().splitlines():
    fields = line.split(";")
    code, name = fields[:2]
    if not name.startswith("<"):
        names[name] = int(code, 16)
for line in (args.ucd / "NameAliases.txt").read_text().splitlines():
    if not line or line.startswith("#"):
        continue
    code, name, _ = line.split(";")
    names[name] = int(code, 16)
L = ["G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P", "H"]
V = [
    "A",
    "AE",
    "YA",
    "YAE",
    "EO",
    "E",
    "YEO",
    "YE",
    "O",
    "WA",
    "WAE",
    "OE",
    "YO",
    "U",
    "WEO",
    "WE",
    "WI",
    "YU",
    "EU",
    "YI",
    "I",
]
T = [
    "",
    "G",
    "GG",
    "GS",
    "N",
    "NJ",
    "NH",
    "D",
    "L",
    "LG",
    "LM",
    "LB",
    "LS",
    "LT",
    "LP",
    "LH",
    "M",
    "B",
    "BS",
    "S",
    "SS",
    "NG",
    "J",
    "C",
    "K",
    "T",
    "P",
    "H",
]
for i, l in enumerate(L):  # noqa: E741 - preserve source variable spelling
    for j, v in enumerate(V):
        for k, t in enumerate(T):
            names["HANGUL SYLLABLE " + l + v + t] = 0xAC00 + (i * 21 + j) * 28 + k
# Pinned CPython source is authoritative for algorithmic accepted ranges.
s = (args.cpython / "Modules/unicodename_db.h").read_text()
ranges = re.search(r"derived_name_ranges\[\] = \{(.*?)\};", s, re.S).group(1)
ranges = [
    (int(a, 16), int(b, 16), int(c)) for a, b, c in re.findall(r"\{(0x[0-9A-F]+), (0x[0-9A-F]+), (\d+)\}", ranges)
]
args.output.mkdir(parents=True, exist_ok=True)
text = (
    (
        "// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0\n// G"
        "enerated from Unicode16 UnicodeData/NameAliases and CPython3.14."
        "7 Hangul rules.\n// Complete notices and exact input/output attri"
        "bution accompany this package.\npub(crate) static NAMES: &[(&str,"
        "u32)] = &[\n"
    )
    + "".join("(" + json.dumps(n) + "," + str(c) + "),\n" for n, c in sorted(names.items()))
    + "];\n"
)
(args.output / "unicode_names.rs").write_text(text)
(args.output / "unicode_name_ranges.rs").write_text(
    (
        "// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0\n// C"
        "Python3.14.7 Modules/unicodename_db.h derived_name_ranges, Unico"
        "de16.\npub(crate) const RANGES: &[(u32,u32,u8)] = &[\n"
    )
    + "".join(f"({a},{b},{c}),\n" for a, b, c in ranges)
    + "];\n"
)
print("names", len(names), "ranges", len(ranges))
