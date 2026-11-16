"""Temporary whole-HTTPX IDNA selector observations; no runtime implementation."""

import argparse  # noqa: I001 - pinned oracle import behavior
import codecs
import hashlib
import json
import sys
from pathlib import Path
import httpx
from urllib.parse import quote_from_bytes

if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires pinned CPython3.14.7")
parser = argparse.ArgumentParser()
parser.add_argument("--policy", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
p = args.policy
if hashlib.sha256(p.read_bytes()).hexdigest() != "4951aac7dc41d9a92b1d6fdeaf6d1888307ff7fc84f878d9c0dbab216215fcf6":
    raise SystemExit("source policy input drift")
policy = json.loads(p.read_text())
labels = sorted({label for row in policy["canonical_codecs"] for label in row["labels"]})
values = [("alias:" + x, x.encode()) for x in labels]
values += [
    ("control:" + str(i), x)
    for i, x in enumerate(
        [
            b"xn--caf-dma",
            b"xn--utf-8-",
            b"xn--utf8-",
            b"xn--",
            b"xn--a",
            b"XN--CAF-DMA",
            b"utf-8\0",
            b"utf-8.xn--caf-dma",
            b"xn--utf-8-.",
            b"\xff",
            b"utf-8",
            b"UTF-8",
            b"utf-8" + b"a" * 2000,
            b"xn--" + b"a" * 2000,
            b"\\u0075tf8",
            b"xn--\xff",
            b"utf8\0.xn--caf-dma",
            b"utf8\0.xn--a",
            b"xn--caf-dma.utf8\0",
            b"xn--a.utf8\0",
            b"xn--caf-dma\0",
            b"\0xn--caf-dma",
            b"utf8\0.xn--6la",
            b"utf8\0.xn--tma",
            b"utf8\0.xn--e28h",
            b"utf8\0.xn--CAF-dma",
            b"utf8\0.XN--CAF-DMA",
            b"utf8\0.xn--caf-dma.xn--a",
            b"utf8\0.xn--a.xn--caf-dma",
            b"xn--CAF-dma",
            b"xn--caf-DMA",
            b"xn--caf-dma.xn--a",
            b"xn--a.xn--caf-dma",
        ]
    )
]
rows = []
for case, value in values:
    header = "text/plain; charset*=idna''" + quote_from_bytes(value, safe="")
    row = {
        "id": case,
        "parameter_bytes_hex": value.hex(),
        "headers": [["content-type", header]],
        "body_hex": "80",
        "exception_metadata": [],
    }
    try:
        response = httpx.Response(200, headers={"content-type": header}, content=b"\x80")
        row["charset_encoding"] = response.charset_encoding
        row["encoding"] = response.encoding
        try:
            row["effective_canonical"] = codecs.lookup(response.encoding).name
        except Exception as e:
            row["lookup_exception"] = {"class": type(e).__name__, "message": str(e)}
        row["decoded_codepoints"] = list(map(ord, response.text))
    except Exception as e:
        row["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    rows.append(row)
args.output.write_text(
    json.dumps(
        {
            "python": sys.version,
            "httpx": httpx.__version__,
            "policy_sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
            "cases": rows,
        },
        ensure_ascii=True,
        indent=2,
    )
    + "\n"
)
print(
    json.dumps(
        {
            "labels": len(labels),
            "cases": len(rows),
            "output_sha256": hashlib.sha256(args.output.read_bytes()).hexdigest(),
        }
    )
)
