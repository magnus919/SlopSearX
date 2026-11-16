"""Temporary A05 offline source oracle. Never invoked by native mandatory gates.

G08 retires this exporter with the legacy backend. Captures originate in source
regressions or explicitly labeled synthetic probes, not live provider traffic.
"""

import ast
import asyncio
import dataclasses
import hashlib
import importlib.metadata
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
import httpx  # noqa: E402

import engines  # noqa: E402, F401
from slopsearx.adapter import discover_engines  # noqa: E402


def literals(path):
    return {
        n.targets[0].id: ast.literal_eval(n.value)
        for n in ast.parse((ROOT / path).read_text()).body
        if isinstance(n, ast.Assign) and isinstance(n.value, ast.Constant)
    }


D = literals("tests/test_duckduckgo_resilience.py")
G = literals("tests/test_google_adapter.py")
HOME = D["HOME_HTML"].decode()
PRIMARY = D["HTML_RESULTS_HTML"].decode()
LITE = D["LITE_RESULTS_HTML"].decode()
WALL = D["CHALLENGE_HTML"].decode()
CASES = []


def ex(body="", status=200, **kw):
    return dict(status=status, body_hex=body.encode().hex(), headers={}, **kw)


def add(id, engine, responses, **kw):
    CASES.append(
        dict(
            id=id,
            engine=engine,
            query=kw.pop("query", "test query"),
            max_results=10,
            exchanges=([ex(HOME)] if engine == "duckduckgo" else []) + responses,
            provenance="synthetic source-contract probe",
            **kw,
        )
    )


for engine, good in [("duckduckgo", PRIMARY), ("google", G["SAMPLE_HTML"])]:
    add(
        engine + "_success",
        engine,
        [ex(good)],
        source="tests/test_duckduckgo_resilience.py" if engine == "duckduckgo" else "tests/test_google_adapter.py",
    )
    for status in [403, 429, 500, 503]:
        add(
            f"{engine}_status_{status}",
            engine,
            [ex(status=status)] + ([ex(LITE)] if engine == "duckduckgo" and status != 429 else []),
        )
    add(engine + "_timeout", engine, [ex(timeout=True)])
    add(engine + "_network", engine, [ex(network=True)] + ([ex(LITE)] if engine == "duckduckgo" else []))
    for name, body in [
        ("wall", WALL),
        ("empty_document", ""),
        ("valid_empty", '<html><div class="no-results">No results for xyz</div></html>'),
        ("soft_empty", "<html></html>"),
        ("plain_text", "plain text"),
        ("comment_only", "<!-- comment -->"),
        (
            "malformed",
            '<div class="result"><a class="result__a" href="/relative">a<b>b</div><div class='
            '"g"><a href="https://x"><h3>x',
        ),
    ]:
        add(
            engine + "_" + name,
            engine,
            [ex(body)] + ([ex(LITE)] if engine == "duckduckgo" and name != "valid_empty" else []),
        )
    for limit in [-1, 0, 1, 2]:
        add(
            f"{engine}_limit_{limit}",
            engine,
            [ex(good)] + ([ex(LITE)] if engine == "duckduckgo" and limit <= 0 else []),
            limit=limit,
        )
for engine, good in [("duckduckgo", PRIMARY), ("google", G["SAMPLE_HTML"])]:
    for label, value in [
        ("null", None),
        ("true", True),
        ("false", False),
        ("float", 1.5),
        ("string", "2"),
        ("list", []),
        ("object", {}),
    ]:
        add(
            engine + "_config_max_" + label,
            engine,
            [ex(good)]
            + (
                [ex(LITE)]
                if engine == "duckduckgo" and (value is None or not isinstance(value, (int, float)) or not value)
                else []
            ),
            limit=value,
        )

for name, primary, fallback in [
    ("wall_lite", ex(WALL), ex(LITE)),
    ("wall_wall", ex(WALL), ex(WALL)),
    ("wall_empty", ex(WALL), ex("No results for q")),
    ("soft_soft", ex("<html/>"), ex("<html/>")),
    ("soft_empty", ex("<html/>"), ex("No results for q")),
    ("fallback_timeout", ex(WALL), ex(timeout=True)),
    ("fallback_rate", ex(WALL), ex(status=429)),
    ("fallback_500", ex(WALL), ex(status=500)),
]:
    add("ddg_" + name, "duckduckgo", [primary, fallback])
for name, body in [
    (
        "image",
        '<div class="tile--img"><img src="//img.example/thumb.jpg" alt="Photo"><a href="h'
        'ttps://page.example/full">Source</a><span class="tile__caption">caption</span></'
        "div>",
    ),
    ("image_no_src", '<div class="tile--img"><a href="https://page.example/full">Source only, no image</a></div>'),
    ("image_empty", "<html/>"),
    ("image_wall", WALL),
    (
        "image_alternates",
        '<div class="image-result"><img src="" data-src="/relative" alt="" title="Title">'
        '<a href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fx%2F%3Fa%3D1&amp;k=2">link</a><d'
        'iv class="caption">C</div></div>',
    ),
]:
    add("ddg_" + name, "duckduckgo", [ex(body)], categories=["images"])
for name, body in [
    (
        "encoded",
        '<div class="result"><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fx&amp;k=2">A</a></div>',
    ),
    (
        "positions",
        '<div class="result">skip</div><div class="result"><a class="result__a" href="">e'
        'mpty</a></div><div class="result"><a class="result__a" href="/relative"><b>A</b>'
        " B<script>script</script></a></div>",
    ),
    (
        "nested",
        '<div class="result"><a class="result__a" href="/a">a</a><div class="result"><a c'
        'lass="result__a" href="/b">b</a></div></div>',
    ),
]:
    add("ddg_" + name, "duckduckgo", [ex(body)])
add(
    "google_positions_dedup",
    "google",
    [
        ex(
            '<div class="g">skip</div><div class="g"><a href="/relative"><h3>skip</h3></a></d'
            'iv><div class="g"><a href="http:odd"><h3>A &amp; B</h3></a><span class="aCOpRe">'
            'first</span><span class="st">second</span></div><div class="g"><a href="http:odd'
            '">duplicate</a></div><div class="g"><a href="https://z">empty title</a></div>'
        )
    ],
)
add(
    "ddg_lite_relative",
    "duckduckgo",
    [
        ex(WALL),
        ex(
            '<table><tr><td><a class="result-link" href="/relative">A<b>B</b></a></td></tr><t'
            'r><td class="x-result-snippet-y">Snippet</td></tr></table>'
        ),
    ],
)
add(
    "ddg_lite_blank_uddg",
    "duckduckgo",
    [
        ex(WALL),
        ex(
            '<table><tr><td><a class="result-link" href="//duckduckgo.com/l/?uddg=&amp;uddg=h'
            'ttps%3A%2F%2Fx">A</a></td></tr></table>'
        ),
    ],
)
add("ddg_bootstrap_failure", "duckduckgo", [ex(PRIMARY)], bootstrap_failure=True)
add("ddg_cookie_continuity", "duckduckgo", [ex(PRIMARY)], cookie=True)


for engine in ["duckduckgo", "google"]:
    for label, body in [
        ("whitespace", "  \n \t"),
        ("doctype_only", "<!doctype html>"),
        ("doctype_comment", "<!DOCTYPE html><!--x-->"),
        ("html_comment", "<html><!--x--></html>"),
        ("comment_prefix_valid", "<!--x--><html></html>"),
    ]:
        add(engine + "_" + label, engine, [ex(body)] + ([ex(LITE)] if engine == "duckduckgo" else []))
    markers = (
        [
            "challenge",
            "verify you're human",
            "hcaptcha",
            "cf-browser-verification",
            "ddg_sl_",
            "data-challenge",
            "bots use duckduckgo",
            "anomaly detected",
        ]
        if engine == "duckduckgo"
        else [
            "challenge",
            "recaptcha",
            "cf-browser-verification",
            "unusual traffic",
            "verify you're not a robot",
            "g-recaptcha",
        ]
    )
    for i, marker in enumerate(markers):
        add(engine + "_marker_" + str(i), engine, [ex(marker.upper())] + ([ex(LITE)] if engine == "duckduckgo" else []))
    add(engine + "_max_null_empty", engine, [ex("No results for q")], limit=None)
add(
    "google_config_query",
    "google",
    [ex(G["SAMPLE_HTML"])],
    config=dict(base_url="https://custom.example/search?old=gone"),
)
add(
    "ddg_cross_site_headers",
    "duckduckgo",
    [ex(PRIMARY)],
    config=dict(bootstrap_url="http://home.example/", base_url="http://other.example/results?kept=1"),
)
add(
    "ddg_same_origin_headers",
    "duckduckgo",
    [ex(PRIMARY)],
    config=dict(bootstrap_url="https://html.duckduckgo.com/", base_url="https://html.duckduckgo.com/html/"),
)

add(
    "ddg_domain_cookie", "duckduckgo", [ex(PRIMARY)], cookie=True, cookie_value="a05=one; Domain=duckduckgo.com; Path=/"
)
add(
    "ddg_cookie_same_host",
    "duckduckgo",
    [ex(PRIMARY)],
    cookie=True,
    config=dict(bootstrap_url="https://html.duckduckgo.com/"),
)
add(
    "ddg_cookie_unrelated_host",
    "duckduckgo",
    [ex(PRIMARY)],
    cookie=True,
    config=dict(base_url="https://unrelated.example/html/"),
)


add("ddg_lite_limit_one", "duckduckgo", [ex(WALL), ex(LITE)], limit=1)
add(
    "ddg_lite_invalid_links",
    "duckduckgo",
    [
        ex(WALL),
        ex(
            '<table><tr><td><a class="result-link">Missing</a><a class="result-link" href="/b'
            'lank"> </a><a class="result-link" href="/ok">OK</a></td></tr><tr><td class="resu'
            'lt-snippet">S</td></tr></table>'
        ),
    ],
)
add(
    "ddg_image_positions",
    "duckduckgo",
    [
        ex(
            '<div class="tile">skip</div><div class="tile"><img data-src="/relative"><a href='
            '"/a"></a></div><div class="tile"><a href="/b">B</a></div>'
        )
    ],
    categories=["images"],
    limit=1,
)
add(
    "google_nested_markup",
    "google",
    [
        ex(
            '<div class="g"><a href="https://a"><h3>A<b>B</b></h3></a><div class="g"><a href='
            '"https://b"><h3>C</h3></a><div class="VwiC3b">D</div></div></div>'
        )
    ],
)


for engine in ["duckduckgo", "google"]:
    markup = (
        '<div class="result"><a class="result__a" href="/a">Caf\u00e9 \u0080</a></div>'
        if engine == "duckduckgo"
        else '<div class="g"><a href="https://a"><h3>Caf\u00e9 \u0080</h3></a></div>'
    )
    for charset in ["iso-8859-1", "windows-1252", "ascii", "bogus"]:
        item = dict(
            status=200,
            body_hex=markup.encode("latin1").hex(),
            headers={"content-type": "text/html; charset=" + charset},
        )
        add(engine + "_charset_" + charset, engine, [item])


# Append-only independent-review corrections. The first 115 source records are
# preserved verbatim; every new case runs the original whole-search adapter.
for id, body, categories in [
    ("ddg_context_result", '<a class="result result__a" href="https://x">X</a>', []),
    ("ddg_context_image_link", '<a class="tile" href="https://x"><img src="https://i" alt="X"></a>', ["images"]),
    ("ddg_context_image_src", '<img class="tile" src="https://i" alt="X">', ["images"]),
    (
        "ddg_context_snippet",
        '<div class="result result__snippet"><a class="result__a" href="https://x">X</a>SNIP</div>',
        [],
    ),
    ("ddg_context_url", '<a class="result result__a result__url" href="https://ignored">https://x</a>', []),
]:
    add(id, "duckduckgo", [ex(body)], categories=categories)
add("google_context_snippet", "google", [ex('<div class="g VwiC3b"><a href="https://x"><h3>X</h3></a>SNIP</div>')])
for engine in ["duckduckgo", "google"]:
    for name, body in [
        ("declaration", "<!foo>"),
        ("processing_instruction", "<?foo?>"),
        ("unclosed_comment", "<!-- unterminated"),
        ("declaration_valid_empty", "<!foo><html></html>"),
        ("processing_instruction_text", "<?foo?>plain text"),
        ("comment_entity", "<!-- &nbsp; -->"),
        ("entity_text", "&amp;"),
        ("entity_space", "&#32;"),
        ("entity_nbsp", "&nbsp;"),
        ("entity_markup", "&lt;html&gt;"),
        ("declaration_entity_space", "<!foo> &#32;"),
        ("declaration_entity_inside", "<!foo &nbsp;> "),
    ]:
        add(engine + "_review_" + name, engine, [ex(body)] + ([ex(LITE)] if engine == "duckduckgo" else []))
    for query in ["q*~", "a b+%&=/?:#@[]!$'(),;", "Café 😀 *~%"]:
        add(
            engine + "_query_review_" + str(len(CASES)),
            engine,
            [ex(PRIMARY if engine == "duckduckgo" else G["SAMPLE_HTML"])],
            query=query,
        )
    markup = (
        '<div class="result"><a class="result__a" href="https://x">Café</a></div>'
        if engine == "duckduckgo"
        else '<div class="g"><a href="https://x"><h3>Café</h3></a></div>'
    )
    for charset, encoding in [("utf-32", "utf-32"), ("utf-32-le", "utf-32-le"), ("utf-32-be", "utf-32-be")]:
        item = dict(
            status=200,
            body_hex=markup.encode(encoding).hex(),
            headers={"content-type": "text/html; charset=" + charset},
        )
        add(engine + "_review_" + charset, engine, [item])
add(
    "ddg_image_query_encoding",
    "duckduckgo",
    [ex('<a class="tile" href="https://x"><img src="https://i" alt="X"></a>')],
    categories=["images"],
    query="q*~ Café% +",
)


# Original independent second-review inputs, retained without expected values.
# Replay the actual whole-search source adapters, never a rewritten oracle.
for line in (ROOT / "docs/rust-rewrite/a05/oracle/review-2-inputs.tsv").read_text().splitlines():
    label, engine, charset, body, mode = line.split("\t")
    headers = {"content-type": "text/html; charset=" + charset.split("|")[0]}
    if "|" in charset:
        headers["content-encoding"] = charset.split("|")[1]
    response = dict(status=200, headers=headers, body_hex=body)
    replies = ([ex("challenge")] if mode == "lite" else []) + [response]
    if engine == "duckduckgo" and mode == "text":
        replies.append(ex(LITE))
    add(label, engine, replies, query="q*~", categories=["images"] if mode == "images" else [])


def normalize(v):
    if isinstance(v, set):
        return sorted(v)
    if isinstance(v, dict):
        return {k: normalize(v) for k, v in v.items()}
    if isinstance(v, list):
        return [normalize(x) for x in v]
    return v


async def capture(c):
    requests = []
    consumed = []
    exceptions = []
    capture_failures = []

    def handler(r):
        i = len(requests)
        requests.append(dict(method=r.method, url=str(r.url), headers=dict(r.headers), body_hex=r.content.hex()))
        if i >= len(c["exchanges"]):
            capture_failures.append(i)
            raise AssertionError("unexpected request")
        item = c["exchanges"][i]
        consumed.append(i)
        if item.get("timeout") or (i == 0 and c.get("bootstrap_failure")):
            exceptions.append(dict(exchange=i, type="httpx.ReadTimeout", message="source oracle timeout"))
            raise httpx.ReadTimeout("source oracle timeout", request=r)
        if item.get("network"):
            exceptions.append(dict(exchange=i, type="httpx.ConnectError", message="source oracle network"))
            raise httpx.ConnectError("source oracle network", request=r)
        return httpx.Response(item["status"], content=bytes.fromhex(item["body_hex"]), headers=item["headers"])

    cfg = dict(enabled=True, max_results=c.get("limit", c["max_results"]), timeout_ms=5000)
    if c.get("cookie"):
        c["exchanges"][0]["headers"]["set-cookie"] = c.get("cookie_value", "a05=one; Path=/")
    cfg.update(c.get("config", {}))
    adapter = discover_engines({c["engine"]: cfg})[c["engine"]]
    transport = httpx.MockTransport(handler)
    adapter.set_http_transport(transport)
    try:
        response = await adapter.search(
            c["query"], dict(categories=c.get("categories", []), page=7, language="fr", safesearch=2, time_range="day")
        )
    finally:
        await adapter.shutdown()
        await transport.aclose()
    if capture_failures:
        raise AssertionError((c["id"], capture_failures))
    c["requests"] = requests
    c["consumed_exchanges"] = consumed
    c["resolved_config"] = cfg
    c["exception_metadata"] = exceptions
    c["expected"] = normalize(dataclasses.asdict(response))
    c["expected"].pop("latency_ms")
    c["expected"]["status"] = response.status.value


async def main():
    for c in CASES:
        await capture(c)
    paths = [
        "engines/duckduckgo.py",
        "engines/google.py",
        "slopsearx/adapter.py",
        "slopsearx/http_pool.py",
        "slopsearx/proxypool.py",
        "tests/test_duckduckgo_resilience.py",
        "tests/test_google_adapter.py",
        "tests/test_http_pooling.py",
        "tests/test_adapters.py",
        "tests/test_media_contract.py",
        "tests/test_adapter.py",
    ]
    pins = []
    for p in paths:
        if not (ROOT / p).exists():
            continue
        pins.append(
            dict(
                path=p,
                sha256=hashlib.sha256((ROOT / p).read_bytes()).hexdigest(),
                git_blob=subprocess.check_output(["git", "hash-object", "--", p], cwd=ROOT, text=True).strip(),
            )
        )
    out = dict(
        plan_id="A05",
        source_commit="01876639fb8d87123f12293c02821981cff3f8b9",
        python=sys.version,
        packages={p: importlib.metadata.version(p) for p in ["httpx", "lxml", "cssselect"]},
        source_files=pins,
        cases=CASES,
    )
    base = dict(out, cases=CASES[:115])
    base_bytes = (json.dumps(base, indent=2, ensure_ascii=False) + "\n").encode()
    Path(sys.argv[1]).write_bytes(base_bytes)
    if len(sys.argv) > 2:
        review = dict(out, cases=CASES[115:158], original_corpus_sha256=hashlib.sha256(base_bytes).hexdigest())
        Path(sys.argv[2]).write_text(json.dumps(review, indent=2, ensure_ascii=False) + "\n")
    if len(sys.argv) > 3:
        review2 = dict(out, cases=CASES[158:])
        Path(sys.argv[3]).write_text(json.dumps(review2, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    asyncio.run(main())
