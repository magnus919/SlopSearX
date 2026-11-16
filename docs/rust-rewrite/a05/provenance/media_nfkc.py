#!/usr/bin/env python3
"""Offline developer generator; no provider/Python runtime backend dependency."""

import argparse
import hashlib
import json
import textwrap
from pathlib import Path

PIN = "9f6d9cb6e81031954d91cc03084237592e9d7bb2224f22cab5a5fba8cebefabd"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    raw = args.source.read_bytes()
    if hashlib.sha256(raw).hexdigest() != PIN:
        raise SystemExit("media NFKC source input drift")
    values = json.loads(raw)["nfkc_url_delimiters"]
    if (
        len(values) != 19
        or any(type(value) is not int or not 0 <= value <= 0x10FFFF for value in values)
        or values != sorted(set(values))
    ):
        raise SystemExit("media NFKC source shape drift")
    header = (
        "// SPDX-License-Identifier: Unicode-3.0\n"
        "// Generated subset of contracts/unicode/python-text-16.0.0.json.\n"
        "// Retain contracts/unicode/{provenance.json,LICENSE.txt} with distribution.\n"
        f"// Source SHA256: {PIN}\n"
    )
    output = (
        header
        + "pub(super) const DELIMITERS: &[u32] = &[\n"
        + textwrap.fill(", ".join(map(str, values)) + ",", width=96, initial_indent="    ", subsequent_indent="    ")
        + "\n"
        + "];\n"
    )
    args.output.write_text(output)


if __name__ == "__main__":
    main()
