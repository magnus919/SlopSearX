# Migration-only Python comparison oracle. Native F11 acceptance never invokes it.
# ruff: noqa: E402
import ast
import copy
import dataclasses
import hashlib
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path.cwd()))
from slopsearx import merger, retrieval_url, scholarly

cases = []


def enc(r):
    d = dataclasses.asdict(r)
    d["engines"] = sorted(d["engines"])
    return d


def feeds(f):
    return [[k, [enc(r) for r in rows]] for k, rows in f.items()]


def capture_group(f, query=""):
    before = feeds(f)
    out = original_group(f, query)
    cases.append(dict(kind="group", input=before, query=query, expected=feeds(out)))
    return out


original_group = scholarly.group_publications
scholarly.group_publications = capture_group
for cls, strategy in [
    (merger.PresenceRanker, "presence"),
    (merger.ReciprocalRankFusionRanker, "reciprocal_rank_fusion"),
]:
    original = cls.rank

    def wrap(self, f, query, params=None, _original=original, _strategy=strategy):
        before = feeds(f)
        out = _original(self, f, query, params)
        cases.append(
            dict(
                kind="rank",
                input=before,
                strategy=_strategy,
                budgets=getattr(self, "per_engine_budget", {}),
                expected=[enc(r) for r in out],
            )
        )
        return out

    cls.rank = wrap
import pytest

code = pytest.main(
    [
        "-q",
        "--no-cov",
        "tests/test_scholarly.py",
        "tests/test_scholarly_arxiv_html_identity.py",
        "tests/test_rank_fusion.py",
        "--disable-warnings",
    ]
)
if code:
    raise SystemExit(code)
strings = set()
for source in [
    "tests/test_retrieval_handoff.py",
    "tests/test_scholarly.py",
    "tests/test_scholarly_arxiv_html_identity.py",
]:
    for node in ast.walk(ast.parse(Path(source).read_text())):
        if (
            isinstance(node, ast.Constant)
            and isinstance(node.value, str)
            and (
                node.value.startswith(
                    ("http", "ftp:", "file:", "data:", "javascript:", "vbscript:", "gopher:", "mailto:", "//")
                )
                or node.value in ["", "   "]
            )
        ):
            strings.add(node.value)
strings.update(
    [
        "http://[v1.example]/",
        "https://[bad]/",
        "http://[::1",
        "http://[::]/",
        "http://example.com:",
        "http://example.com:0",
        "http://example.com:65536",
        "http://example.com:٠/",
        "https://%65xample.com",
        "\thttps://example.com\n/",
        "http:foo",
        "https:///foo",
        "http://0x/",
        "http://0377.0377.0377.0377/",
    ]
)
for s in sorted(strings):
    cases.append(dict(kind="retrieval", input=s, expected=list(retrieval_url.classify_retrieval_url(s))))
for s in sorted(strings):
    cases.append(dict(kind="url", input=s, expected=merger._normalise_url(s)))
# Explicit raw duplicate quota and normalized query-order cases.
from slopsearx.adapter import SearchResult

f = {
    "z": [
        SearchResult(title="Title", content="Body", engine="z", url="https://e/a?utm_source=x"),
        SearchResult(title="Title", content="Body", engine="z", url="https://e/a"),
        SearchResult(title="Title", content="Body", engine="z", url="https://e/b"),
    ],
    "a": [
        SearchResult(title="Title", content="Body", engine="z", url="https://e/a", tier=1),
        SearchResult(title="Title", content="Body", engine="z", url="https://e/c"),
    ],
}
for strategy in ["presence", "reciprocal_rank_fusion", "legacy"]:
    r = merger.PresenceRanker({"z": 2}) if strategy != "reciprocal_rank_fusion" else merger.ReciprocalRankFusionRanker()
    r.rank(copy.deepcopy(f), "")
for s in [
    "HTTPS://E/p?z=1&a=x&z=2&utm_source=x#frag",
    "http://e/?UTM_source=x&blank=&q=%FF&q=a+b",
    "http://e/?a=~+%2F&b",
    "https://[broken",
    "relative?utm_source=x&a=1",
]:
    cases.append(dict(kind="url", input=s, expected=merger._normalise_url(s)))
for count in [63, 64, 65, 129]:
    f = {
        "same": [
            SearchResult(
                title="x",
                content="y",
                engine="same",
                url=f"https://e/{i}",
                payload={"domain": "science", "type": "publication", "data": {"doi": "10.1234/same"}},
            )
            for i in range(count)
        ]
    }
    scholarly.group_publications(f)
for length in [31000, 31500, 31900, 32000, 64000, 64001]:
    f = {
        "a": [SearchResult(title="é" * length, content="x", engine="a", url="https://doi.org/10.1234/same")],
        "b": [
            SearchResult(
                title="b",
                content="y",
                engine="b",
                url="https://e/b",
                payload={"domain": "science", "type": "publication", "data": {"doi": "10.1234/same"}},
            )
        ],
    }
    scholarly.group_publications(f)
for i, c in enumerate(cases):
    c["id"] = f"{c['kind']}-{i:04}"

# Additional explicit source cases beyond existing regressions.
for path in [
    "abs/solv-int/9901001",
    "abs/solv-int/9901001v2",
    "html/math.GT/0309136v1",
    "abs/١٢٣٤.١٢٣٤v١",
    "abs/1234.5678v1",
    "html/1234.5678v2",
]:
    f = {"arxiv": [SearchResult("https://arxiv.org/" + path, "Title", "Body", "arxiv")]}
    scholarly.group_publications(f)
urls = [
    "http://[" + s + "]/"
    for s in [
        "v",
        "v1",
        "v1.",
        "v1.x",
        "vG.x",
        "V1.x",
        "1.2.3.4",
        "2001:20::1",
        "2001:30::1",
        "2001:3::1",
        "2001:db8::1",
        "64:ff9b:1::1",
        "64:ff9b::808:808",
        "::ffff:8.8.8.8",
        "::ffff:0:8.8.8.8",
        "3fff::1",
        "4000::1",
        "ff00::1",
    ]
]
for host in [
    "8.8.8.8",
    "08.8.8.8",
    "192.0.0.9",
    "192.0.0.10",
    "192.0.0.8",
    "192.0.0.11",
    "224.0.0.1",
    "240.0.0.1",
    "100.64.0.1",
    "100.128.0.1",
    "0xffffffff",
    "0x08080808",
    "010.010.010.010",
    "1.2.65535",
    "1.2.65536",
    "1.2.3.4.",
    "1..2",
    "0xg",
    "0x1.0x2.0x3.0x4",
    "4294967295",
    "4294967296",
    "1.2.3.4.5",
    "nip.io",
    "localhost",
    "example.com..",
    "127.0.0.1.nip.io",
]:
    urls.append("http://" + host + "/")
for c in "⁇⁈⁉℀℁℅℆⩴︓︖﹕﹖﹟﹫＃／：？＠":
    urls.append("http://x" + c + "y/")
urls += [
    "http://e/?x=*&q=~",
    "http://user@e:bad/",
    "http://127.1:bad/",
    "http://e:999999999999999999999/",
    "http://e:00080/",
    "http://e:00/",
    "http://e\\@x/",
    "http://[::1]x/",
    "http://[::1]:80:90/",
]
for s in urls:
    cases.append(dict(kind="retrieval", input=s, expected=list(retrieval_url.classify_retrieval_url(s))))
    cases.append(dict(kind="url", input=s, expected=merger._normalise_url(s)))

import platform
import unicodedata


def ranges(points):
    output = []
    for n in points:
        if output and output[-1][1] + 1 == n:
            output[-1][1] = n
        else:
            output.append([n, n])
    return output


root = Path(__file__).resolve().parents[4]
out = root / "docs/rust-rewrite/f11"
for path in ["²³", "１２３", "١٢٣"]:
    scholarly.group_publications(
        {"x": [SearchResult("https://pubmed.ncbi.nlm.nih.gov/" + path + "/", "Title", "Body", "x")]}
    )
for path in ["ARTICLES/S41586-020-2649-2", "artİcles/s41586-020-2649-2"]:
    scholarly.group_publications({"x": [SearchResult("https://nature.com/" + path, "Title", "Body", "x")]})
for budget in [-1, 0, 1, 2]:
    merger.PresenceRanker({"z": budget}).rank(
        {
            "z": [
                SearchResult("https://e/a", "a", "b", "z"),
                SearchResult("https://e/a", "dup", "b", "z"),
                SearchResult("https://e/b", "b", "b", "z"),
            ]
        },
        "",
    )
for raw in [True, 12, [], {}, False, 0, ""]:
    scholarly.group_publications(
        {
            "arxiv": [
                SearchResult(
                    "https://example.test/p",
                    "Title",
                    "Body",
                    "arxiv",
                    payload={
                        "domain": "science",
                        "type": "publication",
                        "data": {"arxiv_id": raw, "publication_id": "2501.12345v1"},
                    },
                )
            ]
        }
    )
for target in [63999, 64000, 64001]:
    rows = [
        SearchResult(
            "https://example.test/a",
            "a",
            "Body",
            "a",
            payload={"domain": "science", "type": "publication", "data": {"doi": "10.1234/bound"}},
        ),
        SearchResult(
            "https://example.test/b",
            "",
            "Body",
            "b",
            payload={"domain": "science", "type": "publication", "data": {"doi": "10.1234/bound"}},
        ),
    ]
    records = [
        {
            "engine": r.engine,
            "original_position": 1,
            "url": r.url,
            "title": r.title,
            "content": r.content,
            "published_date": r.published_date,
            "identifiers": scholarly.publication_ids(r),
            "payload": r.payload,
        }
        for r in rows
    ]
    from slopsearx.payload import payload_serialized_size

    rows[1].title = "x" * (target - payload_serialized_size({"members": records}))
    scholarly.group_publications({"a": [rows[0]], "b": [rows[1]]})
# Production Linux numeric eligibility is authoritative. These differences
# are separately observed by the stdlib-only Linux AI_NUMERICHOST probe.
for url in ["http://018.1.1.1/", "http://4429711368/", "http://0x100000000/", "http://0x108080808/"]:
    cases.append(dict(kind="retrieval", input=url, expected=list(retrieval_url.classify_retrieval_url(url))))
linux_empty_numeric = {"08.8.8.8", "018.1.1.1", "4294967296", "4429711368", "0x100000000", "0x108080808"}
from urllib.parse import urlparse

for case in cases:
    if case["kind"] != "retrieval":
        continue
    try:
        parsed = urlparse(case["input"])
        host = parsed.hostname
    except ValueError:
        continue
    if host == "010.010.010.010":
        case["darwin_observation"] = case["expected"]
        case["platform_evidence"] = "numeric-platforms.json"
        case["expected"] = ["ok", None, parsed.scheme, case["input"]]
    elif host in linux_empty_numeric:
        case["darwin_observation"] = case["expected"]
        case["platform_evidence"] = "numeric-platforms.json"
        case["expected"] = (
            [
                "ambiguous",
                "URL host is a malformed dotted numeric literal; canonicalization is ambiguous",
                parsed.scheme,
                None,
            ]
            if all(c.isdigit() or c == "." for c in host)
            else ["ok", None, parsed.scheme, case["input"]]
        )
for url in ["http://[fe80::1%eth0]/", "http://[fe80::1%25en0]/", "http://[fe80::1%]/"]:
    cases.append(dict(kind="retrieval", input=url, expected=list(retrieval_url.classify_retrieval_url(url))))
    cases.append(dict(kind="url", input=url, expected=merger._normalise_url(url)))
# Independent review regression: urllib's final-segment params are distinct
# from path, and its scheme tables govern splitting and reconstruction.
import urllib.parse

for scheme in sorted(set(urllib.parse.uses_params + urllib.parse.uses_netloc + ["custom", "mailto", "data"])):
    for suffix in ["/p;", "/p;v", "/a;earlier/p;", "/a;earlier/p;later", ";foo"]:
        url = (scheme + ":" if scheme else "") + suffix + "?utm_source=x&keep=1#frag"
        cases.append(dict(kind="url", input=url, expected=merger._normalise_url(url)))
for url in [
    "https://example.com/p;",
    "https://example.com/a;v/p;",
    "https://example.com/a;v/p/;",
    "https://example.com/p;;",
    "https://pubmed.ncbi.nlm.nih.gov/123;foo",
]:
    cases.append(dict(kind="url", input=url, expected=merger._normalise_url(url)))
for path in [
    "ARTICLES/PMC123/",
    "PMC/ARTICLES/PmC123/",
    "artİcles/PMC123/",
    "artıcleſ/PmC١٢٣/",
    "articles/PMC123;foo",
    "articles/PMC123;",
    "article/PMC123",
    "articles/PMC",
    "articles/PMCX123",
    "articles/PMC123/extra",
    "articles/PMC123%2F",
]:
    scholarly.group_publications(
        {"pmc": [SearchResult("https://pmc.ncbi.nlm.nih.gov/" + path, "Title", "Body", "pmc")]}
    )
for host, first, second in [
    ("pubmed.ncbi.nlm.nih.gov", "123;foo", "123"),
    ("pmc.ncbi.nlm.nih.gov", "ARTICLES/PMC123/", "articles/PMC123/"),
]:
    scholarly.group_publications(
        {
            "a": [SearchResult("https://" + host + "/" + first, "Same title", "Body", "a")],
            "b": [SearchResult("https://" + host + "/" + second, "Same title", "Body", "b")],
        }
    )
for strategy in [merger.PresenceRanker(), merger.ReciprocalRankFusionRanker()]:
    strategy.rank(
        {
            "a": [SearchResult("https://example.com/p;", "Empty params", "Body", "a")],
            "b": [
                SearchResult("https://example.com/p", "Bare", "Body", "b"),
                SearchResult("https://example.com/p;kept", "Nonempty params", "Body", "b"),
            ],
        },
        "",
    )
for i, c in enumerate(cases):
    c["id"] = f"{c['kind']}-{i:04}"
sources = [
    "slopsearx/merger.py",
    "slopsearx/scholarly.py",
    "slopsearx/retrieval_url.py",
    "slopsearx/payload.py",
    "slopsearx/publication_metadata.py",
    "tests/test_scholarly.py",
    "tests/test_scholarly_arxiv_html_identity.py",
    "tests/test_rank_fusion.py",
    "tests/test_retrieval_handoff.py",
]
text_table = {
    "unicode": unicodedata.unidata_version,
    "digit_extra_ranges": ranges(n for n in range(0x110000) if chr(n).isdigit() and not chr(n).isdecimal()),
    "word_ranges": ranges(n for n in range(0x110000) if chr(n).isalnum() or chr(n) == "_"),
}
(out / "unicode.json").write_text(json.dumps(text_table, separators=(",", ":")) + "\n")
lines = "".join(json.dumps(c, ensure_ascii=False, separators=(",", ":")) + "\n" for c in cases)
for old in out.glob("goldens*.jsonl"):
    old.unlink()
shards = []
chunk = ""
for line in lines.splitlines(keepends=True):
    if chunk and len((chunk + line).encode()) > 450_000:
        name = f"goldens-{len(shards):03}.jsonl"
        (out / name).write_text(chunk)
        shards.append(name)
        chunk = ""
    chunk += line
if chunk:
    name = f"goldens-{len(shards):03}.jsonl"
    (out / name).write_text(chunk)
    shards.append(name)
obj = {
    "schema_version": 1,
    "baseline": "7cc420f7ffb61127750e77bf6130e225925f9186",
    "license": "MIT repository regression fixtures; synthetic public identifiers only",
    "oracle": {
        "path": "docs/rust-rewrite/f11/oracle/export.py",
        "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "python": platform.python_version(),
        "platform": platform.system(),
        "unicode": unicodedata.unidata_version,
    },
    "sources": {s: subprocess.check_output(["git", "hash-object", s], text=True).strip() for s in sources},
    "unicode_sha256": hashlib.sha256((out / "unicode.json").read_bytes()).hexdigest(),
    "goldens_sha256": hashlib.sha256(lines.encode()).hexdigest(),
    "shards": shards,
    "case_manifest": {v["id"]: v["kind"] for v in cases},
}
(out / "manifest.json").write_text(json.dumps(obj, indent=2) + "\n")
print("exported", len(cases), "cases")
