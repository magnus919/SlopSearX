"""Temporary original strict-IDNA/whole-HTTPX observations; G08 retirement required."""

import argparse  # noqa: I001 - pinned oracle import behavior
import json
import hashlib
import sys
import encodings.idna
from pathlib import Path

if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires source3.14.7")
parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if (
    hashlib.sha256(Path(encodings.idna.__file__).read_bytes()).hexdigest()
    != "ea8d7f01422af0ad138e4bf58d4ae63112f3ceb68833c486d6c4d18252610fd8"
):
    raise SystemExit("source IDNA algorithm drift")
p = args.source
raw = p.read_bytes()
if hashlib.sha256(raw).hexdigest() != "edf887c2be153b76beb9cecf19d9cc332b019d7f7497a93167f731ced58717f4":
    raise SystemExit("whole selector input drift")
rows = []
for case in json.loads(raw)["cases"]:
    row = {"whole_httpx_source": case, "decoded_codepoints": None, "exception_metadata": []}
    try:
        row["decoded_codepoints"] = list(map(ord, bytes.fromhex(case["parameter_bytes_hex"]).decode("idna", "strict")))
    except UnicodeError as e:
        row["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
    rows.append(row)
args.output.write_text(
    json.dumps({"python": sys.version, "whole_source_sha256": hashlib.sha256(raw).hexdigest(), "cases": rows}, indent=2)
    + "\n"
)
