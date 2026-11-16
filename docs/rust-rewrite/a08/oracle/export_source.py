"""Temporary inventoried migration evidence; no runtime/gate imports. Retire G08."""

import asyncio
import hashlib
import json
import math
import os
import pathlib
import platform
import subprocess
import sys
import unicodedata

import httpx

ROOT = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from engines.edgar import EdgarAdapter  # noqa: E402
from engines.fred import FredAdapter  # noqa: E402
from engines.openfda import OpenFDAAdapter  # noqa: E402
from engines.oyez import OyezAdapter  # noqa: E402

CLASSES = {"edgar": EdgarAdapter, "fred": FredAdapter, "openfda": OpenFDAAdapter, "oyez": OyezAdapter}
CASES = []


def add(id, engine, bodies, config=None, query="test", codes=None):
    cfg = {"api_key": "synthetic-fred-key"} if engine == "fred" else {}
    cfg.update(config or {})
    CASES.append(
        dict(
            id=id,
            engine=engine,
            query=query,
            config=cfg,
            responses=[dict(body=body, status=(codes or [200] * len(bodies))[i]) for i, body in enumerate(bodies)],
        )
    )


def ed(*rows):
    return {"hits": {"hits": list(rows)}}


def hit(source, score=2):
    return {"_source": source, "_score": score}


ED = hit(
    {
        "cik": "0000320193",
        "adsh": "0000320193-24-000001",
        "primary_doc": "a.htm",
        "display_names": ["Apple"],
        "form_type": "10-K",
        "description": "Annual report",
        "filed_at": "2024-03-31T00:00:00",
    }
)
FR = {
    "id": "GDP",
    "title": "Gross domestic product",
    "frequency": "Quarterly",
    "units": "Billions",
    "seasonal_adjustment": "Adjusted",
    "observation_start": "1947-01-01",
    "popularity": 98,
    "notes": "N" * 600,
}
OY = {
    "title": ["Brown v Board"],
    "term": "1953",
    "decision_date": "1954-05-17",
    "search_api_url": "https://api.oyez.org/cases/1953/1",
    "field_case_description": "Equal protection",
}
FD = {
    "set_id": "label-1",
    "openfda": {
        "brand_name": ["TEST"],
        "generic_name": ["test"],
        "manufacturer_name": ["Maker"],
        "substance_name": ["Compound"],
    },
    "purpose": ["Purpose"],
    "indications_and_usage": ["I" * 600],
}
SUCCESS = {
    "edgar": ed(ED),
    "fred": {"seriess": [FR]},
    "oyez": {"hits": {"hits": [{"_source": OY, "_score": 9}]}},
    "openfda": {"results": [FD]},
}
EMPTY = {"edgar": ed(), "fred": {"seriess": []}, "oyez": [], "openfda": {"results": []}}
for engine in CLASSES:
    add(engine + "-success", engine, [SUCCESS[engine]], query=' a & "b" \\ ')
    for kind, body in [("empty", EMPTY[engine]), ("missing", {}), ("outer-list", []), ("outer-null", None)]:
        add(engine + "-" + kind, engine, [body, EMPTY[engine]] if engine == "openfda" else [body])
    for code in [404, 429, 503]:
        add(engine + f"-http-{code}", engine, [{}, {}], codes=[code, code])
    for setting in [0, -1, True, 2.0, "2"]:
        add(engine + "-max-" + repr(setting), engine, [SUCCESS[engine]], config={"max_results": setting})
    for setting in [True, "5000", 0, -1]:
        add(engine + "-timeout-" + repr(setting), engine, [SUCCESS[engine]], config={"timeout_ms": setting})
    c = CASES[-1]
    add(engine + "-malformed-json", engine, [{}])
    CASES[-1]["responses"][0]["raw"] = "{broken"
    add(engine + "-timeout", engine, [{}])
    CASES[-1]["responses"][0]["exception"] = "timeout"

# Every EDGAR URL priority alias and source shape has an actual source outcome.
for field in ["filing_url", "file_url", "url", "link", "href"]:
    for loc in ["source", "hit"]:
        row = hit({"display_names": ["supplied"], "cik": "8", "form_type": "10-Q"})
        target = row["_source"] if loc == "source" else row
        target[field] = "https://www.sec.gov:443/Archives/edgar/data/8/000000000824000001/../a.htm"
        add("edgar-url-" + loc + "-" + field, "edgar", [ed(row)])
for value in [True, 0, "0000", " 0008 ", "٨", ["0008"], [], "12345678901"]:
    add("edgar-cik-" + repr(value), "edgar", [ed(hit({"cik": value, "display_names": ["Company"]}))])
for value in ["0000000008-24-000001", "٠٠٠٠٠٠٠٠٠٨-٢٤-٠٠٠٠٠١", "bad", None]:
    add(
        "edgar-accession-" + repr(value),
        "edgar",
        [ed(hit({"cik": "8", "adsh": value, "display_names": ["Filing"], "form_type": "10-K"}))],
    )
add(
    "edgar-distinct-omissions",
    "edgar",
    [
        ed(
            ED,
            hit({"cik": 8, "adsh": "0000000008-24-000002", "display_names": ["Another"], "form_type": "10-Q"}),
            hit({"cik": 8, "display_names": ["Company"]}),
            hit({"cik": 8, "form_type": "10-K"}),
            [],
            hit(None),
        )
    ],
)
for value in [None, [], "AB", 42, {}, [None], [42]]:
    add(
        "edgar-names-" + repr(value),
        "edgar",
        [
            ed(
                hit(
                    {
                        "cik": 8,
                        "display_names": value,
                        "entity_name": "Fallback",
                        "form_type": "10-K",
                        "adsh": "0000000008-24-000001",
                    }
                )
            )
        ],
    )
for value in [None, {}, "", 1]:
    add("edgar-hits-" + repr(value), "edgar", [{"hits": {"hits": value}}])
for value in [
    "https://evil.test/Archives/edgar/data/8/000000000824000001/a.htm",
    "https://www.sec.gov:444/Archives/edgar/data/8/000000000824000001/a.htm",
    "https://user@www.sec.gov/Archives/edgar/data/8/000000000824000001/a.htm",
    "https://www.sec.gov/Archives/edgar/data/8/000000000824000001/a.htm?x=1",
]:
    add("edgar-unsafe-" + value, "edgar", [ed(hit({"url": value, "display_names": ["x"], "form_type": "10-Q"}))])
# FRED auth/preference, full payload, wrong shape and numeric conversion.
for value in [None, "", "   "]:
    add("fred-key-" + repr(value), "fred", [SUCCESS["fred"]], config={"api_key": value})
for value in [None, "", [], {}, "9.5", True, "NaN", "Infinity", "bad"]:
    row = dict(FR, popularity=value)
    add("fred-popularity-" + repr(value), "fred", [{"seriess": [row]}])
for value in [None, {}, "", 1]:
    add("fred-series-" + repr(value), "fred", [{"seriess": value}])
add("fred-falsey-payload", "fred", [{"seriess": [{"id": 0, "title": False, "units": [], "notes": ""}]}])
# Oyez route, strict identity, source score order and omission-before-bound.
add("oyez-rest", "oyez", [[OY]], config={"base_url": "https://local.test/v1///"})
add(
    "oyez-search-override",
    "oyez",
    [[OY]],
    config={"base_url": "https://ignored.test", "search_url": "https://local.test/search"},
)
for value in [
    "/cases/1953/1",
    "/cases/../..",
    "https://user:pw@API.OYEZ.ORG:444/cases/19%35%33/1?x=1#f",
    "/cases/1953/a%2Fb",
    "https://evil.test/cases/1953/1",
    "/cases/1953",
    "/cases//1",
    "cases/1953/1",
]:
    add("oyez-url-" + value, "oyez", [[dict(OY, search_api_url=value)]])
add(
    "oyez-order-omit",
    "oyez",
    [
        {
            "hits": {
                "hits": [
                    {"_source": dict(OY, search_api_url="bad"), "_score": 99},
                    {"_source": OY, "_score": 1},
                    {"_source": dict(OY, search_api_url="/cases/1953/2"), "_score": 50},
                ]
            }
        }
    ],
    config={"max_results": 2},
)
for value in [None, "bad", True, "9.5", "NaN", [], {}]:
    add("oyez-score-" + repr(value), "oyez", [{"hits": {"hits": [{"_source": OY, "_score": value}]}}])
# FDA exact/fallback, identities/ranking and query Unicode transforms.
add("openfda-blank", "openfda", [], query=" \u2003 ")
add("openfda-fallback", "openfda", [{"results": [1, None]}, SUCCESS["openfda"]])
add("openfda-no-id-suppresses-fallback", "openfda", [{"results": [{}]}, SUCCESS["openfda"]])
for stage in [0, 1]:
    add(
        "openfda-stage-failure-" + str(stage),
        "openfda",
        [EMPTY["openfda"], {}],
        codes=[503 if stage == 0 else 200, 503],
    )
for value in [None, {}, "", 1]:
    add("openfda-results-" + repr(value), "openfda", [{"results": value}])
for query in ["Advil", "ßtraße", "ǳELTA test", "istanbul İ ΣΟΣ", 'a"b\\c', "they're BILL'S", "\u0149ﬃ"]:
    add("openfda-query-" + query, "openfda", [SUCCESS["openfda"]], query=query)
for field in ["set_id", "id", "spl_id", "spl_set_id"]:
    for nested in [False, True]:
        row = {"openfda": {"brand_name": ["Test"]}}
        target = row["openfda"] if nested else row
        target[field] = 'a / b"\\'
        add("openfda-id-" + str(nested) + field, "openfda", [{"results": [row]}])
add(
    "openfda-rank-dedup",
    "openfda",
    [
        {
            "results": [
                dict(FD, set_id="other", openfda={"brand_name": ["Other"]}),
                dict(FD, set_id="partial", openfda={"generic_name": ["test long"]}),
                FD,
                FD,
                {},
                dict(FD, set_id="tokens", openfda={"brand_name": ["one test two"]}),
            ]
        }
    ],
)


# Additional exact shape/identity/refusal/redaction boundary controls.
for value in [None, {}, [], True, "7", float("nan"), float("inf")]:
    add("edgar-score-" + repr(value), "edgar", [ed(dict(ED, _score=value))])
    if isinstance(value, float) and not math.isfinite(value):
        CASES[-1]["responses"][0]["raw"] = json.dumps(CASES[-1]["responses"][0]["body"])
for field in ["primary_doc", "primaryDocument", "primary_document", "file_name"]:
    add(
        "edgar-document-" + field,
        "edgar",
        [ed(hit({"cik": "8", "adsh": "0000000008-24-000001", field: "x_1.htm", "display_names": ["doc"]}))],
    )
for value in [
    "https://www.sec.gov:/Archives/edgar/data/8/000000000824000001/a.htm",
    "https://www.sec.gov:00443/Archives/edgar/data/8/000000000824000001/a.htm",
    "https://www.sec.gov/Archives/edgar/data/8/000000000824000001/a.htm?#",
    "https://www.sec.gov/Archives/edgar/data/8/000000000824000001/a.htm;params",
    "https://www.%73ec.gov/Archives/edgar/data/8/000000000824000001/a.htm",
]:
    add("edgar-url-boundary-" + value, "edgar", [ed(hit({"url": value, "display_names": ["URL"]}))])
for value in [None, 1, [], {}, "not a date", "2024-xx-xx-long"]:
    add("edgar-date-" + repr(value), "edgar", [ed(hit({**ED["_source"], "filed_at": value}))])
for value in [None, 1, [], {}, " "]:
    add("edgar-outer-hits-" + repr(value), "edgar", [{"hits": value}])
for value in [None, True, 42, [], {}]:
    add("fred-direct-key-shape-" + repr(value), "fred", [SUCCESS["fred"]], config={"api_key": value})
for value in [None, True, 42, [], {}, "A\ud800B"]:
    add("fred-title-" + repr(value), "fred", [{"seriess": [dict(FR, title=value)]}])
for value in ["X" * 220 + "\ud800", "\ud800bad"]:
    add("fred-notes-surrogate-" + repr(value), "fred", [{"seriess": [dict(FR, notes=value)]}])
for value in [
    "https://www.oyez.org:bad/cases/1953/1",
    "https://www.%6fyez.org/cases/1953/1",
    "/cases/1953/a%é",
    "/cases/1953/%FF",
    "/cases/1953/1;params",
]:
    add("oyez-url-boundary-" + value, "oyez", [[dict(OY, search_api_url=value)]])
for value in ["title\ud800", [None, "title\ud800"]]:
    add("oyez-name-surrogate-" + repr(value), "oyez", [[dict(OY, title=value)]])
for value in [[], {}, None, True, 42, "wrong"]:
    add("oyez-results-shape-" + repr(value), "oyez", [{"results": value}])
for value in [None, [], {}, True, 42, "bad"]:
    add("oyez-hits-shape-" + repr(value), "oyez", [{"hits": {"hits": value}}])
for value in [None, [], {}, True, 42, "bad"]:
    add("openfda-harmonized-" + repr(value), "openfda", [{"results": [dict(FD, openfda=value)]}])
for value in ["I" * 200 + "\ud800", "\ud800bad"]:
    add(
        "openfda-indications-surrogate-" + repr(value),
        "openfda",
        [{"results": [dict(FD, indications_and_usage=[value])]}],
    )
for value in ["id\ud800", ["id\ud800"]]:
    add("openfda-id-surrogate-" + repr(value), "openfda", [{"results": [dict(FD, set_id=value)]}])
for value in ["brand\ud800", ["brand\ud800"]]:
    add("openfda-name-surrogate-" + repr(value), "openfda", [{"results": [dict(FD, openfda={"brand_name": value})]}])
for diagnostic in [
    "https://fred.test/search?api_key=synthetic-fred-key&x=1",
    "failed https://fred.test/search?api_key=synthetic-fred-key&api_key=synthetic-fred-key&x=1 surround",
    "https://fred.test/search?%61pi_key=synthetic-fred-key&token=synthetic-fred-key&x=1",
    "https://[broken?api_key=synthetic-fred-key",
]:
    add("fred-redaction-" + str(len(CASES)), "fred", [{}])
    CASES[-1]["responses"][0].update(exception="runtime", message=diagnostic)


# Whole-adapter URL parsing controls. EDGAR runs both unresolved and valid
# identity fallback paths: accepted supplied bytes are retained verbatim.
archive_base = "https://www.sec.gov/Archives/edgar/data/123/000000000000000001/"
archive_urls = [
    archive_base + suffix
    for suffix in [
        "doc.htm;params/extra",
        "doc.htm;params/../extra",
        "doc.htm;a;b",
        "doc.htm;params",
        "doc.htm?",
        "doc.htm#",
        "doc.htm?#",
        "doc.htm?x",
        "doc.htm#x",
    ]
] + [
    "\x00" + archive_base + "doc.htm",
    archive_base + "do\tc.htm",
    archive_base.replace("www.sec.gov", "www.sec.\ngov") + "doc.htm",
    archive_base.replace("www.sec.gov", "www.sec.gov:+443") + "doc.htm",
    archive_base.replace("www.sec.gov", "www.sec.gov:443") + "doc.htm",
    archive_base.replace("www.sec.gov", "www.sec.gov:00443") + "doc.htm",
    archive_base.replace("www.sec.gov", "www.sec.gov:") + "doc.htm",
    archive_base.replace("www.sec.gov", "user@www.sec.gov") + "doc.htm",
    archive_base.replace("www.sec.gov", "www.sec.gov:65536") + "doc.htm",
    archive_base.replace("www.sec.gov", "[broken") + "doc.htm",
]
for index, url in enumerate(archive_urls):
    for fallback in [False, True]:
        row = {"url": url, "display_names": ["URL boundary"], "form_type": "8-K"}
        if fallback:
            row.update(cik="123", adsh="0000000000-00-000001", primary_doc="fallback.htm")
        add(f"edgar-parse-{index:02}-{'fallback' if fallback else 'unresolved'}", "edgar", [ed(hit(row))])
for index, url in enumerate(
    [
        "/cases/2024/123;a;b",
        "/cases/2024;extra/123",
        "/cases/2024/123;extra/tail",
        "/cases/2024/123?",
        "/cases/2024/123#",
        "/cases/2024/123?x#y",
        "///cases/2024/123",
        "////cases/2024/123",
        "//cases/2024/123",
        "\x00https://www.oyez.org/cases/2024/123",
        "https://www.oyez.org/cases/2024/12\t3",
        "https://www.oyez.org:bad/cases/2024/123",
        "https://user@www.oyez.org:00443/cases/2024/123",
        "https://www.oyez.org]/cases/2024/123",
        "https://[broken/cases/2024/123",
    ]
):
    add(f"oyez-parse-{index:02}", "oyez", [[dict(OY, search_api_url=url)]])
# Balanced-bracket/address precedence and every accepted Unicode16 character
# whose NFKC expansion introduces an ASCII URL delimiter. These invoke the
# actual adapter after response decoding; request body/dispatch remain captured.
netlocs = [
    "www.oyez.org:[x]",
    "www.oyez.org:port",
    "[www.oyez.org]",
    "[127.0.0.1]",
    "[vX.address]",
    "[v1.address]",
    "[V1.address]",
    "[::1]",
    "[::1]suffix",
    "prefix[::1]",
    "user@[::1]:port",
    "[fe80::1%zone]",
    "[fe80::1%]",
    "[fe80::1%zone%other]",
    "[has'quote]",
    '[has"quote]',
    "[has\\slash]",
] + [
    "www.oyez.org:" + chr(code)
    for code in json.loads((ROOT / "contracts/unicode/python-text-16.0.0.json").read_text())["nfkc_url_delimiters"]
]
for index, netloc in enumerate(netlocs):
    add(f"oyez-netloc-{index:02}", "oyez", [[dict(OY, search_api_url=f"https://{netloc}/cases/2024/123")]])
for index, url in enumerate(["//www.oyez.org:[x]/cases/2024/123", "1://www.oyez.org:[x]/cases/2024/123"]):
    add(f"oyez-relative-netloc-{index}", "oyez", [[dict(OY, search_api_url=url)]])
for index, value in enumerate([float("nan"), float("inf"), -float("inf"), "NaN"]):
    add(f"oyez-post-number-{index}", "oyez", [EMPTY["oyez"]], config={"max_results": value})


def safe(value):
    if isinstance(value, str) and any(0xD800 <= ord(c) <= 0xDFFF for c in value):
        return {"$source_text": [ord(c) for c in value]}
    if isinstance(value, float) and not math.isfinite(value):
        return {"$nonfinite": repr(value)}
    if isinstance(value, list):
        return [safe(v) for v in value]
    if isinstance(value, dict):
        return {k: safe(v) for k, v in value.items()}
    return value


async def main():
    assert sys.version_info[:3] == (3, 14, 7) and unicodedata.unidata_version == "16.0.0"
    os.environ.pop("ENGINE_FRED_API_KEY", None)
    out = {
        "schema_version": 1,
        "plan_id": "A08",
        "baseline": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "provenance": (
            "Actual offline Python source adapters under HTTPX MockTransport; "
            "synthetic credentials and inline source controls; no network/provider calls."
        ),
        "toolchain": {"python": sys.version, "httpx": httpx.__version__, "platform": platform.platform()},
        "source_inventory": [],
        "records": [],
    }
    for path in [
        *("engines/" + n + ".py" for n in CLASSES),
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "slopsearx/config.py",
    ]:
        b = (ROOT / path).read_bytes()
        out["source_inventory"].append(
            {
                "path": path,
                "sha256": hashlib.sha256(b).hexdigest(),
                "git_blob": subprocess.check_output(["git", "hash-object", path], cwd=ROOT, text=True).strip(),
            }
        )
    for case in CASES:
        calls = []
        responses = iter(case["responses"])

        def handle(req):
            # Credential values are never retained in source requests or errors.
            url = str(req.url).replace("synthetic-fred-key", "<synthetic-key>")
            calls.append(
                {
                    "method": req.method,
                    "url": url,
                    "body": req.content.decode(),
                    "headers": {k: v for k, v in req.headers.items() if k in ["user-agent", "accept", "content-type"]},
                }
            )
            reply = next(responses)
            if reply.get("exception") == "timeout":
                raise httpx.ReadTimeout("synthetic timeout", request=req)
            if reply.get("exception") == "runtime":
                raise RuntimeError(reply["message"])
            response = (
                httpx.Response(reply["status"], content=reply["raw"])
                if "raw" in reply
                else httpx.Response(
                    reply["status"],
                    content=json.dumps(
                        reply["body"], ensure_ascii=True, separators=(",", ":"), allow_nan=False
                    ).encode(),
                )
            )
            reply["wire_body"] = response.content.decode()
            return response

        adapter = CLASSES[case["engine"]](case["config"])
        adapter.set_http_transport(httpx.MockTransport(handle))
        try:
            result = await adapter.search(case["query"], {"page": 8, "date_from": "2024-01-01"})
            rows = [
                {
                    k: getattr(r, k)
                    for k in [
                        "url",
                        "title",
                        "content",
                        "engine",
                        "position",
                        "score",
                        "category",
                        "published_date",
                        "thumbnail",
                        "img_src",
                        "media",
                        "payload",
                    ]
                }
                for r in result.results
            ]
            output = {"status": result.status.value, "error_message": result.error_message, "results": rows}
        except Exception as exc:
            output = {"raised": type(exc).__name__, "error_message": str(exc)}
        await adapter.shutdown()
        if output.get("error_message"):
            output["error_message"] = output["error_message"].replace("synthetic-fred-key", "<synthetic-key>")
        out["records"].append({**case, "requests": calls, "output": safe(output)})
    output = ROOT / "docs/rust-rewrite/a08/source.json"
    output.write_text(json.dumps(safe(out), ensure_ascii=True, separators=(",", ":"), allow_nan=False) + "\n")
    print(len(CASES), hashlib.sha256(output.read_bytes()).hexdigest())


asyncio.run(main())
