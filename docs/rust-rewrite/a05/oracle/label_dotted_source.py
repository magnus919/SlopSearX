"""Actual source alias/module precedence and whole-HTTPX dotted label controls."""

import argparse  # noqa: I001 - pinned oracle import behavior
import codecs
import encodings
import hashlib
import httpx
import json
import sys
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--policy", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires3.14.7")
raw = a.policy.read_bytes()
if hashlib.sha256(raw).hexdigest() != "4951aac7dc41d9a92b1d6fdeaf6d1888307ff7fc84f878d9c0dbab216215fcf6":
    raise SystemExit("source policy drift")
labels = {"", ".", "..", "utf.8", "latin.1", "iso8859.1", "ansi.x3.4.1968", "utf..8", "iso8859..1", "cp.437"}
for row in json.loads(raw)["canonical_codecs"]:
    for label in row["labels"]:
        labels.update(
            [label, label.replace("_", "."), label.replace("_", ".."), label.replace("_", "._"), label.upper()]
        )
rows = []
for i, label in enumerate(sorted(labels)):
    row = {
        "id": str(i),
        "label": label,
        "source_normalized": encodings.normalize_encoding(label),
        "canonical": None,
        "lookup_exception_metadata": [],
        "whole_httpx_source": [],
    }
    try:
        row["canonical"] = codecs.lookup(label).name
    except Exception as e:
        row["lookup_exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    for form in ["direct", "rfc2231"]:
        header = (
            "text/plain; charset=" + label if form == "direct" else "text/plain; charset*=" + label + "''cp437"
        ).encode()
        whole = {
            "form": form,
            "raw_headers_hex": [[b"content-type".hex(), header.hex()]],
            "status": 200,
            "body_hex": "80",
            "exception_metadata": [],
        }
        try:
            r = httpx.Response(200, headers=[(b"content-type", header)], content=b"\x80")
            whole["charset_encoding"] = r.charset_encoding
            whole["encoding"] = r.encoding
            whole["decoded_codepoints"] = list(map(ord, r.text))
        except Exception as e:
            whole["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
        row["whole_httpx_source"].append(whole)
    rows.append(row)
a.output.write_text(json.dumps({"python": sys.version, "httpx": httpx.__version__, "cases": rows}, indent=2) + "\n")
print(
    json.dumps(
        {
            "cases": len(rows),
            "whole_httpx_cases": 2 * len(rows),
            "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest(),
        }
    )
)
