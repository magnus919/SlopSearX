"""Temporary actual source ACE/strict outcome corpus; no native derivation input."""

import argparse
import encodings.idna
import encodings.punycode
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires CPython3.14.7")
for m, pin in [(encodings.idna, "ea8d7f01422af0ad138e4bf58d4ae63112f3ceb68833c486d6c4d18252610fd8")]:
    if hashlib.sha256(Path(m.__file__).read_bytes()).hexdigest() != pin:
        raise SystemExit("source algorithm drift")
out = bytearray()
counts = [0, 0, 0]
for c in range(0x110000):
    encoded = chr(c).encode("punycode")
    label = b"xn--" + encoded
    try:
        decoded = label.decode("idna", "strict")
        flag = 1 if decoded.isascii() else 2
    except UnicodeError:
        flag = 0
    counts[flag] += 1
    out.extend(struct.pack(">HB", len(encoded), flag))
    out.extend(encoded)
a.output.write_bytes(zlib.compress(out, 9))
print(
    json.dumps(
        {
            "points": 0x110000,
            "counts_unicode_failure_ascii_nonascii": counts,
            "decoded_bytes": len(out),
            "compressed_bytes": a.output.stat().st_size,
            "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest(),
        }
    )
)
