#!/usr/bin/env python3
"""Temporary offline CPython oracle; retire from final backend at G08."""

import argparse
import hashlib
import json
import sys
import unicodedata
import urllib.parse
from pathlib import Path

PINS = {
    "urllib.parse": "484b633b81d024da52649c7ff775c81b338de9f60585c94e5adfe335a27d9509",
    "unicodedata": "d79ecd27e8d62ca652acfd765ecb9b55a82192454eb8c7b1fc3b96ea0cd3711b",
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
        raise SystemExit("pinned Python/Unicode version required")
    sources = {}
    for name, module in [("urllib.parse", urllib.parse), ("unicodedata", unicodedata)]:
        path = Path(module.__file__)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != PINS[name]:
            raise SystemExit(f"{name} source pin drift")
        sources[name] = {"path": str(path), "sha256": digest}
    bits = bytearray(0x110000 // 8)
    failures = []
    for point in range(0x110000):
        value = "x" + chr(point) + "y"
        try:
            urllib.parse._checknetloc(value)
        except ValueError as error:
            bits[point // 8] |= 1 << (point % 8)
            failures.append({"point": point, "exception_class": type(error).__name__, "message": str(error)})
    payload = {
        "scope": (
            "actual urllib.parse._checknetloc for every codepoint surrounded by ASCII x/y; "
            "not full URL or adapter outcomes"
        ),
        "python_version": sys.version,
        "unicode_version": unicodedata.unidata_version,
        "sources": sources,
        "point_count": 0x110000,
        "membership_bits_hex": bits.hex(),
        "membership_bits_sha256": hashlib.sha256(bits).hexdigest(),
        "failures": failures,
    }
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
