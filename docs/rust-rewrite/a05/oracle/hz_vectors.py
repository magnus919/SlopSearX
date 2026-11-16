import json  # noqa: I001 - pinned oracle import behavior
import zlib
import argparse
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
rows = []
for prefix in [b"", b"~{"]:
    cases = []
    for pair in range(65536):
        body = prefix + pair.to_bytes(2, "big") + b"~}"
        cases.append(list(map(ord, body.decode("hz", "replace"))))
    rows.append(cases)
args.output.write_bytes(zlib.compress(json.dumps(rows, separators=(",", ":")).encode(), 9))
print(len(rows) * 65536, args.output.stat().st_size)
