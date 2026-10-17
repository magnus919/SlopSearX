"""Actual HTTPX/email charset selection and whole arXiv outcomes; no providers."""

import asyncio
import base64
import email.message
import email.utils
import hashlib
import inspect
import json
import runpy
from pathlib import Path

import httpx

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))


def controls():
    yield "absent", []
    values = [
        b"",
        b"text/plain",
        b"garbage; charset=utf-16",
        b"text/plain; charset",
        b"text/plain; charset=",
        b"text/plain; charset=bogus",
        b"text/plain; CHARSET=UTF-7",
        b'text/plain; charset="utf-7"',
        b'text/plain; charset="utf-7;bad"; charset=utf-8',
        b'text/plain; charset="utf-7; charset=utf-8',
        b'text/plain; charset="utf\\"7"; charset=utf-8',
        b"text/plain; charset=<utf-7>",
        b"text/plain; charset=utf-7; charset=utf-8",
        b"text/plain; charset; charset=utf-7",
        b"text/plain; charset*=us-ascii''utf-7",
        b"text/plain; charset*=unknown''utf-7",
        b"text/plain; charset*=utf-8''%FFutf-7",
        b"text/plain; charset*=utf-8''utf-7",
        b"text/plain; charset*=utf-8'en'utf-7",
        b"text/plain; charset*=utf-7''utf-7%2B%21",
        b"text/plain; charset*=utf-16''utf-7",
        b"text/plain; charset*=rot-13''utf-7",
        b"text/plain; charset*=utf-8''%C3%A9",
        b"text/plain; charset*=utf-8''%75tf-7",
        b"text/plain; charset*=utf-8''utf-%ZZ7",
        b"text/plain; charset*1=7; charset*0=utf-",
        b"text/plain; charset*2=7; charset*0=utf-",
        b"text/plain; charset*0*=us-ascii''utf-; charset*1=7",
        b"text/plain; charset*0=utf-; charset*1*=7",
        b"text/plain; charset*=utf-8''utf-8; charset*0=utf-7",
        b"text/plain; charset*=utf-8''utf-7; charset=utf-8",
        b"text/plain; charset*0=utf-; charset*0=7",
        b"text/plain; charset*000=utf-7",
        b"text/plain; charset**=utf-7; charset=utf-8",
        b"text/plain; charset*+1=utf-7",
        b"text/plain; charset=utf-7\x1c",
        b"text/plain; charset=ut\xc3\xa9f-7",
        b"text/plain; charset=utf-7, text/html; charset=utf-8",
    ]
    for index, value in enumerate(values):
        yield f"value_{index}", [(b"Content-Type", value)]
    for index, headers in enumerate(
        [
            [(b"Content-Type", b"text/plain; charset=utf-7"), (b"content-type", b"text/plain; charset=utf-8")],
            [(b"Content-Type", b"text/plain"), (b"content-type", b"text/plain; charset=utf-7")],
            [(b"Content-Type", b"text/plain; charset=ut\xc3\xa9f-7"), (b"Other", b"\xff")],
            [(b"Content-Type", b"text/plain; charset=utf-7"), (b"Other", b"\xc3\xa9")],
            [(b"Content-Type", b"text/plain; charset=utf-7"), (b"Other", b"\xff")],
        ]
    ):
        yield f"headers_{index}", headers
    yield (
        "irrelevant_continuation_integer_limit",
        [(b"content-type", b"text/plain; foo*" + b"9" * 4301 + b"=x; charset=utf-7")],
    )


async def main():
    records = []
    for name, headers in controls():
        response = httpx.Response(200, headers=headers, content=BASE["FEED"].encode())
        observation = {
            "header_encoding": response.headers.encoding,
            "content_type": response.headers.get("content-type"),
        }
        try:
            observation["charset_encoding"] = response.charset_encoding
            observation["selected_encoding"] = response.encoding
        except Exception as error:  # noqa: BLE001 - actual source observation
            observation["error"] = {"type": type(error).__name__, "message": str(error)}
        case = {
            "id": "arxiv.charset." + name,
            "engine": "arxiv",
            "query": "test",
            "responses": [
                {"status": 200, "body_b64": base64.b64encode(BASE["FEED"].encode()).decode(), "headers": headers}
            ],
        }
        row = await BASE["capture"](case)
        row["responses"] = [
            {
                "status": 200,
                "body_b64": case["responses"][0]["body_b64"],
                "headers_b64": [[base64.b64encode(k).decode(), base64.b64encode(v).decode()] for k, v in headers],
            }
        ]
        row["selector"] = observation
        records.append(row)
    paths = [
        inspect.getsourcefile(httpx.Response.encoding.fget),
        inspect.getsourcefile(email.message),
        inspect.getsourcefile(email.utils),
    ]
    print(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A06",
                "scope": "actual HTTPX Headers/email selector plus complete actual arXiv adapter outputs",
                "normalization": (
                    "latency_ms omitted; sets sorted; enum values serialized; "
                    "no other output normalization"
                ),
                "library_source_sha256": {
                    str(Path(p).name): hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in paths
                },
                "records": records,
            },
            ensure_ascii=False,
            indent=2,
            allow_nan=False,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
