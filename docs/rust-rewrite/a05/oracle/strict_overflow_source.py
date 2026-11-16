"""Actual strict punycode/IDNA overflow and precedence observations."""

import argparse  # noqa: I001 - pinned oracle import behavior
import encodings.idna
import encodings.punycode
import hashlib
import json
import sys
from pathlib import Path
from urllib.parse import quote_from_bytes
import httpx

p = argparse.ArgumentParser()
p.add_argument("--punycode-source", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires source3.14.7")
raw = a.punycode_source.read_bytes()
if hashlib.sha256(raw).hexdigest() != "27bf55786beefe0b8545b4bf4585fbfee62b2f4b9c7d8ad9c5c5f64b32dc925c":
    raise SystemExit("original punycode source drift")
inputs = [("original/" + str(i), "punycode", bytes.fromhex(r["input_hex"])) for i, r in enumerate(json.loads(raw))]
limit = sys.maxsize
for n in [
    2**31 - 1,
    2**31,
    limit - 20,
    limit - 5,
    limit - 4,
    limit - 3,
    limit - 2,
    limit - 1,
    limit,
    limit + 1,
    limit * 2,
]:
    encoded = encodings.punycode.generate_generalized_integer(n, 72)
    for prefix in [b"", b"a-"]:
        value = prefix + encoded
        tag = str(n) + "/" + prefix.hex()
        inputs.append(("puny-boundary/" + tag, "punycode", value))
        for lead in [b"", b"a.", b"prefix.", b"utf8\0.", b"xn--caf-dma.", b"xn--a."]:
            inputs.append(("idna-boundary/" + tag + "/" + lead.hex(), "idna", lead + b"xn--" + value))
    for suffix in [b"\xff", b"\xff-"]:
        inputs.append(("puny-later/" + str(n) + "/" + suffix.hex(), "punycode", encoded + suffix))
for length in [1019, 1020, 1021, 1024, 1025]:
    for lead in [b"", b"utf8\0.", b"xn--a.", b"xn--caf-dma."]:
        inputs.append(("idna-length/" + str(length) + "/" + lead.hex(), "idna", lead + b"xn--" + b"9" * length))
for lead, tail in [(b"xn--a.", b""), (b"", b".xn--a"), (b"xn--caf-dma.", b""), (b"", b".xn--caf-dma")]:
    inputs.append(
        ("idna-precedence/" + lead.hex() + "/" + tail.hex(), "idna", lead + b"xn--" + b"9" * 256 + b"a" + tail)
    )
rows = []
for ident, label, body in inputs:
    row = {"id": ident, "label": label, "body_hex": body.hex(), "decoded_codepoints": None, "exception_metadata": []}
    try:
        row["decoded_codepoints"] = list(map(ord, str(body, label, "strict")))
    except Exception as e:
        row["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    header = "text/plain; charset*=" + label + "''" + quote_from_bytes(body, safe="")
    whole = {"headers": [["content-type", header]], "body_hex": "80", "status": 200, "exception_metadata": []}
    try:
        response = httpx.Response(200, headers={"content-type": header}, content=b"\x80")
        whole["charset_encoding"] = response.charset_encoding
        whole["encoding"] = response.encoding
        whole["decoded_codepoints"] = list(map(ord, response.text))
    except Exception as e:
        whole["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    row["whole_httpx_source"] = whole
    rows.append(row)
a.output.write_text(json.dumps({"python": sys.version, "httpx": httpx.__version__, "cases": rows}, indent=2) + "\n")
print(
    json.dumps(
        {
            "cases": len(rows),
            "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest(),
            "exception_classes": sorted({e["class"] for r in rows for e in r["exception_metadata"]}),
        }
    )
)
