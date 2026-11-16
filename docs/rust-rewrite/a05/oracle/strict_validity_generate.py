"""Native 67+2 validity bits only; no Unicode mapping value arrays are read."""

import argparse
import hashlib
import json
import re
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--source-validity", type=Path, required=True)
p.add_argument("--icu-inputs", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
raw = a.source_validity.read_bytes()
if hashlib.sha256(raw).hexdigest() != "1ff44944ffa02ee27c9682a5fb2829e7f3e9a3f6677bdaa9b0fc19a63ab1bb8e":
    raise SystemExit("source validity input drift")
rows = json.loads(raw)["cases"]
native = []
for row in rows:
    name = row["canonical"]
    bits = bytes.fromhex(row["validity_bits_hex"])
    if name in ["hp-roman8", "koi8-t"]:
        path = a.icu_inputs / (name + ".ucm")
        b = path.read_bytes()
        pin = {
            "hp-roman8": "631da16f5b63ea77a6eee1ece7500f835d61754855aae5d9ec5d0b9e8b15acf7",
            "koi8-t": "171d77bcba2343f7e3fecba7bec4da625b079c5f27f24c4e54aae15a66e96802",
        }[name]
        if hashlib.sha256(b).hexdigest() != pin:
            raise SystemExit("ICU validity input drift " + name)
        nativebits = bytearray(32)
        for byte, flag in re.findall(r"<U[0-9A-F]+>\s+\\x([0-9A-F]{2})\s+\|([0-3])", b.decode()):
            if flag in ["0", "3"]:
                n = int(byte, 16)
                nativebits[n // 8] |= 1 << (n % 8)
        if bytes(nativebits) != bits:
            raise SystemExit("independent ICU validity differs source")
        bits = bytes(nativebits)
    native.append((name, bits))
s = (
    "// Native strict validity bits:67 source masks +2 independent IC"
    "U UCM masks.\n// See provenance/singlebyte-icu/manifest.json and "
    "retained source notices.\npub fn valid(name:&str,byte:u8)->Option"
    "<bool>{ let bits:&[u8;32]=match name {\n"
)
for name, bits in native:
    s += json.dumps(name) + "=> &[" + ",".join(map(str, bits)) + "],\n"
s += " _=>return None,\n};Some(bits[usize::from(byte)/8]&(1<<(byte%8))!=0) }\n"
a.output.write_text(s)
