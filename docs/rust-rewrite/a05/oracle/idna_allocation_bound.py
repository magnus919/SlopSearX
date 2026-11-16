"""Derive private IDNA scratch sequence bounds from the reviewed data only."""

import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--input", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
raw = args.input.read_bytes()
pin = "5d6d1c88e4ccf80e10da18bb60d01d7d12616f9eb8c96100b255f2360c10bd8d"
if hashlib.sha256(raw).hexdigest() != pin:
    raise SystemExit("approved IDNA profile input drift")
data = json.loads(raw)
mapping = dict(data["compatibility_decomposition"])
cache = {}
active = set()


def expanded(point):
    if 0xAC00 <= point <= 0xD7A3:
        return 2 + bool((point - 0xAC00) % 28)
    if point not in mapping:
        return 1
    if point in cache:
        return cache[point]
    if point in active:
        raise SystemExit("cyclic decomposition")
    active.add(point)
    count = sum(expanded(child) for child in mapping[point])
    active.remove(point)
    cache[point] = count
    return count


maximum, witness = max((expanded(point), point) for point in mapping)
b2 = max(len(sequence) for _, sequence in data["b2"])
result = {
    "input_sha256": pin,
    "recursive_decomposition_max": maximum,
    "witness_codepoint": witness,
    "b2_max": b2,
    "decomposed_points_per_input_point_upper_bound": maximum * b2,
    "source_label_bytes_max": 1024,
    "decomposed_points_per_label_upper_bound": maximum * b2 * 1024,
    "scope": "Sequence counts only; no Vec capacity, integer scratch, total heap or RSS claim.",
}
args.output.write_text(json.dumps(result, indent=2) + "\n")
