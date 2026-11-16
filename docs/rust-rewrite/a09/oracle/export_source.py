# ruff: noqa: E402
# Exact source guards and local root must precede backend imports.
"""Temporary offline A09 migration oracle. Manual invocation only; retire G08."""

import asyncio
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import unicodedata

import httpx

ROOT = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
# Fail before backend import if any actual source/test dependency changed.
SOURCE_GUARDS = {
    "engines/openlibrary.py": "feeb75a9de3c2474304b04bfa45b635cbf1d6066f64e5b8eec036a056c41c6c6",
    "engines/musicbrainz.py": "63cac10045fcd77406a43c1011ae6542e5091d582cee94a4a11232ef4ee2f062",
    "engines/tmdb.py": "72155872efdef1916ee4c37ef4046588c619940bd3e50d3dae20957d133a4a18",
    "engines/nominatim.py": "e2abcb603545e4fefedd3aa26f8daea122707a6c698200a430cbf9231112d632",
    "engines/internetarchive.py": "6a4a0cdc10f2bc206380e3b67cbd82cbb329d02f9bbe0e5471ea483bdc44c4b4",
    "slopsearx/adapter.py": "5883a35c48d0dad1077306fbda21997ee1d32116a8c03633a6b762de0c26d6f3",
    "slopsearx/payload.py": "a44a95655566f9fadaf62da1bab8c975f71f3ead6c3cc57f48c411a9c093c40d",
    "slopsearx/config.py": "1cf76b8732439782342a9d217fb2f7c423f688533f287dbf78729a1defc5236e",
    "slopsearx/formatter.py": "8eaf74e1215ebaebc7a9d81438e02512aa56b9f5a7929ebb2df35e5b2f632e7a",
    "slopsearx/merger.py": "5bb1a599ed8330aad02bd6567984d12c02f4ed735059c4f50cd6dfefd21e5eda",
    "tests/test_internetarchive_adapter.py": "ec75ebdf9184ebb3c352323a96cf03ae6cf8f7946e5149b9290e2c13c5d36baa",
    "tests/test_openlibrary_url_contract.py": "7e8a1d198551a56d72e94372d455ca7df11d5f5e8c33480a1a72c7968f908672",
    "tests/test_tmdb.py": "ae06578f92be07aa382b5779d120d0588aad75ae306e1c9b10e968a429d9d180",
    "tests/test_new_engines.py": "a6df3df9b68fdc8a4b617d4059a5bd26a8cd82c5d8de1574ac9c48d2d14ad8a4",
    "tests/test_tier1_adapters.py": "1ddcbbbe307319500bc456a388a9650b8e81fab574d20c8df96a91cdf2d04679",
    "tests/test_payload.py": "a7bc52b2747f0ebc440a4d7ce69671635530c7142d43eb128cb3132e297fb11a",
    "tests/test_adapters.py": "0f5fdc19df84214e5a6422437686d41def5f9e9827ad8dfc8ca8ee8b2ad10454",
}
for source_path, expected_sha in SOURCE_GUARDS.items():
    if hashlib.sha256((ROOT / source_path).read_bytes()).hexdigest() != expected_sha:
        raise RuntimeError("A09 oracle source drift: " + source_path)

if sys.version_info[:3] != (3, 14, 7) or unicodedata.unidata_version != "16.0.0":
    raise RuntimeError("A09 oracle requires Python 3.14.7 and Unicode 16.0.0, including optimized mode")
from engines.internetarchive import (
    InternetArchiveAdapter,
)
from engines.musicbrainz import (
    MusicBrainzAdapter,
)
from engines.nominatim import NominatimAdapter
from engines.openlibrary import (
    OpenLibraryAdapter,
)
from engines.tmdb import TMDBAdapter

CLASSES = dict(
    openlibrary=OpenLibraryAdapter,
    musicbrainz=MusicBrainzAdapter,
    tmdb=TMDBAdapter,
    nominatim=NominatimAdapter,
    internetarchive=InternetArchiveAdapter,
)
CASES = []
OL = dict(
    key="/works/OL166894W",
    title="Dune",
    author_name=["Frank Herbert"],
    first_publish_year=1965,
    edition_count=8,
    cover_i=123,
    ratings_count=7,
)
MB = dict(id="artist-1", name="Artist", type="Group", country="GB", **{"life-span": {"begin": "1985"}})
TM = dict(
    media_type="movie",
    id=42,
    title="Movie",
    release_date="2024-02-03",
    overview="O" * 530,
    vote_average=7.5,
    poster_path="/poster.jpg",
)
NO = dict(
    display_name="Paris",
    category="place",
    type="city",
    osm_type="relation",
    osm_id=123,
    place_id=9,
    importance=0.9,
    lat="48.8",
    lon="2.3",
)
IA = dict(identifier="book-1", title="Book", description="Description", downloads=5000, date="2024-01-01")
SUCCESS = dict(
    openlibrary={"docs": [OL]},
    musicbrainz={"artists": [MB]},
    tmdb={"results": [TM]},
    nominatim=[NO],
    internetarchive={"response": {"docs": [IA]}},
)
EMPTY = dict(
    openlibrary={"docs": []},
    musicbrainz={"artists": []},
    tmdb={"results": []},
    nominatim=[],
    internetarchive={"response": {"docs": []}},
)


def add(id, engine, body=None, config=None, query="test", responses=None):
    cfg = {"api_key": "synthetic-tmdb-key"} if engine == "tmdb" else {}
    cfg.update(config or {})
    CASES.append(
        dict(
            id=id,
            engine=engine,
            query=query,
            config=cfg,
            responses=responses if responses is not None else [dict(status=200, body=body)],
        )
    )


def row(engine, record):
    return {
        "openlibrary": {"docs": [record]},
        "musicbrainz": {"artists": [record]},
        "tmdb": {"results": [record]},
        "nominatim": [record],
        "internetarchive": {"response": {"docs": [record]}},
    }[engine]


for engine in CLASSES:
    for label, body in [
        ("success", SUCCESS[engine]),
        ("empty", EMPTY[engine]),
        ("missing", {}),
        ("null", None),
        ("scalar", 42),
        ("malformed-row", row(engine, None)),
    ]:
        add(engine + "-" + label, engine, body, query=" a & b / ")
    for code in [301, 401, 403, 404, 408, 429, 503]:
        add(engine + f"-http-{code}", engine, responses=[dict(status=code, body={})])
    add(engine + "-bad-json", engine, responses=[dict(status=200, raw="{broken")])
    add(engine + "-timeout", engine, responses=[dict(exception="timeout")])
    add(engine + "-network", engine, responses=[dict(exception="runtime", message="synthetic network failure")])
    for value in [0, -1, True, 2.0, "2"]:
        add(engine + "-max-" + repr(value), engine, SUCCESS[engine], config={"max_results": value})
    for value in [True, "5000"]:
        add(engine + "-timeoutcfg-" + repr(value), engine, SUCCESS[engine], config={"timeout_ms": value})
    for value in [None, "", {}, [None], 42]:
        collection = {"openlibrary": "docs", "musicbrainz": "artists", "tmdb": "results"}.get(engine)
        body = {collection: value} if collection else value if engine == "nominatim" else {"response": {"docs": value}}
        add(engine + "-collection-" + repr(value), engine, body)
for field, values in {
    "key": [" OL١٢W ", "works/OL3W", "OL4W", "/books/OL5M", None, "/works/OL4W\n", 42],
    "edition_key": ["OL8M", [None, "bad", "/books/OL9M"], {}, ["OL١٢M"]],
    "isbn": [
        "978-0441172719",
        [1234567890123],
        ["030640615x"],
        ["12345678901"],
        ["1234X67890"],
        [True],
        ["bad", "9780441172719"],
    ],
    "author_name": [[], None, "Ab", {}, [None], [42]],
    "ratings_count": [None, False, " ١_٢ ", [], "bad"],
    "cover_i": [None, 0, False, [1], {"id": 2}],
    "title": [None, 42, "🙂" * 600],
}.items():
    for i, v in enumerate(values):
        record = dict(OL)
        if field in ("edition_key", "isbn"):
            record.pop("key")
        record[field] = v
        add(f"openlibrary-{field}-{i}", "openlibrary", row("openlibrary", record))
add(
    "openlibrary-identity-priority",
    "openlibrary",
    {
        "docs": [
            OL,
            {"title": "Edition", "edition_key": ["bad", "OL2M"]},
            {"title": "Unknown"},
            {"title": "ISBN", "isbn": ["9780441172719"]},
        ]
    },
)
for field, values in {
    "life-span": [None, False, [], {}, "bad", {"begin": 42}],
    "type": [42, None, [], ""],
    "country": [42, None, ""],
    "id": [None, 42, {"x": [1]}],
    "name": [None, 42],
}.items():
    for i, v in enumerate(values):
        record = dict(MB)
        record[field] = v
        add(f"musicbrainz-{field}-{i}", "musicbrainz", row("musicbrainz", record))
for field, values in {
    "media_type": ["tv", "person", "mIxEd", None, "", 42],
    "vote_average": [None, 0, False, "0", " ١_٢ ", "", [], "bad"],
    "overview": [None, False, 42, []],
    "release_date": [None, 42, "x"],
    "title": [None, 42],
    "poster_path": [None, False, 42, {"x": [1]}],
}.items():
    for i, v in enumerate(values):
        record = dict(TM)
        record[field] = v
        record["name"] = "Name"
        record["first_air_date"] = "2020-01-01"
        add(f"tmdb-{field}-{i}", "tmdb", row("tmdb", record))
for i, cfg in enumerate(
    [
        {"api_key": ""},
        {"api_key": "   "},
        {"api_key": "\u001c"},
        {"api_key": "", "ENGINE_TMDB_API_KEY": "synthetic-tmdb-key"},
    ]
):
    add(f"tmdb-key-{i}", "tmdb", EMPTY["tmdb"], config=cfg)
for field, values in {
    "importance": [None, 0, False, "0", " ١_٢ ", "bad", []],
    "osm_id": [0, None, False, "0", {"x": [1]}],
    "osm_type": [None, "", 42],
    "display_name": ["🙂" * 101, None, 42],
    "category": [None, 42],
}.items():
    for i, v in enumerate(values):
        record = dict(NO)
        record[field] = v
        add(f"nominatim-{field}-{i}", "nominatim", row("nominatim", record))
for field, values in {
    "downloads": [None, False, " ١_٢ ", "bad"],
    "date": [None, 42],
    "description": [None, [], 42],
    "title": [None, 42],
    "identifier": [None, 0, {"x": [1]}],
}.items():
    for i, v in enumerate(values):
        record = dict(IA)
        record[field] = v
        add(f"internetarchive-{field}-{i}", "internetarchive", row("internetarchive", record))
CDX = [["urlkey", "timestamp", "original", "mimetype"], ["key", "20240102123456", "https://example.com/a", "text/html"]]
CLOSEST = dict(
    available=True,
    status="200",
    timestamp="20240102123456",
    url="http://web.archive.org/web/20240102123456/https://example.com/",
)
for label, body in [
    ("success", CDX),
    ("empty", []),
    ("header", [[]]),
    ("skips", [[], None, [], [1, "bad", ""], [1, "20240102123456", "", ""]]),
    ("unicode-date", [[], [1, "٢٠٢٤٠١٠٢١٢٣٤٥٦", "https://other.example/", None]]),
    ("bad-outer", {}),
    ("bad-header", [42]),
]:
    add(
        "cdx-" + label,
        "internetarchive",
        query="example.com",
        responses=[dict(status=200, body=body), dict(status=200, body={"archived_snapshots": {}})],
    )
for label, reply in [
    ("timeout", dict(exception="timeout")),
    ("json", dict(status=200, raw="bad")),
    ("403", dict(status=403, body={})),
    ("429", dict(status=429, body={})),
]:
    add(
        "cdx-fallback-" + label,
        "internetarchive",
        query="example.com",
        responses=[reply, dict(status=200, body={"archived_snapshots": {"closest": CLOSEST}})],
    )
for field, values in {
    "available": [False, 1, "true", None],
    "status": [200, 404, True, None],
    "timestamp": ["bad", "20240102123455", None],
    "url": [
        "https://evil.example/web/20240102123456/https://example.com/",
        "https://web.archive.org:443/web/20240102123456/https://example.com/",
        "https://user@web.archive.org/web/20240102123456/https://example.com/",
        "https://web.archive.org/web/20240102123456/https://other.example/",
        "https://web.archive.org/web/20240102123456/https://example.com/?q=x",
        "https://web.archive.org/web/20240102123456/https://example.com/#x",
        "https://WEB.ARCHIVE.ORG/web/20240102123456/https://EXAMPLE.COM/",
        None,
    ],
}.items():
    for i, v in enumerate(values):
        closest = dict(CLOSEST)
        closest[field] = v
        add(
            f"availability-{field}-{i}",
            "internetarchive",
            query="example.com",
            responses=[dict(status=500, body={}), dict(status=200, body={"archived_snapshots": {"closest": closest}})],
        )
for label, reply in [
    ("none", dict(status=200, body=None)),
    ("missing", dict(status=200, body={})),
    ("empty", dict(status=200, body={"archived_snapshots": {}})),
    ("bad-json", dict(status=200, raw="bad")),
    ("timeout", dict(exception="timeout")),
] + [(str(code), dict(status=code, body={})) for code in [401, 403, 408, 429, 503]]:
    add("availability-" + label, "internetarchive", query="example.com", responses=[dict(status=500, body={}), reply])
for i, query in enumerate(
    [
        "example.com",
        "sub.example.org",
        "my-site.co.uk",
        "python programming",
        "hello world",
        "test",
        "éxample.com",
        "x.İıſK",
        "x\u0301.com",
        " x.com ",
        "a_b.org",
    ]
):
    import engines.internetarchive as archive

    body = CDX if archive._is_domain_query(query) else EMPTY["internetarchive"]
    add(f"archive-routing-{i}", "internetarchive", body, query=query)

# Source trust/parser distinctions are frozen, including intentionally weak CDX validation.
for i, url in enumerate(
    [
        "HTTPS://web.archive.org/web/20240102123456/HTTPS://example.com/",
        "https://web.archive.org:/web/20240102123456/https://example.com/",
        "https://web.archive.org/web/20240102123456/https://user:pass@example.com:443/p",
        "https://web.archive.org/web/20240102123456/https://example.com/p;params",
        "https://web.archive.org/web/20240102123456/https://example.com/p?",
        "https://web.archive.org/web/20240102123456/https://example.com/p#",
        "https://web.archive.org/web/20240102123456/https://example.com./",
        "https://web.archive.org/web/20240102123456/https://example.com:bad/",
        "https://web.archive.org/web/20240102123456/https://[broken/",
    ]
):
    closest = dict(CLOSEST)
    closest["url"] = url
    add(
        f"availability-url-structural-{i}",
        "internetarchive",
        query="example.com",
        responses=[dict(status=500, body={}), dict(status=200, body={"archived_snapshots": {"closest": closest}})],
    )
for i, kind in enumerate(["ßΣ", "ǅABC", "İΣ", "AΣ", "AΣ'", "AΣ'A", "ﬃMOVIE"]):
    record = dict(TM)
    record["media_type"] = kind
    record["name"] = "Name"
    add(f"tmdb-unicode-capitalize-{i}", "tmdb", row("tmdb", record))
add(
    "openlibrary-isbn-strip-after-remove", "openlibrary", {"docs": [{"title": "ISBN", "isbn": ["-\t9780441172719-\t"]}]}
)

for engine, field, bad_values in [
    ("openlibrary", "title", [None, 42]),
    ("musicbrainz", "name", [None, 42]),
    ("tmdb", "title", [None, 42]),
    ("internetarchive", "title", [None, 42]),
    ("internetarchive", "date", [42]),
]:
    templates = {"openlibrary": OL, "musicbrainz": MB, "tmdb": TM, "internetarchive": IA}
    for i, bad in enumerate(bad_values):
        record = dict(templates[engine])
        record[field] = bad
        collection = [dict(templates[engine]), record]
        body = (
            {"docs": collection}
            if engine == "openlibrary"
            else {"artists": collection}
            if engine == "musicbrainz"
            else {"results": collection}
            if engine == "tmdb"
            else {"response": {"docs": collection}}
        )
        add(f"{engine}-atomic-{field}-{i}", engine, body)

record = dict(TM)
record["overview"] = "🙂" * 201
add("tmdb-unicode-overview-prefix", "tmdb", row("tmdb", record))


for i, url in enumerate(
    [
        "HTTPS://web.archive.org/web/20240102123456/https://example.com/",
        "https://web.archive.org/web/20240102123456/https://example.com/a;",
        "https://web.archive.org/web/20240102123456/https://example.com/a/;",
        "\x00 https://web.archive.org/web/20240102123456/https://example.com/",
    ]
):
    closest = dict(CLOSEST)
    closest["url"] = url
    add(
        f"availability-url-empty-delimiters-{i}",
        "internetarchive",
        query="example.com",
        responses=[dict(status=500, body={}), dict(status=200, body={"archived_snapshots": {"closest": closest}})],
    )


def output(result):
    return dict(
        status=result.status.value,
        error_message=result.error_message,
        results=[
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
        ],
    )


async def main():
    for key in list(os.environ):
        if key.startswith("ENGINE_"):
            os.environ.pop(key)
    out = dict(
        schema_version=1,
        baseline=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        python=sys.version,
        unicode=unicodedata.unidata_version,
        producer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
        source_inventory=[],
        records=[],
    )
    for p in ["engines/" + e + ".py" for e in CLASSES] + [
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "slopsearx/config.py",
    ]:
        out["source_inventory"].append(dict(path=p, sha256=hashlib.sha256((ROOT / p).read_bytes()).hexdigest()))
    for case in CASES:
        calls = []
        replies = iter(case["responses"])

        def handle(req):
            calls.append(
                dict(
                    method=req.method,
                    url=str(req.url),
                    body=req.content.decode(),
                    headers={
                        k: v
                        for k, v in req.headers.items()
                        if k in ["user-agent", "accept", "authorization", "content-type"]
                    },
                )
            )
            reply = next(replies)
            if reply.get("exception") == "timeout":
                raise httpx.ReadTimeout("synthetic timeout", request=req)
            if reply.get("exception") == "runtime":
                raise RuntimeError(reply["message"])
            body = reply.get("raw", json.dumps(reply.get("body"), ensure_ascii=True, separators=(",", ":")))
            reply["wire_body"] = body
            return httpx.Response(reply["status"], content=body.encode())

        adapter = CLASSES[case["engine"]](case["config"])
        adapter.set_http_transport(httpx.MockTransport(handle))
        try:
            result = await adapter.search(case["query"], {"page": 8, "language": "fr", "safesearch": 1})
            result = output(result)
        except Exception as exc:
            result = dict(raised=type(exc).__name__, error_message=str(exc))
        finally:
            await adapter.shutdown()
        out["records"].append({**case, "requests": calls, "output": result})
    p = ROOT / "docs/rust-rewrite/a09/source.json"
    p.write_text(json.dumps(out, ensure_ascii=True, separators=(",", ":"), allow_nan=False) + "\n")
    print(len(CASES), hashlib.sha256(p.read_bytes()).hexdigest())


asyncio.run(main())
