"""Actual pinned source observations only; not an input to native derivation."""

import argparse
import codecs
import hashlib
import importlib
import json
import platform
import sys
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if platform.python_implementation() != "CPython" or sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("source oracle requires pinned CPython3.14.7")
rows = []
body = bytes(range(256))
for name in ["koi8-t", "hp-roman8"]:
    module = importlib.import_module("encodings." + name.replace("-", "_"))
    path = Path(module.__file__)
    text = body.decode(name, "replace")
    if len(text) != 256:
        raise SystemExit("source single-byte invariant changed")
    rows.append(
        dict(
            canonical=name,
            body_hex=body.hex(),
            errors="replace",
            module_file=str(path),
            module_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
            resolved=codecs.lookup(name).name,
            decoded_codepoints=list(map(ord, text)),
            exception_metadata=[],
        )
    )
args.output.write_text(
    json.dumps(
        dict(python=sys.version, implementation=platform.python_implementation(), cases=rows),
        ensure_ascii=True,
        indent=2,
    )
    + "\n"
)
