"""Temporary pinned offline CPython/HTTPX storage evidence; retire at G08."""

import argparse  # noqa: I001 - pinned oracle import behavior
import ctypes
import hashlib
import json
import sys
import sysconfig
from pathlib import Path
import httpx
import lxml

ROOT = Path(__file__).resolve().parents[4]


class UnicodeHead(ctypes.Structure):
    _fields_ = [
        ("refcount", ctypes.c_ssize_t),
        ("type", ctypes.c_void_p),
        ("length", ctypes.c_ssize_t),
        ("hash", ctypes.c_ssize_t),
        ("state", ctypes.c_uint32),
    ]


def storage(text):
    h = UnicodeHead.from_address(id(text))
    assert h.length == len(text)
    kind = (h.state >> 2) & 7
    ascii_flag = bool((h.state >> 6) & 1)
    capacity = 127 if ascii_flag else {1: 255, 2: 65535, 4: 1114111}[kind]
    maximum = max(map(ord, text), default=0)
    expected = 127 if maximum <= 127 else 255 if maximum <= 255 else 65535 if maximum <= 65535 else 1114111
    return dict(
        kind=kind,
        ascii=ascii_flag,
        capacity=capacity,
        maximum=maximum,
        minimal=capacity == expected,
        codepoints=list(map(ord, text)),
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--output", required=True)
    args = ap.parse_args()
    assert sys.version_info[:3] == (3, 14, 7) and not sysconfig.get_config_var("Py_GIL_DISABLED")
    assert [storage(x)["kind"] for x in ["A", "é", "Ā", "😀"]] == [1, 1, 2, 4]
    policy_path = ROOT / "docs/rust-rewrite/a05/source-codec-policy.json"
    policy = json.loads(policy_path.read_text())
    codecs = [c for c in policy["canonical_codecs"] if c["http_text_applicability"] == "applicable_text_decoder"]
    assert len(codecs) == 108
    rows = []
    for c in codecs:
        codec = c["canonical"]
        inputs = [("all_bytes", bytes(range(256)))]
        for name, text in [
            ("empty", ""),
            ("ASCII", "ABC"),
            ("Latin1", "é"),
            ("BMP", "Ā"),
            ("astral", "😀"),
            ("NUL", "\0"),
            ("high", "\ud800"),
            ("pair", "\ud800\udc00"),
            ("embedded_high", "<h3>A\ud800B</h3>"),
            ("astral_anywhere", "😀<h3>A\ud800B</h3>"),
        ]:
            try:
                inputs.append((name, text.encode(codec)))
            except Exception as e:
                rows.append(dict(canonical=codec, input=name, encode_error=dict(type=type(e).__name__, message=str(e))))
        for name, body in inputs:
            row = dict(canonical=codec, input=name, body_hex=body.hex())
            try:
                row["storage"] = storage(
                    httpx.Response(200, headers={"content-type": "text/html;charset=" + codec}, content=body).text
                )
            except Exception as e:
                row["decode_error"] = dict(type=type(e).__name__, message=str(e))
            rows.append(row)
    parser = Path(lxml.__file__).parent / "parser.pxi"
    result = dict(
        status="source observations not runtime memory access",
        python=sys.version,
        httpx=httpx.__version__,
        lxml=lxml.__version__,
        parser_source_path=str(parser),
        parser_source_sha256=hashlib.sha256(parser.read_bytes()).hexdigest(),
        policy_sha256=hashlib.sha256(policy_path.read_bytes()).hexdigest(),
        calibration=(
            "CPython3.14.7 GIL PyASCIIObject layout from pinned Include/cpyth"
            "on/unicodeobject.h; checked length and four known kinds"
        ),
        cases=rows,
    )
    Path(args.output).write_text(json.dumps(result, ensure_ascii=True, indent=2) + "\n")
    mismatches = [r for r in rows if r.get("storage", {}).get("minimal") is False]
    print(
        "codecs",
        len(codecs),
        "rows",
        len(rows),
        "decoded",
        sum("storage" in r for r in rows),
        "overwide",
        len(mismatches),
    )


if __name__ == "__main__":
    main()
