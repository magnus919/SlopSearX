"""Temporary source validity-only oracle. Stores no decoded Unicode values."""

import argparse
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
bits = bytearray(8192)
for first in range(0x81, 0xFF):
    for second in list(range(0x40, 0x7F)) + list(range(0xA1, 0xFF)):
        pair = (first << 8) | second
        try:
            pair.to_bytes(2, "big").decode("big5hkscs", "strict")
        except UnicodeDecodeError:
            continue
        bits[pair // 8] |= 1 << (pair % 8)
args.output.write_bytes(bits)
