"""Temporary G08-retired actual arXiv/HTTPX Unicode boundary source oracle."""

import asyncio
import base64
import hashlib
import inspect
import json
import runpy
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import httpx

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))


async def main():
    records = []
    for number, body in enumerate(
        [
            b"\\ud800",
            b"a\\ud800\\udfff",
            b"\\ud800a\\udfff",
            b"\\udfff",
            b"<feed>\\ud800</feed>",
            b"\\U0010ffff",
            b"",
            b"<notxml",
        ]
    ):
        headers = {"content-type": "text/plain; charset=unicode_escape"}
        case = {
            "id": f"arxiv.decode.{number:02}",
            "engine": "arxiv",
            "query": "test",
            "responses": [{"status": 200, "body_b64": base64.b64encode(body).decode(), "headers": headers}],
        }
        row = await BASE["capture"](case)
        try:
            decoded = httpx.Response(200, headers=headers, content=body).text
            row["decoded_codepoints"] = [ord(c) for c in decoded]
            try:
                ET.fromstring(decoded)
            except Exception as error:
                row["xml_error"] = {"type": type(error).__name__, "message": str(error)}
        except Exception as error:
            row["body_error"] = {"type": type(error).__name__, "message": str(error)}
        records.append(row)
    paths = [
        BASE["ROOT"] / "engines/arxiv.py",
        Path(inspect.getsourcefile(httpx.Response.text.fget)),
        Path(ET.__file__),
    ]
    print(
        json.dumps(
            {
                "schema_version": 1,
                "purpose": "temporary offline source oracle; retire G08",
                "python": sys.version,
                "oracle_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "source_sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},
                "records": records,
            },
            ensure_ascii=True,
            indent=2,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
