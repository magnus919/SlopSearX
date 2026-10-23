"""Temporary offline A06 large-input oracle: five full outcomes, three summaries."""

import asyncio
import base64
import hashlib
import json
import pyexpat
import runpy
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))
ATOM = "http://www.w3.org/2005/Atom"


def cases():
    yield "deep_100000", ("<x>" * 100000 + "</x>" * 100000).encode(), {"kind": "deep", "count": 100000}
    yield (
        "siblings_1000001",
        (f'<x xmlns="{ATOM}">' + "<i/>" * 1000001 + "<entry><title>T</title></entry></x>").encode(),
        {"kind": "siblings", "count": 1000001},
    )
    decl = "<!ENTITY e0 'X'>" + "".join(f"<!ENTITY e{i} '&e{i - 1};'>" for i in range(1, 10001))
    yield (
        "entity_chain_10000",
        f'<!DOCTYPE x [{decl}]><x xmlns="{ATOM}"><entry><title>&e10000;</title></entry></x>'.encode(),
        {"kind": "entity_chain", "count": 10000},
    )
    prefix = f'<x xmlns="{ATOM}"><ignored>'.encode()
    suffix = b"</ignored><entry><title>T</title></entry></x>"
    for name, fill in [("ascii", b"p"), ("utf8_replacement", b"\xff")]:
        n = 32 * 1024 * 1024 - len(prefix) - len(suffix)
        yield (
            "transport_max_" + name,
            prefix + fill * n + suffix,
            {"kind": name, "padding": n, "body_bytes": 32 * 1024 * 1024},
        )
    decl = "<!ENTITY e0 '" + "A" * 1024 + "'>" + "".join(f"<!ENTITY e{i} '&e{i - 1};&e{i - 1};'>" for i in range(1, 17))
    body = (
        f"<!DOCTYPE x [{decl}]><!--"
        + "p" * (1024 * 1024)
        + f'--><x xmlns="{ATOM}"><entry><title>&e16;</title></entry></x>'
    ).encode()
    for late in [False, True]:
        yield (
            "retained64m_" + str(late),
            body + (b"<bad>" if late else b""),
            {"base": "A", "base_count": 1024, "doublings": 16, "comment_padding": 1024 * 1024, "late_invalid": late},
        )

    summary = "A" * (5 * 1024 * 1024)
    yield (
        "large_abstract5m",
        f'<x xmlns="{ATOM}"><entry><title>T</title><summary>{summary}</summary></entry></x>'.encode(),
        {"kind": "large_abstract", "summary_codepoints": len(summary)},
    )


async def main():
    records = []
    for name, body, generator in cases():
        case = {
            "id": "arxiv.stress." + name,
            "engine": "arxiv",
            "query": "q",
            "responses": [{"status": 200, "headers": {}, "body_b64": base64.b64encode(body).decode()}],
        }
        record = await BASE["capture"](case)
        row = {"id": case["id"], "generator": generator, "body_sha256": hashlib.sha256(body).hexdigest()}
        if name == "large_abstract5m":
            response = record["output"]
            abstract = response["results"][0]["payload"]["data"].pop("abstract")
            row["abstract_codepoints"] = len(abstract)
            row["abstract_sha256"] = hashlib.sha256(abstract.encode()).hexdigest()
            row["output_without_abstract"] = response
        elif name.startswith("retained64m_"):
            response = record["output"]
            results = response.pop("results")
            response["results_summary"] = [
                {
                    "title_codepoints": len(r["title"]),
                    "title_sha256": hashlib.sha256(r["title"].encode()).hexdigest(),
                    "other_fields": {k: v for k, v in r.items() if k != "title"},
                }
                for r in results
            ]
            row["body_bytes"] = len(body)
            row["source_output_summary"] = response
        else:
            row.update({key: record[key] for key in ["requests", "client_options", "output"]})
        records.append(row)
    out = {
        "source_sha256": hashlib.sha256((BASE["ROOT"] / "engines/arxiv.py").read_bytes()).hexdigest(),
        "runtime_expat": pyexpat.EXPAT_VERSION,
        "scope": (
            "5 complete whole-source outputs and 3 explicit giant-field source outcome summaries; "
            "input generators and hashes retained"
        ),
        "records": records,
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
