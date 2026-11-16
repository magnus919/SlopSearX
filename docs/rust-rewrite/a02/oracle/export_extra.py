# Temporary G08-retired offline supplemental oracle. Requires explicit output path.
import asyncio
import sys

from export import add, cases, main

cases.clear()
add(
    "pypi_direct_redirect",
    "pypi",
    exchanges=[
        {"status": 302, "body": "", "headers": {"location": "/pypi/final/json"}},
        {"status": 200, "body": '{"info":{"name":"final"}}'},
    ],
)
for phase in ["simple", "detail"]:
    ex = [{"status": 404, "body": ""}, {"status": 200, "body": '<a href="/simple/python/">x</a>'}]
    i = {"simple": 1, "detail": 2}[phase]
    ex = ex[:i] + [{"status": 302, "body": "", "headers": {"location": "/must-not-follow"}}]
    add("pypi_redirect_" + phase, "pypi", exchanges=ex)
for label, detail in [("malformed", "{"), ("null_info", '{"info":null}'), ("root_array", "[]")]:
    add(
        "pypi_detail_" + label,
        "pypi",
        exchanges=[
            {"status": 404, "body": ""},
            {"status": 200, "body": '<a href="/simple/python/">x</a>'},
            {"status": 200, "body": detail},
        ],
    )
add(
    "pypi_duplicate_candidates",
    "pypi",
    max_results=2,
    exchanges=[
        {"status": 404, "body": ""},
        {
            "status": 200,
            "body": (
                '<a href="/simple/python/">A</a><a href="/simple/python/">B</a><a href="/simple/python-other/">C</a>'
            ),
        },
        {"status": 200, "body": '{"info":{"version":"1"}}'},
        {"status": 200, "body": '{"info":{"version":"2"}}'},
    ],
)
add(
    "pypi_cap_before_failed_detail",
    "pypi",
    max_results=1,
    exchanges=[
        {"status": 404, "body": ""},
        {"status": 200, "body": '<a href="/simple/python/">A</a><a href="/simple/python-other/">B</a>'},
        {"status": 500, "body": ""},
    ],
)
add(
    "pypi_zero_cap_source_first_match",
    "pypi",
    max_results=0,
    exchanges=[
        {"status": 404, "body": ""},
        {"status": 200, "body": '<a href="/simple/python/">A</a><a href="/simple/python-other/">B</a>'},
        {"status": 200, "body": '{"info":{}}'},
    ],
)
for engine in ["rubygems", "dockerhub"]:
    add(
        engine + "_existing_query_replaced",
        engine,
        [] if engine == "rubygems" else {},
        query="a & b",
        base_suffix="/search?keep=yes",
    )
add("dockerhub_empty_string_iterable", "dockerhub", {"results": ""})
add("dockerhub_nonempty_mapping_errors", "dockerhub", {"results": {"python": {}}})
if len(sys.argv) > 2 and sys.argv[2] == "--review-fixes":
    cases.clear()
    raw_html = {
        "utf8_without_meta": b"<a href='/simple/caf\xc3\xa9/'>x</a>",
        "latin1_default": b"<a href='/simple/caf\xe9/'>x</a>",
        "c1_default": b"<a href='/simple/a\x80\x91\x9f/'>x</a>",
        "utf8_bom": b"\xef\xbb\xbf<a href='/simple/caf\xc3\xa9/'>x</a>",
        "meta_utf8": b"<meta charset='utf-8'><a href='/simple/caf\xc3\xa9/'>x</a>",
        "meta_latin1": b"<meta charset='iso-8859-1'><a href='/simple/a\x80\xe9/'>x</a>",
        "meta_cp1252": b"<meta charset='windows-1252'><a href='/simple/a\x80\xe9/'>x</a>",
        "bom_over_meta": b"\xef\xbb\xbf<meta charset='iso-8859-1'><a href='/simple/caf\xc3\xa9/'>x</a>",
        "http_equiv_utf8": (
            b"<meta http-equiv='Content-Type' content='text/html; charset=utf-8'><a href='/simple/caf\xc3\xa9/'>x</a>"
        ),
        "utf16le_bom": b"\xff\xfe" + "<a href='/simple/caf\u00e9/'>x</a>".encode("utf-16-le"),
        "utf16be_bom": b"\xfe\xff" + "<a href='/simple/caf\u00e9/'>x</a>".encode("utf-16-be"),
        "whitespace_only": b"  \n\t",
        "comment_only": b"<!-- only -->",
        "doctype_only": b"<!DOCTYPE html>",
        "xml_only": b"<?xml version='1.0'?>",
        "contextual_lower": "<meta charset='utf-8'><a href='/simple/\u03bf\u03c2/'>x</a>".encode(),
    }
    for label, body in raw_html.items():
        add(
            "pypi_review_" + label,
            "pypi",
            query="\u039f\u03a3" if label == "contextual_lower" else "",
            exchanges=[
                {"status": 404, "body": ""},
                {"status": 200, "body_hex": body.hex()},
                {"status": 200, "body": '{"info":{}}'},
            ],
        )
    add("dockerhub_review_query_python_strip", "dockerhub", {"results": []}, query="\u001cpython\u001f")
    add(
        "dockerhub_review_namespace_python_strip",
        "dockerhub",
        {"results": [{"name": "python", "namespace": "\u001clibrary\u001f"}]},
    )
    add(
        "dockerhub_review_name_sort_python_strip",
        "dockerhub",
        {"results": [{"name": "other"}, {"name": "\u001cpython\u001f"}]},
    )

    add(
        "pypi_review_cookie_session",
        "pypi",
        exchanges=[
            {"status": 404, "body": "", "headers": {"set-cookie": "direct=secret; Path=/"}},
            {
                "status": 200,
                "body": "<a href='/simple/python-one/'>A</a><a href='/simple/python-two/'>B</a>",
                "headers": {"set-cookie": "index=one; Path=/"},
            },
            {"status": 200, "body": '{"info":{}}', "headers": {"set-cookie": "index=two; Path=/"}},
            {"status": 200, "body": '{"info":{}}'},
        ],
    )

if len(sys.argv) > 2 and sys.argv[2] == "--meta-fixes":
    cases.clear()
    for label, body in [
        ("meta_content_only", '<meta content="text/html; charset=utf-8"><a href="/simple/café/">x</a>'),
        ("meta_name_content", '<meta name="description" content="charset=utf-8"><a href="/simple/café/">x</a>'),
        ("meta_unknown_then_valid", '<meta charset="bogus"><meta charset="utf-8"><a href="/simple/café/">x</a>'),
    ]:
        add(
            label,
            "pypi",
            query="caf",
            exchanges=[
                {"status": 404, "body": ""},
                {"status": 200, "body": body},
                {"status": 200, "body": '{"info":{}}'},
            ],
        )

asyncio.run(main())
