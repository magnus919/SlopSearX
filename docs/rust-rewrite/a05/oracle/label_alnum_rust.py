"""Format the pin-verified classifier as Rust; no Unicode backend or source oracle."""

import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--ranges", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
raw = args.ranges.read_bytes()
if hashlib.sha256(raw).hexdigest() != "a59e61c6d089745657942532e98ee0216ed2e174463715a3592100f52a8c0f49":
    raise SystemExit("Unicode16 classifier range input drift")
ranges = json.loads(raw)["ranges_half_open"]
prefix = """// SPDX-License-Identifier: Unicode-3.0
// Unicode16 source categories L*/N* exactly match pinned CPython3.14.7 isalnum.
// Scoped to codec-label normalization; provenance/idna/label-alnum-manifest.json.
pub(crate) fn is_alnum(c: char) -> bool {
    let c = u32::from(c);
    let i = RANGES.partition_point(|r| r.0 <= c);
    i != 0 && c < RANGES[i - 1].1
}
static RANGES: &[(u32, u32)] = &[
"""
output = prefix + "".join(f"    ({start}, {end}),\n" for start, end in ranges) + "];\n"
args.output.write_text(output)
print(hashlib.sha256(output.encode()).hexdigest())
