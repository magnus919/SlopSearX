"""Offline native mapping derivation; reads only three independently pinned inputs.
No codec imports, CPython source path, existing HKSCS table or decoded-source
Unicode data is accepted. Source validity-only bits select compatibility scope.
"""

import argparse
import hashlib
import re
import struct
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--index", type=Path, required=True)
p.add_argument("--core", type=Path, required=True)
p.add_argument("--validity", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
p.add_argument("--single-output", type=Path, required=True)
args = p.parse_args()
for file, pin in [
    (args.index, "08e24270c8e95d998c994c03f907e972480dc01f58743e078654cc466203c8ff"),
    (args.core, "6cdaa4f545baf91372ce0a063b5751fc222a9dde2d3bcc87bffd08f7e5a85de6"),
    (args.validity, "9109699c47243cbcd085314c5e7c2bb81646f2d3397375a35a52fcc62533381b"),
]:
    if hashlib.sha256(file.read_bytes()).hexdigest() != pin:
        raise SystemExit("independent input drift: " + str(file))
index = {}
for line in args.index.read_text().splitlines():
    if line and not line.startswith("#"):
        pointer, point, *_ = line.split()
        index[int(pointer)] = [int(point, 16)]
# Normative WHATWG Big5 decoder special-pointer cases, under source BSD3 grant.
for pointer, points in [(1133, [0xCA, 0x304]), (1135, [0xCA, 0x30C]), (1164, [0xEA, 0x304]), (1166, [0xEA, 0x30C])]:
    index[pointer] = points
# Regex reads literal assignments, never resolves the XML's external DTD.
core = {
    int("".join(raw.split()), 16): int(point, 16)
    for point, raw in re.findall(r'<a u="([0-9A-F]+)" b="([0-9A-F ]+)"', args.core.read_text())
}
legacy = {0xA145, 0xA14E, 0xA1C2, 0xA1E3, 0xA1F2, 0xA1F3, 0xA241, 0xA242, 0xA244, 0xA246, 0xA247}
valid = args.validity.read_bytes()
output = bytearray()
for pair in range(65536):
    if not valid[pair // 8] & (1 << (pair % 8)):
        continue
    first, second = pair.to_bytes(2, "big")
    pointer = (first - 0x81) * 157 + second - (0x40 if second < 0x7F else 0x62)
    points = [core[pair]] if pair in legacy else index[pointer]
    if not all(0 <= point <= 0x10FFFF for point in points):
        raise SystemExit("invalid independently mapped codepoint")
    output += struct.pack(">HII", pair, points[0], points[1] if len(points) == 2 else 0xFFFFFFFF)
if len(output) != 18402 * 10:
    raise SystemExit("unexpected mapping count")
digest = hashlib.sha256(output).hexdigest()
if digest != "fc9461564284bac32fca61da4fb07b5b8a144b6943092425f0ac51876d1b344a":
    raise SystemExit("independent output drift")
# WHATWG Big5 decoder returns ASCII bytes unchanged; all other lone bytes
# require a trail or fail. No decoded source mapping contributes this table.
single = b"".join(struct.pack(">I", byte if byte < 128 else 0xFFFFFFFF) for byte in range(256))
args.output.write_bytes(output)
args.single_output.write_bytes(single)
print(digest)
