"""Export source-only metadata and 67 mapping arrays for independent native derivation.
The immutable whole-source capture remains unchanged; this is temporary G08 tooling.
"""

import argparse
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--policy", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
pin = "4951aac7dc41d9a92b1d6fdeaf6d1888307ff7fc84f878d9c0dbab216215fcf6"
raw = a.policy.read_bytes()
if hashlib.sha256(raw).hexdigest() != pin:
    raise SystemExit("source policy input drift")
rows = []
for row in json.loads(raw)["canonical_codecs"]:
    out = {k: row[k] for k in ["canonical", "labels", "implementation_family"] if k in row}
    if row.get("implementation_family") == "single_byte_table" and row["canonical"] not in ["hp-roman8", "koi8-t"]:
        out["decoded"] = row["decoded"]
    rows.append(out)
data = (
    json.dumps({"source_policy_sha256": pin, "canonical_codecs": rows}, ensure_ascii=False, indent=2) + "\n"
).encode()
if hashlib.sha256(data).hexdigest() != "634dac6dfb1034ba03bf26d437a03315c4b4d38333081ef446c655b393666bde":
    raise SystemExit("filtered source output drift")
a.output.write_bytes(data)
