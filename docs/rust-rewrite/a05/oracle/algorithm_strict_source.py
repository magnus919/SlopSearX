"""Actual strict outcomes over existing complete Unicode/escape source vectors."""

import argparse  # noqa: I001 - pinned oracle import behavior
import hashlib
import json
import sys
import zlib
import warnings
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--tests", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires3.14.7")
inputs = [
    ("utf-7", "unicode/utf-7.json.zlib", "54452e9d49add51279ce05fa0dc523dc96469bcd75a69c7b4b2924a06914b044"),
    (
        "raw-unicode-escape",
        "raw-unicode-escape-source.json",
        "63425d0a87921e1a64af7a1e30472f03264d30495c08fcf8c6b95910f53c675d",
    ),
    (
        "unicode-escape",
        "unicode-escape-source.json",
        "b707048a3943128a985ef76bddada3ac153d7c75b3b6208200d01a0dde70ea45",
    ),
    ("unicode-escape", "unicode-name-source.json", "d4af24adcbe1aba41380e983ab29f549178b98df58926c5b2dedad9e53735f66"),
]
rows = []
a.output.mkdir(exist_ok=True)
for codec, name, pin in inputs:
    raw = (a.tests / name).read_bytes()
    if not pin or hashlib.sha256(raw).hexdigest() != pin:
        raise SystemExit("original algorithm corpus drift " + name)
    source = json.loads(zlib.decompress(raw) if name.endswith(".zlib") else raw)
    out = bytearray()
    counts = [0, 0]
    for r in source:
        if name == "unicode-name-source.json":
            body = ("\\N{" + r[0] + "}").encode()
            expected = r[1]
        else:
            body = bytes.fromhex(r.get("body_hex", r.get("input_hex")))
            expected = r.get("points", r.get("expected_codepoints"))
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", DeprecationWarning)
            try:
                points = list(map(ord, str(body, codec, "strict")))
                if points != expected:
                    raise SystemExit("strict valid text differs original full source " + name)
                mode = 1
            except UnicodeError:
                mode = 0
        out.append(mode)
        counts[mode] += 1
    file = name.replace("/", "-") + ".strict.zlib"
    data = zlib.compress(out, 9)
    (a.output / file).write_bytes(data)
    rows.append(
        {
            "codec": codec,
            "source": name,
            "source_sha256": pin,
            "cases": len(source),
            "file": file,
            "sha256": hashlib.sha256(data).hexdigest(),
            "counts_unicode_valid": counts,
        }
    )
(a.output / "manifest.json").write_text(json.dumps({"python": sys.version, "groups": rows}, indent=2) + "\n")
print(json.dumps({"groups": len(rows), "cases": sum(r["cases"] for r in rows)}))
