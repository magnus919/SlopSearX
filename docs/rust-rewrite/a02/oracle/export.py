# Temporary offline migration oracle only. G08 retirement inventory: ../coverage.json.
# Invoke explicitly with an output path; never required by native tests/gates.
import asyncio
import hashlib
import json
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(root))
import httpx  # noqa: E402

import engines  # noqa: E402, F401 - populate the source registry before capture
from slopsearx.adapter import discover_engines  # noqa: E402

cases = []


def add(id, engine, body=None, query="python", max_results=10, exchanges=None, **kw):
    cases.append(
        dict(
            id=id,
            engine=engine,
            query=query,
            max_results=max_results,
            exchanges=exchanges or [dict(status=200, body=body if isinstance(body, str) else json.dumps(body))],
            **kw,
        )
    )


py = {"info": {"name": "requests", "version": "2.34.2", "summary": "Python HTTP for Humans."}}
add("pypi_exact", "pypi", py, query="requests")
for label, info in [
    ("missing", {}),
    ("null", None),
    ("wrong", []),
    ("empty_summary", {"name": "x", "summary": ""}),
    ("nonmapping_urls", {"project_urls": ["url"]}),
    ("ordered_urls", {"project_urls": {"Homepage": "a", "Source": "first", "Repository": "second"}}),
    (
        "payload_full",
        {
            "name": "requests",
            "version": "2.31.0",
            "summary": "Python HTTP for Humans.",
            "license": "Apache-2.0",
            "home_page": "https://requests.readthedocs.io",
        },
    ),
    ("payload_absent", {"name": "requests", "version": "2.31.0", "summary": "HTTP for Humans."}),
    ("payload_null", {"name": None, "version": None, "summary": None}),
    ("payload_numbers", {"name": 42, "version": False, "license": 123}),
    ("long_summary", {"summary": "😀" * 501}),
]:
    add("pypi_" + label, "pypi", {"info": info})
for name, body in [("root_array", []), ("invalid_json", "{"), ("root_null", None)]:
    add("pypi_" + name, "pypi", body)
for status in [404, 429, 500]:
    add(
        f"pypi_fallback_{status}",
        "pypi",
        exchanges=[
            {"status": status, "body": ""},
            {"status": 200, "body": "<html><a href='/simple/flask/'>Ignored text</a></html>"},
            {"status": 404, "body": ""},
        ],
        query="flask",
    )
add(
    "pypi_fallback_html_order",
    "pypi",
    query="Fl-As_K",
    max_results=3,
    exchanges=[
        {"status": 404, "body": ""},
        {
            "status": 200,
            "body": (
                "<html><a href='/simple/flask-A/'>other</a><a href='/simple/no/'>flask<"
                "/a><a href='/simple/flask_A/'>A</a><a href='/simple/flask-A/'>duplicat"
                "e</a><a href='/simple/flask-z/'>outside cap</a></html>"
            ),
        },
        {"status": 200, "body": '{"info":{"name":"payload-name","version":"1","summary":""}}'},
        {"status": 500, "body": ""},
        {"status": 200, "body": '{"info":{"version":"2"}}'},
    ],
)
for label, body in [
    ("empty_html", ""),
    ("no_matches", '<html><a href="/simple/no/">python</a></html>'),
    ("broken_html", '<a href="/simple/python/">x'),
    ("missing_href", "<a>x</a>"),
]:
    add(
        "pypi_" + label,
        "pypi",
        exchanges=[{"status": 404, "body": ""}, {"status": 200, "body": body}, {"status": 200, "body": '{"info":{}}'}]
        if label == "broken_html"
        else [{"status": 404, "body": ""}, {"status": 200, "body": body}],
    )
for phase in ["direct", "simple", "detail"]:
    ex = [
        {"status": 404, "body": ""},
        {"status": 200, "body": '<a href="/simple/python/">x</a>'},
        {"status": 200, "body": '{"info":{}}'},
    ]
    i = {"direct": 0, "simple": 1, "detail": 2}[phase]
    ex = ex[:i] + [{"timeout": True}]
    add("pypi_timeout_" + phase, "pypi", exchanges=ex)
for phase in ["simple", "detail"]:
    i = {"simple": 1, "detail": 2}[phase]
    for status in [302, 429, 500]:
        ex = [{"status": 404, "body": ""}, {"status": 200, "body": '<a href="/simple/python/">x</a>'}]
        ex = ex[:i] + [{"status": status, "body": ""}]
        add(f"pypi_{phase}_status_{status}", "pypi", exchanges=ex)
for query in ["a b", "a/b", "a?b", "a#b", "café"]:
    add("pypi_query_" + query, "pypi", py, query=query)
rg = [
    {
        "name": "rails",
        "version": "8.1.3",
        "info": "Ruby on Rails web framework",
        "downloads": 50000000,
        "authors": "David Heinemeier Hansson",
    },
    {
        "name": "rack",
        "version": "3.1.0",
        "info": "Ruby web server interface",
        "downloads": 40000000,
        "authors": "Christian Neukirchen",
    },
]
add("rubygems_legacy", "rubygems", rg, query="rails")
for label, row in [
    ("description", {"description": "fallback"}),
    ("authors", {"authors": ["a", 1]}),
    ("null", {"name": None, "version": None}),
    ("numeric_info_authors", {"info": 12, "authors": "A"}),
    ("numeric_info", {"info": 12}),
    ("long", {"info": "😀" * 501}),
]:
    add("rubygems_" + label, "rubygems", [row])
dh = {
    "results": [
        {"name": "nginx", "description": "Official nginx image", "pull_count": 1000000000, "star_count": 15000},
        {"name": "ubuntu", "description": "Official Ubuntu base image", "pull_count": 2000000000, "star_count": 12000},
    ]
}
add("dockerhub_legacy", "dockerhub", dh, query="nginx")
for query in [
    "python",
    "library/python",
    "docker.io/library/python:3.13",
    "index.docker.io/library/python@sha256:abc",
    "registry-1.docker.io/library/python:tag",
    "acme/python",
    "docker.io/acme/python:3.13",
    "",
    "quay.io/acme/python/image",
    "py",
    "  PYTHON  ",
    "acme///python",
    "MÜNCHEN/python",
    "STRASSE/straße",
]:
    ns = "acme" if "acme" in query and not query.startswith("quay") else "library"
    add(
        "dockerhub_scope_" + query,
        "dockerhub",
        {
            "results": [
                {"name": "python-tools", "namespace": ns},
                {"name": "python", "namespace": ns},
                {"name": "python-dev", "namespace": ns},
            ]
        },
        query=query,
    )
for max_results in [0, 1, 2, 20, 21, 80, 100, 101]:
    add(
        "dockerhub_cap_" + str(max_results),
        "dockerhub",
        {"results": [{"name": "python-tools"}, {"name": "python-dev"}, {"name": "python"}]},
        max_results=max_results,
    )
for label, namespace in [("null", None), ("empty", ""), ("spaces", "  "), ("number", 7), ("library_case", "LiBrArY")]:
    add("dockerhub_namespace_" + label, "dockerhub", {"results": [{"name": "python", "namespace": namespace}]})
for engine in ["rubygems", "dockerhub"]:
    for label, body in [
        ("empty", [] if engine == "rubygems" else {}),
        ("object_empty", {}),
        ("results_null", {"results": None}),
        ("results_wrong", {"results": {}}),
        ("root_null", None),
        ("root_string", '"str"'),
        ("invalid_json", "{"),
        ("row_null", {"results": [None]}),
        ("long_content", {"results": [{"name": "x", "description": "😀" * 501}]}),
    ]:
        add(engine + "_" + label, engine, body)
    for status in [302, 400, 404, 429, 500]:
        add(f"{engine}_status_{status}", engine, exchanges=[{"status": status, "body": ""}])
    add(engine + "_timeout", engine, exchanges=[{"timeout": True}])
    for label, n in [
        ("hugeint", "123456789012345678901234567890"),
        ("bool", "true"),
        ("float", "1234.500"),
        ("negative_zero", "-0.0"),
        ("small_float", "0.00001"),
        ("underflow", "1e-400"),
        ("null", "null"),
        ("string", '"12"'),
        ("scientific", "1e20"),
        ("precise", "1.234567890123456789"),
    ]:
        row = '{"downloads":' + n + "}" if engine == "rubygems" else '{"pull_count":' + n + "}"
        add(
            engine + "_number_" + label,
            engine,
            "[" + row + "]" if engine == "rubygems" else '{"results":[' + row + "]}",
        )


async def capture(c):
    requests = []
    idx = 0

    def handler(r):
        nonlocal idx
        requests.append(
            {
                "method": r.method,
                "url": str(r.url),
                "headers": {
                    k: v for k, v in r.headers.items() if k in ["user-agent", "accept", "authorization", "cookie"]
                },
            }
        )
        if idx >= len(c["exchanges"]):
            raise AssertionError("unexpected request")
        ex = c["exchanges"][idx]
        idx += 1
        if ex.get("timeout"):
            raise httpx.ReadTimeout("source oracle timeout", request=r)
        return httpx.Response(
            ex["status"],
            content=bytes.fromhex(ex["body_hex"]) if "body_hex" in ex else ex["body"].encode(),
            headers=ex.get("headers", {}),
        )

    base = "http://127.0.0.1:12345" + c.get("base_suffix", "")
    adapter = discover_engines(
        {c["engine"]: {"enabled": True, "base_url": base, "max_results": c["max_results"], "timeout_ms": 5000}}
    )[c["engine"]]
    transport = httpx.MockTransport(handler)
    adapter.set_http_transport(transport)
    try:
        r = await adapter.search(c["query"], {"page": 7, "language": "fr", "safesearch": 2})
    finally:
        await adapter.shutdown()
        await transport.aclose()
    c["requests"] = requests
    c["expected"] = {
        "status": r.status.value,
        "results": [
            {
                "url": x.url,
                "title": x.title,
                "content": x.content,
                "engine": x.engine,
                "position": x.position,
                "score": x.score,
                "payload": x.payload,
            }
            for x in r.results
        ],
    }
    c["exchanges"] = c["exchanges"][:idx]


async def main():
    paths = [
        "engines/pypi.py",
        "engines/rubygems.py",
        "engines/dockerhub.py",
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "tests/test_new_engines.py",
        "tests/test_payload.py",
        "tests/test_dockerhub.py",
    ]
    pins = [
        {
            "path": p,
            "git_blob": subprocess.check_output(
                ["git", "rev-parse", "025ab2f2d50febcb573ba62579eecf7e2d07ef3a:" + p], cwd=root, text=True
            ).strip(),
            "sha256": hashlib.sha256((root / p).read_bytes()).hexdigest(),
        }
        for p in paths
    ]
    expected_sources = [
        {
            "path": "engines/pypi.py",
            "git_blob": "e10a1da680178cb333dbcd53ef9dbb0dd5be81b4",
            "sha256": "16b09518585f440ddb20ecdd7bcf398d93004813a4d1e916afc83e18081b9590",
        },
        {
            "path": "engines/rubygems.py",
            "git_blob": "127ff07eec863a450b3c89e1e876201ecea89880",
            "sha256": "29c00458aaf2804872565acaa19df9f447463509b3cc0f4869959c33d82e7849",
        },
        {
            "path": "engines/dockerhub.py",
            "git_blob": "cf1792ec3d34a9e8c535235791c1aeb7a29ea439",
            "sha256": "4915cf45ba079dc47f776f03884c678ee8cc824f94a551ee80057d8c9a5bcbb9",
        },
        {
            "path": "slopsearx/adapter.py",
            "git_blob": "664e853d9ab1330596d01899ebe015701de09e88",
            "sha256": "5883a35c48d0dad1077306fbda21997ee1d32116a8c03633a6b762de0c26d6f3",
        },
        {
            "path": "slopsearx/payload.py",
            "git_blob": "efa149d48bae401f7a0a9df6e50c8c614adecbb5",
            "sha256": "a44a95655566f9fadaf62da1bab8c975f71f3ead6c3cc57f48c411a9c093c40d",
        },
        {
            "path": "tests/test_new_engines.py",
            "git_blob": "4e5293d694bb40332584a9541b6ad41326a01cc0",
            "sha256": "a6df3df9b68fdc8a4b617d4059a5bd26a8cd82c5d8de1574ac9c48d2d14ad8a4",
        },
        {
            "path": "tests/test_payload.py",
            "git_blob": "4b5bc1a0940ad6c1214eb55764fb4aea57188de9",
            "sha256": "a7bc52b2747f0ebc440a4d7ce69671635530c7142d43eb128cb3132e297fb11a",
        },
        {
            "path": "tests/test_dockerhub.py",
            "git_blob": "ab8e9686fc37305ceb629eb3a6f56651ef49a88b",
            "sha256": "61fbc8b4c4c876e474330d711b2fd35b993cf52252cbb7919e8d90e086e56994",
        },
    ]
    if pins != expected_sources:
        raise RuntimeError("A02 source identity drift; semantic review required before capture")
    for c in cases:
        await capture(c)
    out = {
        "schema_version": 1,
        "plan_id": "A02",
        "capture": "source-bound Python3.14 offline MockTransport; synthetic regressions, no providers",
        "source_commit": "025ab2f2d50febcb573ba62579eecf7e2d07ef3a",
        "source_files": pins,
        "cases": cases,
    }
    Path(sys.argv[1]).write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    print(len(cases), hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest())


if __name__ == "__main__":
    asyncio.run(main())
