"""Temporary G08-retired, deterministic HTTPX/email selector source oracle.
No sockets, provider calls, adapter dispatch, or runtime fallback.
"""

import base64
import codecs
import email.message
import email.utils
import hashlib
import inspect
import json
import sys
from pathlib import Path

import httpx


def controls():
    for label in [
        b"utf-7",
        b"UTF8",
        b"latin1",
        b'utf"7',
        b"\x00",
        b"base64",
        b"rot-13",
        b"idna",
        b"undefined",
        b"bogus",
    ]:
        for wrap in [lambda x: x, lambda x: b'"' + x + b'"', lambda x: b"<" + x + b">"]:
            yield [(b"content-type", b"text/plain; charset=" + wrap(label))], False
    for label in [
        b"ascii",
        b"utf-8",
        b"utf-7",
        b"utf-16",
        b"utf-32",
        b"utf-8-sig",
        b"punycode",
        b"idna",
        b"rot-13",
        b"base64",
        b"undefined",
        b"\x00",
        b"unknown",
    ]:
        for value in [b"utf-7", b"%FFutf-7", b"utf-7%2B%21", b"%C3%A9", b"", b"%00"]:
            yield [(b"content-type", b"text/plain; charset*=" + label + b"''" + value)], False
    for a in [
        b"charset*",
        b"CHARSET*",
        b"charset*0",
        b"charset*00",
        b"charset*1",
        b"charset*0*",
        b"charset**",
        b"charset*+1",
    ]:
        for b in [b"charset*", b"CHARSET*0", b"charset*1", b"charset*2*"]:
            yield [(b"content-type", b"text/plain; " + a + b"=utf-; " + b + b"=7; charset*=utf-8''utf-8")], False
    for value in [
        b'"utf\\\\7"',
        b'"utf\\"7"',
        b'"utf\\\\\\"7"',
        b'"utf-7;bad"',
        b'"utf-7; charset=utf8',
        b"utf-8''%5Cu0075tf-7",
        b"utf-8''%5C%22utf-7",
        b"utf-8''<utf-7>",
    ]:
        for prefix in [b"charset=", b"charset*="]:
            yield [(b"Content-Type", b"text/plain; " + prefix + value + b"; charset=utf-16")], False
    for whitespace in [b" ", b"\t", b"\r\n", b"\x1c", b"\xc2\xa0", b"\xe2\x80\x83"]:
        yield (
            [
                (
                    b"content-type",
                    b"text/plain; " + whitespace + b"CHARSET" + whitespace + b"=" + whitespace + b"UTF-7" + whitespace,
                )
            ],
            False,
        )
    for other in [b"\xff", b"\xc3\xa9", b"\xed\xa0\x80", b"ascii"]:
        for value in [b"utf-7", b"ut\xc3\xa9f-7", b"utf-8''%C3%A9"]:
            yield (
                [
                    (b"Content-Type", b"text/plain; charset=" + value),
                    (b"other", other),
                    (b"content-type", b"text/plain; charset=utf-16"),
                ],
                False,
            )
    for n in [4300, 4301]:
        yield [(b"content-type", b"text/plain; foo*" + b"9" * n + b"=x; charset=utf-7")], False
    for value in [b"\x00", b"utf-8''%00", b"utf-7"]:
        yield [(b"content-type", b"text/plain; charset=" + value)], True


rows = []
for number, (headers, empty) in enumerate(controls()):
    response = httpx.Response(200, headers=headers, content=b"" if empty else b"x")
    row = {
        "id": f"boundary.{number:03}",
        "headers_b64": [[base64.b64encode(k).decode(), base64.b64encode(v).decode()] for k, v in headers],
        "body_empty": empty,
    }
    if empty:
        row["empty_text"] = response.text
        row["selected_encoding"] = "utf-8"
        row["selection_bypassed"] = True
    else:
        try:
            row["charset_encoding"] = response.charset_encoding
            row["selected_encoding"] = response.encoding
            row["source_registry_name"] = codecs.lookup(response.encoding).name
        except Exception as error:
            row["error"] = {"type": type(error).__name__, "message": str(error)}
    rows.append(row)
paths = [
    inspect.getsourcefile(httpx.Response.encoding.fget),
    inspect.getsourcefile(email.message),
    inspect.getsourcefile(email.utils),
]
print(
    json.dumps(
        {
            "schema_version": 1,
            "purpose": "temporary offline source oracle; retire G08",
            "python": sys.version,
            "oracle_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "source_sha256": {str(Path(p)): hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in paths},
            "records": rows,
        },
        ensure_ascii=True,
        indent=2,
    )
)
