"""Independent two-table derivation; only pinned ICU UCM inputs accepted.
No CPython/codecs/source expectations/native tables are read by this generator.
"""

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--koi8-t", type=Path, required=True)
parser.add_argument("--hp-roman8", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
inputs = [
    (
        "koi8-t",
        args.koi8_t,
        "171d77bcba2343f7e3fecba7bec4da625b079c5f27f24c4e54aae15a66e96802",
        "beda557dded134d3effaad83a426a72e87aa6372262d34622110d7e2c00c82e3",
        237,
    ),
    (
        "hp-roman8",
        args.hp_roman8,
        "631da16f5b63ea77a6eee1ece7500f835d61754855aae5d9ec5d0b9e8b15acf7",
        "3ab1586ed736dd798be7a724be74affacff64233ac96a591366099298f470c5e",
        255,
    ),
]
for name, path, pin, expected, count in inputs:
    if hashlib.sha256(path.read_bytes()).hexdigest() != pin:
        raise SystemExit("independent ICU input drift: " + name)
outputs = []
for name, path, pin, expected, count in inputs:
    mappings = {}
    for point, byte, flag in re.findall(r"<U([0-9A-F]+)>\s+\\x([0-9A-F]{2})\s+\|([0-3])", path.read_text()):
        if flag not in ["0", "3"]:
            continue  # Encoding-only fallback/substitution is not decoding data.
        point, byte = int(point, 16), int(byte, 16)
        if byte in mappings or point > 0x10FFFF or 0xD800 <= point <= 0xDFFF:
            raise SystemExit("invalid independent ICU record: " + name)
        mappings[byte] = point
    if len(mappings) != count:
        raise SystemExit("independent ICU mapping count drift: " + name)
    raw = b"".join(struct.pack(">I", mappings.get(byte, 0xFFFD)) for byte in range(256))
    if hashlib.sha256(raw).hexdigest() != expected:
        raise SystemExit("independent ICU output drift: " + name)
    outputs.append((name, raw))
args.output.mkdir(parents=True, exist_ok=True)
for name, raw in outputs:
    (args.output / (name + ".single.bin")).write_bytes(raw)
print(json.dumps({name: hashlib.sha256(raw).hexdigest() for name, raw in outputs}, sort_keys=True))
