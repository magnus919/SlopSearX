import json  # noqa: I001 - pinned oracle import behavior
import argparse
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
L = [0xA1, 0xA2, 0xA4, 0xA7, 0xA8, 0xA9, 0xB1, 0xB2, 0xB3, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE]
T = [
    0xD4,
    0xA1,
    0xA2,
    0xA3,
    0xA4,
    0xA5,
    0xA6,
    0xA7,
    0xA9,
    0xAA,
    0xAB,
    0xAC,
    0xAD,
    0xAE,
    0xAF,
    0xB0,
    0xB1,
    0xB2,
    0xB4,
    0xB5,
    0xB6,
    0xB7,
    0xB8,
    0xBA,
    0xBB,
    0xBC,
    0xBD,
    0xBE,
]
samples = [bytes([0xA4, 0xD4, 0xA4, l, 0xA4, v, 0xA4, t]) for l in L for v in range(0xBF, 0xD4) for t in T]  # noqa: E741 - preserve source variable spelling
base = samples[0]
for at in range(8):
    for byte in range(256):
        samples.append(base[:at] + bytes([byte]) + base[at + 1 :] + b"A")
for n in range(1, 8):
    samples.append(base[:n])
    samples.append(base[:n] + b"AB")
rows = [[raw.hex(), list(map(ord, raw.decode("euc_kr", "replace")))] for raw in samples]
args.output.write_text(json.dumps(rows, separators=(",", ":")) + "\n")
print(len(rows))
