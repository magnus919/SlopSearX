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


add("ct-success", "clinicaltrials", [fixture("clinicaltrials-success")], query="heart disease")
add("pc-success", "pubchem", [fixture("pubchem-search"), fixture("pubchem-detail")], query="aspirin/a b")
add("pm-success", "pubmed", [fixture("pubmed-search"), fixture("pubmed-summary")], query="covid vaccine")
add("epmc-med", "europepmc", [fixture("europepmc-biomedical")])
add("epmc-ppr", "europepmc", [fixture("europepmc-preprints")])
add("up-success", "uniprot", [fixture("uniprot-success")], query="p53")
empty = {
    "clinicaltrials": {"studies": []},
    "pubchem": {"IdentifierList": {"CID": []}},
    "pubmed": {"esearchresult": {"idlist": []}},
    "europepmc": {"resultList": {"result": []}},
    "uniprot": {"results": []},
}
for engine in classes:
    add(engine + "-empty", engine, [empty[engine]])
    add(engine + "-wrong-outer", engine, [[]])
    add(engine + "-missing", engine, [{}])
    for code in [404, 429, 503]:
        add(engine + f"-http-{code}", engine, [{}])
        cases[-1]["responses"][0]["status"] = code
    for key, value in [
        ("max_results", 0),
        ("max_results", -1),
        ("max_results", True),
        ("max_results", 2.0),
        ("max_results", "2"),
        ("timeout_ms", True),
        ("timeout_ms", "5000"),
    ]:
        add(engine + f"-config-{key}-{type(value).__name__}-{str(value)}", engine, [empty[engine]], config={key: value})
proteins = [
    {
        "primaryAccession": a,
        "uniProtkbId": a + "_TEST",
        "proteinDescription": {"recommendedName": {"fullName": {"value": n}}},
        "genes": [{"geneName": {"value": g}}],
        "organism": {"scientificName": o},
    }
    for a, g, o, n in [
        ("Q99999", "VIMBP", "Homo sapiens", "Vimentin-binding protein"),
        ("P20152", "Vim", "Mus musculus", "Vimentin"),
        ("P08670", "VIM", "Homo sapiens", "Vimentin"),
    ]
]
for query in [
    "VIM",
    "P08670",
    "gene:VIM",
    "id:P08670_TEST",
    "accession:p08670",
    " (gene:VIM) AND organism_id:9606 ",
    "v",
    " VIM ",
    "K12345",
    "gene:VİM",
]:
    add("up-query-" + query, "uniprot", [{"results": proteins}], query=query, config={"max_results": 2})
for ids in [[], [1], [""], ["1", "2"]]:
    add("pm-ids-" + repr(ids), "pubmed", [{"esearchresult": {"idlist": ids}}, {"result": {"2": {"title": "Two"}}}])
add(
    "pm-authors",
    "pubmed",
    [
        {"esearchresult": {"idlist": ["1"]}},
        {
            "result": {
                "1": {
                    "title": "T",
                    "authors": [{"name": v} for v in ["A", "B", "C", "D", "E"]],
                    "articleids": [{"idtype": "doi", "value": "10/x"}],
                }
            }
        },
    ],
)
for date in ["2026-99-99", "20260102", "2026-W01-1", "2026-W01", "0000-01-01"]:
    add(
        "epmc-date-" + date,
        "europepmc",
        [
            {
                "resultList": {
                    "result": [
                        {
                            "source": "MED",
                            "id": "1",
                            "title": "t",
                            "firstPublicationDate": date,
                            "journalTitle": [],
                            "doi": "10/x",
                        }
                    ]
                }
            }
        ],
    )
cases = []
for capture in ["source_matrix.json", "source_supplement.json", "source_numeric.json", "source_digits.json"]:
    for record in json.loads((root / "docs/rust-rewrite/a07" / capture).read_text())["records"]:
        cases.append({k: record[k] for k in ["id", "engine", "query", "config", "responses"]})


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
    p = pathlib.Path("/private/tmp/a07-source-complete.json")
    p.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n")
    print(len(cases), hashlib.sha256(p.read_bytes()).hexdigest())


asyncio.run(main())
