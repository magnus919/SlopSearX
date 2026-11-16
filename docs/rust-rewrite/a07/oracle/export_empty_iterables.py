import asyncio
import hashlib
import json
import pathlib
import platform
import subprocess
import sys

import httpx

root = pathlib.Path("/Users/magnus/.codex/worktrees/rust-a04-broad-api-adapters/SlopSearX")
sys.path.insert(0, str(root))
from engines.clinicaltrials import ClinicalTrialsAdapter  # noqa: E402
from engines.europepmc import EuropePMCAdapter  # noqa: E402
from engines.pubchem import PubChemAdapter  # noqa: E402
from engines.pubmed import PubMedAdapter  # noqa: E402
from engines.uniprot import UniProtAdapter  # noqa: E402

classes = {
    "clinicaltrials": ClinicalTrialsAdapter,
    "europepmc": EuropePMCAdapter,
    "pubchem": PubChemAdapter,
    "pubmed": PubMedAdapter,
    "uniprot": UniProtAdapter,
}
f = root / "docs/rust-rewrite/a07/fixtures"


def fixture(name):
    return json.loads((f / (name + ".json")).read_text())


def response(body, code=200):
    return {"status": code, "body": body}


cases = []


def add(id, engine, bodies, query="test", config=None):
    cases.append(
        {"id": id, "engine": engine, "query": query, "config": config or {}, "responses": [response(b) for b in bodies]}
    )


cases = []
for label, value in [("string", ""), ("map", {})]:
    for fieldname in ["studies", "phases", "conditions"]:
        body = fixture("clinicaltrials-success")
        if fieldname == "studies":
            body["studies"] = value
        elif fieldname == "phases":
            body["studies"][0]["protocolSection"]["designModule"]["phases"] = value
        else:
            body["studies"][0]["protocolSection"]["conditionsModule"]["conditions"] = value
        add("empty-iterable-clinicaltrials-" + fieldname + "-" + label, "clinicaltrials", [body])
    for fieldname in ["CID", "PC_Compounds", "props"]:
        first = fixture("pubchem-search")
        second = fixture("pubchem-detail")
        if fieldname == "CID":
            first["IdentifierList"]["CID"] = value
        elif fieldname == "PC_Compounds":
            second["PC_Compounds"] = value
        else:
            second["PC_Compounds"][0]["props"] = value
        add("empty-iterable-pubchem-" + fieldname + "-" + label, "pubchem", [first, second])
    for fieldname in ["idlist", "authors", "articleids", "pubtype"]:
        first = {"esearchresult": {"idlist": ["1"]}}
        second = {"result": {"1": {"title": "One"}}}
        if fieldname == "idlist":
            first["esearchresult"]["idlist"] = value
        else:
            second["result"]["1"][fieldname] = value
        add("empty-iterable-pubmed-" + fieldname + "-" + label, "pubmed", [first, second])
    add("empty-iterable-europepmc-result-" + label, "europepmc", [{"resultList": {"result": value}}])
    for fieldname in ["results", "genes", "synonyms"]:
        protein = {"primaryAccession": "P08670", "uniProtkbId": "VIM_HUMAN"}
        if fieldname == "genes":
            protein["genes"] = value
        elif fieldname == "synonyms":
            protein["genes"] = [{"synonyms": value}]
        body = {"results": value if fieldname == "results" else [protein]}
        add("empty-iterable-uniprot-" + fieldname + "-" + label, "uniprot", [body], query="VIM")


async def main():
    out = {
        "baseline": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "provenance": (
            "actual offline Python adapters with HTTPX MockTransport; "
            "synthetic regression/control cases; no network/provider calls"
        ),
        "toolchain": {"python": sys.version, "httpx": httpx.__version__, "platform": platform.platform()},
        "source_inventory": [],
        "records": [],
    }
    for path in [
        *(f"engines/{n}.py" for n in classes),
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "slopsearx/config.py",
    ]:
        b = (root / path).read_bytes()
        out["source_inventory"].append(
            {
                "path": path,
                "sha256": hashlib.sha256(b).hexdigest(),
                "git_blob": subprocess.check_output(["git", "rev-parse", "HEAD:" + path], cwd=root, text=True).strip(),
            }
        )
    for case in cases:
        calls = []
        responses = iter(case["responses"])

        def handle(request):
            calls.append({"method": request.method, "url": str(request.url), "headers": dict(request.headers)})
            reply = next(responses)
            return httpx.Response(reply["status"], json=reply["body"])

        adapter = classes[case["engine"]](case["config"])
        adapter.set_http_transport(httpx.MockTransport(handle))
        try:
            result = await adapter.search(case["query"], {"page": 8, "date_from": "2024-01-01"})
        except Exception as exc:
            out["records"].append(
                {**case, "requests": calls, "output": {"raised": type(exc).__name__, "error_message": str(exc)}}
            )
            await adapter.shutdown()
            continue
        await adapter.shutdown()
        values = []
        for r in result.results:
            values.append(
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
                        "tier",
                        "work_group",
                    ]
                }
            )
            values[-1]["engines"] = sorted(r.engines)
        out["records"].append(
            {
                **case,
                "requests": calls,
                "output": {
                    "status": result.status.value,
                    "error_message": result.error_message,
                    "results": values,
                    "answers": result.answers,
                    "corrections": result.corrections,
                    "infoboxes": result.infoboxes,
                    "synthetic": result.synthetic,
                },
            }
        )
    p = pathlib.Path("/private/tmp/a07-source-empty-iterables.json")
    p.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n")
    print(len(cases), hashlib.sha256(p.read_bytes()).hexdigest())


asyncio.run(main())
