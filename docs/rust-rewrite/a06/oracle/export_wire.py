"""Temporary offline A06 request/Unicode predicate observations; no provider calls."""

import asyncio
import hashlib
import json
import re
import runpy
import unicodedata
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))


async def main():
    queries = [
        "",
        " graph neural ",
        "graph\tneural",
        "graph\u001cneural",
        "graph\u00a0neural",
        "graph\u0085neural",
        "graph\u200bneural",
        "graph and neural",
        "graph or neural",
        "graph andnot neural",
        "C++ & climate",
        "a/b c",
        "a*b ~c",
        "a?b #c",
        "café biology",
        "αβ γδ",
        "a\u0301 b",
        "\u0345 b",
        "a_b c-d",
        "² Ⅳ",
        "\U0001e4d0 b",
    ]
    records = []
    for engine in ["arxiv", "openalex", "semanticscholar"]:
        body = "<x/>" if engine == "arxiv" else ({"results": []} if engine == "openalex" else {"data": []})
        for index, query in enumerate(queries):
            case = {
                "id": f"{engine}.wire.query_{index}",
                "engine": engine,
                "query": query,
                "responses": [BASE["reply"](body)],
            }
            records.append(await BASE["capture"](case))
    # Actual HTTPX UTF7 decoding preserves isolated surrogates; ElementTree then
    # raises UnicodeEncodeError, which the adapter classifies as whole ERROR.
    import base64

    for name, text in [("high", "<x>\ud800</x>"), ("low", "<x>\udc00</x>"), ("syntax", "<x>"), ("valid", "<x/>")]:
        case = {
            "id": "arxiv.wire.utf7_" + name,
            "engine": "arxiv",
            "query": "test",
            "responses": [
                {
                    "status": 200,
                    "body_b64": base64.b64encode(text.encode("utf-7")).decode(),
                    "headers": {"Content-Type": "application/atom+xml; charset=utf-7"},
                }
            ],
        }
        records.append(await BASE["capture"](case))
    ranges = []
    start = previous = None
    for point in range(0x110000):
        if re.fullmatch(r"\w", chr(point)) is not None:
            if start is None:
                start = point
            previous = point
        elif start is not None:
            ranges.append([start, previous])
            start = previous = None
    if start is not None:
        ranges.append([start, previous])
    print(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A06",
                "unicode_version": unicodedata.unidata_version,
                "word_predicate": {
                    "source": "actual Python re.fullmatch(r'\\w', chr(point)) for every Unicode codepoint",
                    "ranges": ranges,
                },
                "source_sha256": {
                    p: hashlib.sha256((BASE["ROOT"] / p).read_bytes()).hexdigest()
                    for p in ["engines/arxiv.py", "engines/openalex.py", "engines/semanticscholar.py"]
                },
                "normalization": (
                    "latency_ms omitted; sets sorted; enum values serialized; no other output normalization"
                ),
                "records": records,
            },
            ensure_ascii=False,
            indent=2,
            allow_nan=False,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
