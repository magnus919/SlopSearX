"""Source registry aliases only; separate from direct codec-module names."""

import argparse
import ast
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--aliases", type=Path, required=True)
p.add_argument("--registry", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
raw = a.aliases.read_bytes()
registry = a.registry.read_bytes()
if hashlib.sha256(raw).hexdigest() != "1ee993f8eb4986999b6501381e53947be9a04443626cb48b4190bcff2e87d3b4":
    raise SystemExit("source aliases input drift")
if hashlib.sha256(registry).hexdigest() != "634dac6dfb1034ba03bf26d437a03315c4b4d38333081ef446c655b393666bde":
    raise SystemExit("registry metadata input drift")


def normalize(s):
    return "_".join("".join(c.lower() if c.isalnum() or c == "." else " " for c in s).split())


lookup = {
    normalize(name): r["canonical"]
    for r in json.loads(registry)["canonical_codecs"]
    for name in [r["canonical"], *r["labels"]]
}
module = ast.parse(raw)
aliases = next(
    ast.literal_eval(n.value)
    for n in module.body
    if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name) and n.targets[0].id == "aliases"
)
rows = []
skipped = []
for key, target in sorted(aliases.items()):
    if not key.isascii() or not target.isascii():
        raise SystemExit("unexpected nonASCII registry literal")
    if normalize(target) not in lookup:
        skipped.append([key, target])
        continue
    rows.append((key, lookup[normalize(target)]))
s = (
    (
        "// SPDX-License-Identifier: PSF-2.0\n// Actual aliases dictionary"
        " only; module-name registry remains separate.\n// Pinned CPython3"
        ".14.7 Lib/encodings/aliases.py; full LICENSE-PYTHON retained.\npu"
        "b(crate) fn canonical(name:&str)->Option<&'static str>{match nam"
        "e {\n"
    )
    + "".join(json.dumps(k) + "=>Some(" + json.dumps(v) + "),\n" for k, v in rows)
    + "_=>None,\n}}\n"
)
a.output.write_text(s)
print(
    json.dumps(
        {
            "aliases": len(rows),
            "unavailable_platform_aliases": skipped,
            "sha256": hashlib.sha256(s.encode()).hexdigest(),
        }
    )
)
