"""Actual codec normalization plus ordered raw-header HTTPX outcomes."""

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
labels = sorted({l for r in json.loads(raw)["canonical_codecs"] for l in r["labels"]})  # noqa: E741 - preserve source variable spelling
for c in ["\U00011db0", "\U0001e6c0", "\U00016ea0", "\u0345", "\u05b0", "\u0903"]:
    labels.extend(
        prefix + c + suffix for prefix, suffix in [("cp", "037"), ("cp", "437"), ("latin", "1"), ("utf", "8")]
    )
rows = []
for i, label in enumerate(labels):
    row = {
        "id": str(i),
        "label": label,
        "source_normalized": encodings.normalize_encoding(label),
        "lookup_exception_metadata": [],
    }
    try:
        row["canonical"] = codecs.lookup(label).name
    except Exception as e:
        row["canonical"] = None
        row["lookup_exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    header = ("text/plain; charset*=" + label + "''cp437").encode()
    whole = {
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
    row["whole_httpx_source"] = whole
    rows.append(row)
a.output.write_text(json.dumps({"python": sys.version, "httpx": httpx.__version__, "cases": rows}, indent=2) + "\n")
print(json.dumps({"cases": len(rows), "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest()}))
