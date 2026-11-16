"""Actual nonincremental strict str(bytes,encoding) observations, not body decoding."""

import argparse
import codecs  # noqa: F401 - pinned oracle import behavior
import hashlib
import json
import sys
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--policy", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires source CPython3.14.7")
raw = a.policy.read_bytes()
if hashlib.sha256(raw).hexdigest() != "4951aac7dc41d9a92b1d6fdeaf6d1888307ff7fc84f878d9c0dbab216215fcf6":
    raise SystemExit("source policy drift")
policy = json.loads(raw)
inputs = [
    ("empty", b""),
    ("ascii", b"utf-8"),
    ("utf7_malformed", b"utf-7+!"),
    ("utf8sig_prefix1", b"\xef"),
    ("utf8sig_prefix2", b"\xef\xbb"),
    ("utf16le_bomless", "cp437".encode("utf-16le")),
    ("utf16be_bomless", "cp437".encode("utf-16be")),
    ("utf32le_bomless", "cp437".encode("utf-32le")),
    ("utf32be_bomless", "cp437".encode("utf-32be")),
    ("all_bytes", bytes(range(256))),
    ("utf16_bom", b"\xff\xfec\0p\x004\x003\x007\0"),
    ("utf8_nonascii", b"utf8\xc3\xa9"),
    ("nul", b"utf8\0"),
    ("esc_ascii", b"utf\\x2d8"),
    ("unicode_escape_ascii", b"\\u0063p437"),
    ("iso2022_state", b"utf\x1b$B8"),
    ("hz_state", b"utf~{!~}"),
    ("literal_replacement", "\ufffd".encode()),
    ("valid_surrogate_utf7", b"+2AA-"),
    ("odd_utf16", b"utf8!"),
    ("iso2022_runtime", b"\x1b.J\x1bN\0"),
    ("iso2022_prior_unicode", b"\xff\x1b.J\x1bN\0"),
]
rows = []
for codec in policy["canonical_codecs"]:
    for name, body in inputs:
        row = {
            "id": codec["canonical"] + "/" + name,
            "label": codec["canonical"],
            "body_hex": body.hex(),
            "decoded_codepoints": None,
            "exception_metadata": [],
        }
        try:
            row["decoded_codepoints"] = list(map(ord, str(body, codec["canonical"], "strict")))
        except Exception as e:
            row["exception_metadata"].append({"class": type(e).__name__, "message": str(e)})
        rows.append(row)
a.output.write_text(
    json.dumps({"python": sys.version, "policy_sha256": hashlib.sha256(raw).hexdigest(), "cases": rows}, indent=2)
    + "\n"
)
print(
    json.dumps(
        {
            "cases": len(rows),
            "sha256": hashlib.sha256(a.output.read_bytes()).hexdigest(),
            "exception_classes": sorted({e["class"] for r in rows for e in r["exception_metadata"]}),
        }
    )
)
