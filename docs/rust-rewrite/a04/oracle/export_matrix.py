"""Offline-only source capture exporter for A04 edge matrices.

Run from the repository root with the pinned Python 3.14 environment. Every
exchange uses httpx.MockTransport; no DNS, provider, credential, or paid call.
The emitted matrix is a developer oracle and is never imported by Rust tests.
"""

import argparse
import asyncio
import base64
import dataclasses
import json
import math
from unittest.mock import patch

import httpx

from engines.brave import BraveAdapter
from engines.reddit import RedditAdapter
from engines.wikipedia import WikipediaAdapter


def plain(result):
    return {
        "url": result.url,
        "title": result.title,
        "content": result.content,
        "engine": result.engine,
        "score": result.score,
        "position": result.position,
        "category": result.category,
        "published_date": result.published_date,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "media": dataclasses.asdict(result.media) if result.media else None,
        "payload": result.payload,
    }


def response(value, status=200, content_type="application/json"):
    body = value if isinstance(value, bytes) else json.dumps(value, separators=(",", ":")).encode()
    return httpx.Response(status, content=body, headers={"content-type": content_type})


async def capture(case_id, engine, config, query, params, replies):
    seen, index = [], 0

    def handler(request):
        nonlocal index
        seen.append(
            {
                "method": request.method,
                "url": str(request.url),
                "headers": {
                    k.lower(): v
                    for k, v in request.headers.items()
                    if k.lower() in ("accept", "user-agent", "authorization", "x-subscription-token")
                },
            }
        )
        item = replies[min(index, len(replies) - 1)]
        index += 1
        if isinstance(item, BaseException):
            raise item
        return item

    adapter_type = {"brave": BraveAdapter, "wikipedia": WikipediaAdapter, "reddit": RedditAdapter}[engine]
    async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client:
        with patch("httpx.AsyncClient") as mocked:
            mocked.return_value.__aenter__.return_value = client
            result = await adapter_type(config).search(query, params)
    return {
        "id": case_id,
        "engine": engine,
        "config": config,
        "query": query,
        "params": params,
        "request": seen,
        "responses": [
            {"status": r.status_code, "headers": dict(r.headers), "body_b64": base64.b64encode(r.content).decode()}
            for r in replies
            if isinstance(r, httpx.Response)
        ],
        "exception_types": [type(r).__name__ for r in replies if isinstance(r, BaseException)],
        "output": {
            "status": result.status.value,
            "error_message": result.error_message,
            "results": [plain(x) for x in result.results],
            "answers": result.answers,
            "corrections": result.corrections,
            "infoboxes": result.infoboxes,
        },
    }


async def main():
    specs = []
    brave = {"api_key": "offline-fake-token"}
    specs += [
        ("brave.web.absent_results", "brave", brave, "edge", None, [response({"web": {}})]),
        ("brave.web.null_results", "brave", brave, "edge", None, [response({"web": {"results": None}})]),
        (
            "brave.image.bool_dimensions",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [
                response(
                    {
                        "results": [
                            {
                                "title": "bools",
                                "page_url": "https://example.test/",
                                "thumbnail": {"src": ""},
                                "image": {"src": "https://img.test/x", "width": True, "height": False},
                            }
                        ]
                    }
                )
            ],
        ),
        (
            "brave.image.null_fields",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [
                response(
                    {
                        "results": [
                            {
                                "title": None,
                                "page_url": None,
                                "thumbnail": None,
                                "image": {"src": None, "width": None, "height": None},
                            }
                        ]
                    }
                )
            ],
        ),
        (
            "brave.route.disabled",
            "brave",
            {**brave, "_feature_brave_category_routing": False},
            "edge",
            {"categories": ["images", "news"]},
            [response({"web": {"results": []}})],
        ),
        ("brave.answers.null_main", "brave", brave, "edge", None, [response({"mixed": {"main": None}})]),
    ]
    wiki = {}
    specs += [
        (
            "wikipedia.correction_trim",
            "wikipedia",
            wiki,
            "  rust  ",
            None,
            [
                response(["rust", ["Rust programming"], [], []]),
                response({"query": {"pages": {"1": {"title": "Rust programming", "extract": "Language"}}}}),
            ],
        ),
        (
            "wikipedia.position_skipped_page",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [
                response(["Rust", ["Page"], [], []]),
                response({"query": {"pages": {"-1": {"missing": "", "title": "Missing"}, "2": {"title": "Page"}}}}),
            ],
        ),
        (
            "wikipedia.opensearch.http_503_html",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [httpx.Response(503, content=b"gateway failure", headers={"content-type": "text/html; charset=utf-8"})],
        ),
        (
            "wikipedia.opensearch.malformed_json",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response(b"{bad", content_type="text/plain")],
        ),
        (
            "wikipedia.rich_query.http_403",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [
                response(["Rust", ["Rust"]], 200),
                httpx.Response(403, content=b"denied", headers={"content-type": "text/html"}),
            ],
        ),
        (
            "wikipedia.rich_query.wrong_shape",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [
                response(["Rust", ["Rust"]], 200),
                response({"query": {"pages": None}}, content_type="application/json; charset=utf-8"),
            ],
        ),
    ]
    reddit = {}

    def listing(post):
        return {"data": {"children": [{"kind": "t3", "data": post}]}}

    specs += [
        ("reddit.missing_defaults", "reddit", reddit, "edge", None, [response(listing({"title": "x"}))]),
        ("reddit.null_author", "reddit", reddit, "edge", None, [response(listing({"title": "x", "author": None}))]),
        ("reddit.zero_epoch", "reddit", reddit, "edge", None, [response(listing({"title": "x", "created_utc": 0}))]),
        ("reddit.invalid_score", "reddit", reddit, "edge", None, [response(listing({"title": "x", "score": "bad"}))]),
        ("reddit.null_score", "reddit", reddit, "edge", None, [response(listing({"title": "x", "score": None}))]),
        (
            "reddit.invalid_date",
            "reddit",
            reddit,
            "edge",
            None,
            [response(listing({"title": "x", "created_utc": "bad"}))],
        ),
        ("reddit.timeout", "reddit", {"timeout_ms": 10}, "edge", None, [httpx.TimeoutException("timeout")]),
        (
            "reddit.empty_container_nsfw",
            "reddit",
            reddit,
            "edge",
            None,
            [
                response(
                    {
                        "data": {
                            "children": [
                                {"data": {"title": "array", "over_18": []}},
                                {"data": {"title": "object", "over_18": {}}},
                            ]
                        }
                    }
                )
            ],
        ),
        (
            "reddit.raw_token_precedence",
            "reddit",
            {"access_token": " ", "api_key": "fallback"},
            "edge",
            None,
            [response({"data": {"children": []}})],
        ),
        ("reddit.malformed_children", "reddit", reddit, "edge", None, [response({"data": {"children": None}})]),
        ("reddit.nonobject_child", "reddit", reddit, "edge", None, [response({"data": {"children": [None]}})]),
    ]
    specs += [
        (
            "brave.image.empty_full_fallback",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [response({"results": [{"image": {"src": ""}, "thumbnail": {"src": "https://img.test/thumb"}}]})],
        ),
        (
            "brave.image.empty_sources",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [response({"results": [{"image": {"src": ""}, "thumbnail": {"src": ""}}]})],
        ),
        (
            "brave.image.empty_thumbnail_only",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [response({"results": [{"thumbnail": {"src": ""}}]})],
        ),
        (
            "brave.video_empty_thumbnail",
            "brave",
            brave,
            "edge",
            {"categories": ["videos"]},
            [response({"results": [{"url": "https://page.test/v", "thumbnail": {"src": ""}}]})],
        ),
        ("brave.web.empty_object_results", "brave", brave, "edge", None, [response({"web": {"results": {}}})]),
        ("brave.web.empty_string_results", "brave", brave, "edge", None, [response({"web": {"results": ""}})]),
        ("brave.web.nonempty_object_results", "brave", brave, "edge", None, [response({"web": {"results": {"x": 1}}})]),
        ("brave.web.nonempty_string_results", "brave", brave, "edge", None, [response({"web": {"results": "x"}})]),
        ("brave.answers.empty_string_main", "brave", brave, "edge", None, [response({"mixed": {"main": ""}})]),
        ("brave.answers.empty_object_main", "brave", brave, "edge", None, [response({"mixed": {"main": {}}})]),
        ("brave.answers.nonempty_string_main", "brave", brave, "edge", None, [response({"mixed": {"main": "skip"}})]),
        (
            "brave.answers.mapping_main_iterable",
            "brave",
            brave,
            "edge",
            None,
            [response({"mixed": {"main": {"entry": {"type": "answer"}}}})],
        ),
        ("brave.answers.null_main_error", "brave", brave, "edge", None, [response({"mixed": {"main": None}})]),
        (
            "wikipedia.correction_trim_title",
            "wikipedia",
            wiki,
            "rust programming",
            None,
            [
                response(["rust programming", [" rust "], [], []]),
                response({"query": {"pages": {"1": {"title": " rust "}}}}),
            ],
        ),
        (
            "wikipedia.opensearch.shape_65",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response([0] * 65, content_type="application/json")],
        ),
        (
            "wikipedia.opensearch.mime_plain",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response({"bad": True}, content_type="plain")],
        ),
        (
            "wikipedia.opensearch.mime_slash_prefix",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response({"bad": True}, content_type="/text")],
        ),
        (
            "wikipedia.opensearch.mime_double_slash",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response({"bad": True}, content_type="text//html")],
        ),
        (
            "wikipedia.opensearch.mime_empty",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response({"bad": True}, content_type="")],
        ),
        (
            "wikipedia.rich_query.mime_invalid",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response(["Rust", ["Rust"]]), response({"query": {"pages": None}}, content_type="text//html")],
        ),
        (
            "wikipedia.rich_query.shape_65",
            "wikipedia",
            wiki,
            "Rust",
            None,
            [response(["Rust", ["Rust"]]), response({str(i): {} for i in range(65)})],
        ),
        (
            "reddit.whitespace_score_boolean_summary",
            "reddit",
            reddit,
            "edge",
            None,
            [response(listing({"title": "x", "score": " 1.25 ", "num_comments": True, "author": False}))],
        ),
        (
            "reddit.nested_container_summary",
            "reddit",
            reddit,
            "edge",
            None,
            [
                response(
                    listing(
                        {
                            "title": "x",
                            "score": 1.5,
                            "num_comments": [False, None],
                            "author": {"name": "Ada", "active": True},
                        }
                    )
                )
            ],
        ),
        (
            "reddit.oversized_date",
            "reddit",
            reddit,
            "edge",
            None,
            [response(listing({"title": "x", "created_utc": 10**30}))],
        ),
        (
            "reddit.raw_token_both_values",
            "reddit",
            {"access_token": " ", "api_key": "fallback"},
            "edge",
            None,
            [response({"data": {"children": []}})],
        ),
    ]
    # Third-review corrections: append authentic source observations; the first
    # 48 matrix records remain byte-for-byte unchanged.
    for field, values in [
        (
            "score",
            [
                "1_000",
                "+١_٢.٥e+٢",
                "\u00a0-1_2.5_0e-0_1\u00a0",
                "1__0",
                "_10",
                "10_",
                "1_.0",
                "1._0",
                "1e_2",
                "1e+_2",
                "1_0e1_0",
                "  +.5  ",
                "1e",
                "1 0",
                "\x1c1",
            ],
        ),
        (
            "created_utc",
            [
                "1_000",
                "+١_٢",
                "\u00a0-1_0\u00a0",
                "1__0",
                "_10",
                "10_",
                "1.0",
                "1e3",
                "1 0",
                "\x1c1",
                "+",
                "-0",
                0.9,
                -0.9,
                -62135596800,
                -62135596801,
                253402300799,
                253402300800,
                -67768040609740801,
                -67768040609740800,
                67767976233532799,
                67767976233532800,
                67768036191676799,
                67768036191676800,
                67900000000000000,
                9223372036854775808,
                "9223372036854775808",
            ],
        ),
    ]:
        for index, value in enumerate(values):
            specs.append(
                (
                    f"reddit.numeric_grammar.{field}.{index}",
                    "reddit",
                    reddit,
                    "edge",
                    None,
                    [response(listing({"title": "x", field: value}))],
                )
            )
    specs += [
        (
            "reddit.numeric_json_repr",
            "reddit",
            reddit,
            "edge",
            None,
            [
                response(
                    b'{"data":{"children":[{"data":{"title":"x","score":1e3,"num_comments":[1e-7,1e-6,1e16,1e15,-0.0,9223372036854775808],"author":{"n":1.0}}}]}}'
                )
            ],
        ),
        (
            "reddit.nonprintable_repr",
            "reddit",
            reddit,
            "edge",
            None,
            [
                response(
                    listing(
                        {
                            "title": "x",
                            "num_comments": ["\x01", "\x7f", "\u00a0", "\u2028", "\U000e0001", "é", "\n\t\r", "'\"\\"],
                            "author": {"\u200b": "x"},
                        }
                    )
                )
            ],
        ),
        (
            "reddit.date_precedes_score_error",
            "reddit",
            reddit,
            "edge",
            None,
            [response(listing({"title": "x", "created_utc": "bad", "score": "bad"}))],
        ),
        ("reddit.invalid_root_shape", "reddit", reddit, "edge", None, [response([])]),
    ]
    for stage in ["opensearch", "rich_query"]:
        for index, value in enumerate([9223372036854775808, -9223372036854775809, 10**100, 1e3, 1.0]):
            replies = [response(value)]
            if stage == "rich_query":
                replies.insert(0, response(["Rust", ["Rust"]]))
            specs.append((f"wikipedia.{stage}.numeric_shape.{index}", "wikipedia", wiki, "Rust", None, replies))
    for category in ["images", "videos"]:
        for suffix, media_url in [
            ("secrets", "https://media.test/?api_key=fake&token=fake&x=a%20b"),
            ("blank_duplicate", "https://media.test/?x=&x=two%20words&secret=fake#frag"),
            ("relative", "/media?token=fake&x=a%20b"),
        ]:
            item = {
                "title": "media boundary",
                "url": media_url,
                "page_url": media_url,
                "thumbnail": {"src": media_url},
                "image": {"src": media_url},
                "duration": 12,
            }
            specs.append(
                (
                    f"brave.{category}.media_url.{suffix}",
                    "brave",
                    brave,
                    "edge",
                    {"categories": [category]},
                    [response({"results": [item]})],
                )
            )
    # Fourth full-source review: genuine whole-search observations, retaining
    # the first 110 records unchanged. Literal bodies preserve JSON numeric kind.
    review_probes = [
        {
            "id": "brave_duration_1",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":1}]}'],
            "category": "videos",
        },
        {
            "id": "brave_duration_1.0",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":1.0}]}'],
            "category": "videos",
        },
        {
            "id": "brave_duration_1e3",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":1e3}]}'],
            "category": "videos",
        },
        {
            "id": "brave_duration_1e-7",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":1e-7}]}'],
            "category": "videos",
        },
        {
            "id": "brave_duration_1.234567890123456789",
            "engine": "brave",
            "bodies": [
                '{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":1.234567890123456789}]}'
            ],
            "category": "videos",
        },
        {
            "id": "brave_duration_-0.0",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":-0.0}]}'],
            "category": "videos",
        },
        {
            "id": "brave_duration_null",
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":null}]}'],
            "category": "videos",
        },
        {
            "id": 'brave_duration_"12"',
            "engine": "brave",
            "bodies": ['{"results":[{"title":"X","url":"https://p","thumbnail":{"src":"https://i"},"duration":"12"}]}'],
            "category": "videos",
        },
        {"id": "brave_query_encoding", "engine": "brave", "bodies": ['{"web":{"results":[]}}'], "query": "q*~"},
        {"id": "reddit_query_encoding", "engine": "reddit", "bodies": ['{"data":{"children":[]}}'], "query": "q*~"},
        {"id": "wikipedia_query_encoding", "engine": "wikipedia", "bodies": ['["q",[]]'], "query": "q*~"},
        {"id": "brave_answer_int", "engine": "brave", "bodies": ['{"infobox":{"results":[{"description":5}]}}']},
        {
            "id": "wiki_null_extract",
            "engine": "wikipedia",
            "bodies": ['["edge",["edge"]]', '{"query":{"pages":{"1":{"title":"X","extract":null}}}}'],
        },
        {
            "id": "wiki_null_page",
            "engine": "wikipedia",
            "bodies": ['["edge",["edge"]]', '{"query":{"pages":{"1":null}}}'],
        },
        {
            "id": "wiki_extract_strip",
            "engine": "wikipedia",
            "bodies": [
                '["edge",["edge"]]',
                '{"query":{"pages":{"1":{"title":"X","extract":"\\u001cA\\u001fB\\u001e"}}}}',
            ],
        },
        {
            "id": "wiki_correction_strip",
            "engine": "wikipedia",
            "bodies": ['["edge",["\\u001cRust\\u001f"]]', '{"query":{"pages":{}}}'],
            "query": "rust programming",
        },
        {
            "id": "reddit_null_permalink",
            "engine": "reddit",
            "bodies": ['{"data":{"children":[{"data":{"title":"X","url":"https://x","permalink":null}}]}}'],
        },
        {
            "id": "reddit_huge_nsfw",
            "engine": "reddit",
            "bodies": [
                '{"data":{"children":[{"data":{"title":"X","over_18":10000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000}}]}}'
            ],
        },
        {
            "id": "brave_duration_wide_int",
            "engine": "brave",
            "category": "videos",
            "bodies": [
                '{"results":[{"url":"https://p","thumbnail":{"src":"https://i"},"duration":18446744073709551617}]}'
            ],
        },
    ]
    for case in review_probes:
        specs.append(
            (
                "fourth." + case["id"],
                case["engine"],
                brave if case["engine"] == "brave" else {},
                case.get("query", "edge"),
                {"categories": [case["category"]]} if "category" in case else None,
                [response(body.encode()) for body in case["bodies"]],
            )
        )
    for location in ["infobox", "mixed"]:
        for index, value in enumerate([None, False, 0, 0.0, [], {}, True, 1, 1.5, [1, "x"], {"x": 1}, "text"]):
            for mixed in [False, True]:
                entries = [{"description": value}]
                if mixed:
                    entries = [{"description": "before"}] + entries + [{"description": "after"}]
                body = (
                    {"infobox": {"results": entries}}
                    if location == "infobox"
                    else {"mixed": {"main": [{"type": "answer", "data": e} for e in entries]}}
                )
                specs.append(
                    (f"fourth.brave.answer.{location}.{index}.{mixed}", "brave", brave, "edge", None, [response(body)])
                )

    def wiki_page(page):
        return [response(["edge", ["edge"]]), response({"query": {"pages": {"1": page}}})]

    for index, value in enumerate([None, True, 1, 1.0, [], {}, "", "text"]):
        specs.append((f"fourth.wiki.page.{index}", "wikipedia", {}, "edge", None, wiki_page(value)))
        specs.append(
            (f"fourth.wiki.extract.{index}", "wikipedia", {}, "edge", None, wiki_page({"title": "X", "extract": value}))
        )
        if not isinstance(value, str):
            specs.append((f"fourth.wiki.title.{index}", "wikipedia", {}, "edge", None, wiki_page({"title": value})))
        specs.append(
            (
                f"fourth.wiki.shortdesc.{index}",
                "wikipedia",
                {},
                "edge",
                None,
                wiki_page({"title": "X", "pageprops": {"wikibase-shortdesc": value}}),
            )
        )
    specs.append(
        ("fourth.wiki.extract_before_title", "wikipedia", {}, "edge", None, wiki_page({"title": None, "extract": None}))
    )
    for index, value in enumerate([None, False, 0, [], {}, True, 1, [1], {"x": 1}, "", "/r/x"]):
        for url in [None, "https://x"]:
            specs.append(
                (
                    f"fourth.reddit.permalink.{index}.{url is None}",
                    "reddit",
                    {},
                    "edge",
                    None,
                    [response(listing({"title": "X", "url": url, "permalink": value}))],
                )
            )
        if value is not None and not isinstance(value, str):
            specs.append(
                (
                    f"fourth.reddit.url.{index}",
                    "reddit",
                    {},
                    "edge",
                    None,
                    [response(listing({"title": "X", "url": value, "permalink": None}))],
                )
            )
    for index, value in enumerate([True, 1, 1.0, {"x": 1}, None, False, 0, {}, ""]):
        specs.append(
            (
                f"fourth.reddit.selftext.{index}",
                "reddit",
                {},
                "edge",
                None,
                [response(listing({"title": "X", "selftext": value}))],
            )
        )
    for index, value in enumerate([10**400, -(10**400), 0, -0.0, 1e-100, [], {}, True, False]):
        specs.append(
            (
                f"fourth.reddit.nsfw.{index}",
                "reddit",
                {},
                "edge",
                None,
                [response(listing({"title": "X", "over_18": value}))],
            )
        )
    for ch in ["\x1c", "\x1d", "\x1e", "\x1f", "\u0085", "\u00a0"]:
        specs.append(
            (
                f"fourth.wiki.whitespace.{ord(ch)}",
                "wikipedia",
                {},
                "rust programming",
                None,
                [
                    response(["edge", [ch + "Rust" + ch]]),
                    response(
                        {"query": {"pages": {"1": {"title": ch + "X" + ch, "extract": ch + "A" + ch + "B" + ch}}}}
                    ),
                ],
            )
        )
        specs.append(
            (
                f"fourth.reddit.token_whitespace.{ord(ch)}",
                "reddit",
                {"access_token": ch + "token" + ch},
                "edge",
                None,
                [response({"data": {"children": []}})],
            )
        )
    query = " *~%+&=?/é雪\x1c"
    specs += [
        ("fourth.brave.query_punctuation", "brave", brave, query, None, [response({"web": {"results": []}})]),
        ("fourth.reddit.query_punctuation", "reddit", {}, query, None, [response({"data": {"children": []}})]),
        (
            "fourth.wiki.query_and_titles_punctuation",
            "wikipedia",
            {},
            query,
            None,
            [response([query, [query, "A|B*"]]), response({"query": {"pages": {}}})],
        ),
    ]
    for index, raw in enumerate(["0", "-0", "1e-500", "1E+03", "1.234567890123456789", "1.0"]):
        body = ('{"results":[{"thumbnail":{"src":"https://i"},"duration":' + raw + "}]}").encode()
        specs.append(
            (
                f"fourth.brave.duration_lexeme.{index}",
                "brave",
                brave,
                "edge",
                {"categories": ["videos"]},
                [response(body)],
            )
        )
    for index, value in enumerate([None, "12", [], {}, 1.0]):
        specs.append(
            (
                f"fourth.brave.dimension_nonint.{index}",
                "brave",
                brave,
                "edge",
                {"categories": ["images"]},
                [response({"results": [{"image": {"src": "https://i", "width": value, "height": value}}]})],
            )
        )
    specs.append(
        (
            "fourth.brave.no_media_ignores_bad_numbers",
            "brave",
            brave,
            "edge",
            {"categories": ["images"]},
            [response({"results": [{"image": {"width": 10**400}}]})],
        )
    )
    specs += [
        (
            "fourth.wiki.mixed_valid_invalid_extract",
            "wikipedia",
            {},
            "edge",
            None,
            [
                response(["edge", ["edge"]]),
                response(
                    {
                        "query": {
                            "pages": {
                                "1": {"title": "before", "extract": "ok"},
                                "2": {"title": "bad", "extract": None},
                                "3": {"title": "after"},
                            }
                        }
                    }
                ),
            ],
        ),
        (
            "fourth.reddit.mixed_valid_invalid_permalink",
            "reddit",
            {},
            "edge",
            None,
            [
                response(
                    {
                        "data": {
                            "children": [
                                {"data": {"title": "before"}},
                                {"data": {"title": "bad", "url": "https://x", "permalink": None}},
                                {"data": {"title": "after"}},
                            ]
                        }
                    }
                )
            ],
        ),
        (
            "fourth.reddit.empty_list_selftext",
            "reddit",
            {},
            "edge",
            None,
            [response(listing({"title": "X", "selftext": []}))],
        ),
    ]
    for index, value in enumerate([None, True, 1, 1.0, "", [], [{}]]):
        specs.append((f"fourth.brave.root_type.{index}", "brave", brave, "edge", None, [response(value)]))
    for index, value in enumerate([10**400, -(10**400)]):
        specs.append(
            (
                f"fourth.reddit.score_integer_overflow.{index}",
                "reddit",
                {},
                "edge",
                None,
                [response(listing({"title": "X", "score": value}))],
            )
        )
    specs.append(
        (
            "fourth.reddit.mixed_score_integer_overflow",
            "reddit",
            {},
            "edge",
            None,
            [
                response(
                    {
                        "data": {
                            "children": [
                                {"data": {"title": "before"}},
                                {"data": {"title": "bad", "score": 10**400}},
                                {"data": {"title": "after"}},
                            ]
                        }
                    }
                )
            ],
        )
    )
    # Controls for the F05 String/Option<String> output boundary. Original
    # source errors take precedence over any native representational refusal.
    for engine in ["brave", "wikipedia", "reddit"]:
        for kind in ["missing", "null_optional", "empty_strings"]:
            if engine == "brave":
                item = (
                    {}
                    if kind == "missing"
                    else (
                        {"thumbnail": {"src": None}}
                        if kind == "null_optional"
                        else {"title": "", "url": "", "description": "", "thumbnail": {"src": ""}}
                    )
                )
                replies = [response({"web": {"results": [item]}})]
            elif engine == "wikipedia":
                page = {"title": "X"}
                if kind == "null_optional":
                    page["thumbnail"] = {"source": None}
                if kind == "empty_strings":
                    page.update(extract="", thumbnail={"source": ""})
                replies = [response(["edge", ["edge"]]), response({"query": {"pages": {"1": page}}})]
            else:
                post = {"title": "X"}
                if kind == "null_optional":
                    post["thumbnail"] = None
                if kind == "empty_strings":
                    post.update(thumbnail="", selftext="")
                replies = [response(listing(post))]
            specs.append(
                (f"f05.control.{engine}.{kind}", engine, brave if engine == "brave" else {}, "edge", None, replies)
            )
    for index, value in enumerate([None, False, 0, [], {}]):
        specs.append(
            (
                f"f05.control.reddit.false_title.{index}",
                "reddit",
                {},
                "edge",
                None,
                [response(listing({"title": value}))],
            )
        )
        specs.append(
            (
                f"f05.control.reddit.false_thumbnail.{index}",
                "reddit",
                {},
                "edge",
                None,
                [response(listing({"title": "X", "thumbnail": value}))],
            )
        )
    for engine in ["brave", "wikipedia", "reddit"]:
        if engine == "brave":
            boundaries = [
                ("title", {"title": {"name": "x"}}),
                ("duration", {"thumbnail": {"src": "https://i"}, "duration": True}),
            ]
            bad = None
            for name, boundary in boundaries:
                for reverse in [False, True]:
                    items = [boundary, bad] if not reverse else [bad, boundary]
                    specs.append(
                        (
                            f"f05.error_priority.brave.{name}.{reverse}",
                            "brave",
                            brave,
                            "edge",
                            {"categories": ["videos"]},
                            [response({"results": items})],
                        )
                    )
            specs.append(
                (
                    "f05.error_priority.brave.answer_after_boundary",
                    "brave",
                    brave,
                    "edge",
                    None,
                    [response({"web": {"results": [{"title": 1}]}, "infobox": {"results": [{"description": 1}]}})],
                )
            )
        elif engine == "wikipedia":
            for reverse in [False, True]:
                pages = [{"title": "X", "thumbnail": {"source": 1}}, None]
                if reverse:
                    pages.reverse()
                specs.append(
                    (
                        f"f05.error_priority.wiki.{reverse}",
                        "wikipedia",
                        {},
                        "edge",
                        None,
                        [
                            response(["edge", ["edge"]]),
                            response({"query": {"pages": {str(i): page for i, page in enumerate(pages)}}}),
                        ],
                    )
                )
            specs.append(
                (
                    "f05.error_priority.wiki.same_page",
                    "wikipedia",
                    {},
                    "edge",
                    None,
                    [
                        response(["edge", ["edge"]]),
                        response(
                            {"query": {"pages": {"1": {"title": "X", "thumbnail": {"source": 1}, "extract": None}}}}
                        ),
                    ],
                )
            )
        else:
            boundaries = [
                ("title", {"title": {"name": "x"}}),
                ("selftext", {"title": "X", "selftext": ["x"]}),
                ("score", {"title": "X", "score": "nan"}),
                ("thumbnail", {"title": "X", "thumbnail": 1}),
            ]
            bad = {"title": "bad", "url": "https://x", "permalink": None}
            for name, boundary in boundaries:
                for reverse in [False, True]:
                    posts = [boundary, bad] if not reverse else [bad, boundary]
                    specs.append(
                        (
                            f"f05.error_priority.reddit.{name}.{reverse}",
                            "reddit",
                            {},
                            "edge",
                            None,
                            [response({"data": {"children": [{"data": post} for post in posts]}})],
                        )
                    )
            specs.append(
                (
                    "f05.error_priority.reddit.date_after_list",
                    "reddit",
                    {},
                    "edge",
                    None,
                    [response(listing({"title": "X", "selftext": ["x"], "created_utc": "bad"}))],
                )
            )
            specs.append(
                (
                    "f05.error_priority.reddit.startswith_after_title",
                    "reddit",
                    {},
                    "edge",
                    None,
                    [response(listing({"title": {"name": "x"}, "url": 1}))],
                )
            )
    records = [await capture(*spec) for spec in specs]
    print(json.dumps({"schema_version": 1, "plan_id": "A04", "records": records}, ensure_ascii=False, indent=2))


async def capture_nonfinite(output):
    records = []
    for spelling in ["nan", "inf", "-inf"]:
        for mixed in [False, True]:
            posts = [{"data": {"title": "nonfinite", "score": spelling}}]
            if mixed:
                posts = [{"data": {"title": "before", "score": 1}}] + posts + [{"data": {"title": "after", "score": 2}}]
            records.append(
                await capture(
                    f"reddit.nonfinite.{spelling}.{'mixed' if mixed else 'single'}",
                    "reddit",
                    {},
                    "edge",
                    None,
                    [response({"data": {"children": posts}})],
                )
            )

    def tagged(value):
        if isinstance(value, float) and not math.isfinite(value):
            return {"$source_nonfinite_float": "nan" if math.isnan(value) else ("inf" if value > 0 else "-inf")}
        if isinstance(value, dict):
            return {k: tagged(v) for k, v in value.items()}
        if isinstance(value, list):
            return [tagged(v) for v in value]
        return value

    from pathlib import Path

    Path(output).write_text(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A04",
                "encoding": (
                    "Only actual nonfinite float values are tagged with $source_nonfinite_float; "
                    "this is an observation encoding, not a native expected result."
                ),
                "records": tagged(records),
            },
            ensure_ascii=False,
            indent=2,
            allow_nan=False,
        )
        + "\n"
    )


async def capture_representation_boundaries(output):
    from pathlib import Path

    records = []
    variants = [
        (
            "duration_true",
            "videos",
            {"thumbnail": {"src": "https://i"}, "duration": True},
            "Brave video duration must be a finite non-boolean number",
        ),
        (
            "duration_false",
            "videos",
            {"thumbnail": {"src": "https://i"}, "duration": False},
            "Brave video duration must be a finite non-boolean number",
        ),
        (
            "duration_overrange",
            "videos",
            {"thumbnail": {"src": "https://i"}, "duration": 10**400},
            "Brave video duration must be a finite non-boolean number",
        ),
        (
            "width_overrange",
            "images",
            {"image": {"src": "https://i", "width": 18446744073709551617}},
            "Brave image width must fit a signed 64-bit integer",
        ),
        (
            "height_overrange",
            "images",
            {"image": {"src": "https://i", "height": -(10**400)}},
            "Brave image height must fit a signed 64-bit integer",
        ),
        (
            "both_dimensions_overrange",
            "images",
            {"image": {"src": "https://i", "width": 10**400, "height": -(10**400)}},
            "Brave image width must fit a signed 64-bit integer",
        ),
    ]
    for name, category, item, diagnostic in variants:
        for mixed in [False, True]:
            items = [{"title": "boundary", **item}]
            if mixed:
                items = [{"title": "before"}] + items + [{"title": "after"}]
            record = await capture(
                f"brave.media_boundary.{name}.{'mixed' if mixed else 'single'}",
                "brave",
                {"api_key": "offline-fake-token"},
                "edge",
                {"categories": [category]},
                [response({"results": items})],
            )
            record["native_expectation"] = {
                "status": "error",
                "error_message": diagnostic,
                "results": [],
                "answers": [],
                "corrections": [],
                "infoboxes": [],
            }
            records.append(record)
    for mixed in [False, True]:
        posts = [{"data": {"title": "boundary", "selftext": ["source", "list"]}}]
        if mixed:
            posts = [{"data": {"title": "before"}}] + posts + [{"data": {"title": "after"}}]
        record = await capture(
            f"reddit.content_boundary.list.{'mixed' if mixed else 'single'}",
            "reddit",
            {},
            "edge",
            None,
            [response({"data": {"children": posts}})],
        )
        record["native_expectation"] = {
            "status": "error",
            "error_message": "Reddit selftext must be a string when nonempty",
            "results": [],
            "answers": [],
            "corrections": [],
            "infoboxes": [],
        }
        records.append(record)
    field_variants = []
    invalid = [None, False, 0, [], {}, 1, ["x"], {"name": "x"}]
    for field, key in [("url", "url"), ("title", "title"), ("content", "description")]:
        for index, value in enumerate(invalid):
            field_variants.append((f"brave_{field}_{index}", "brave", None, field, {key: value}))
    for index, value in enumerate(invalid[1:]):
        field_variants.append((f"brave_thumbnail_{index}", "brave", None, "thumbnail", {"thumbnail": {"src": value}}))
        field_variants.append(
            (
                f"wiki_thumbnail_{index}",
                "wikipedia",
                None,
                "thumbnail",
                {"title": "boundary", "thumbnail": {"source": value}},
            )
        )
    for index, value in enumerate([True, 1, ["x"], {"name": "x"}]):
        field_variants.append((f"brave_media_url_{index}", "brave", "images", "media.url", {"image": {"src": value}}))
        field_variants.append((f"reddit_title_{index}", "reddit", None, "title", {"title": value}))
        field_variants.append(
            (f"reddit_thumbnail_{index}", "reddit", None, "thumbnail", {"title": "boundary", "thumbnail": value})
        )
    field_variants += [
        ("brave_image_url", "brave", "images", "url", {"page_url": 1}),
        ("brave_image_title", "brave", "images", "title", {"title": 1}),
        ("brave_image_content_null", "brave", "images", "content", {"description": None}),
        (
            "brave_video_thumbnail_false",
            "brave",
            "videos",
            "thumbnail",
            {"thumbnail": {"src": False}, "duration": True},
        ),
        ("brave_first_field", "brave", None, "url", {"url": 1, "title": 1, "description": None}),
        (
            "reddit_first_field",
            "reddit",
            None,
            "title",
            {"title": True, "selftext": ["x"], "score": 1, "thumbnail": 1},
        ),
    ]
    for name, engine, category, field, item in field_variants:
        for mixed in [False, True]:
            items = [item]
            if mixed:
                items = [{"title": "before"}] + items + [{"title": "after"}]
            if engine == "brave":
                body = {"results": items} if category else {"web": {"results": items}}
                replies = [response(body)]
            elif engine == "wikipedia":
                replies = [
                    response(["edge", ["edge"]]),
                    response({"query": {"pages": {str(i): page for i, page in enumerate(items)}}}),
                ]
            else:
                replies = [response({"data": {"children": [{"data": post} for post in items]}})]
            optional = field in ["thumbnail", "media.url"]
            diagnostic = (
                f"{dict(brave='Brave', wikipedia='Wikipedia', reddit='Reddit')[engine]} result {field} must be a string"
                + (" or null" if optional else "")
            )
            record = await capture(
                f"typed.{name}.{'mixed' if mixed else 'single'}",
                engine,
                {"api_key": "offline-fake-token"} if engine == "brave" else {},
                "edge",
                {"categories": [category]} if category else None,
                replies,
            )
            record["boundary_field"] = field
            record["native_expectation"] = {
                "status": "error",
                "error_message": diagnostic,
                "results": [],
                "answers": [],
                "corrections": [],
                "infoboxes": [],
            }
            records.append(record)
    Path(output).write_text(
        json.dumps({"schema_version": 1, "plan_id": "A04", "records": records}, ensure_ascii=False, indent=2) + "\n"
    )


def source_tree(value):
    """Flat fully typed tree; strings use UTF8 surrogatepass base64.

    Node references prevent source depth probes from exceeding the separate
    finite public JSON fixture reader's depth limit. Tags never replace data.
    """
    nodes = []

    def node(value):
        index = len(nodes)
        nodes.append(None)
        if value is None:
            encoded = ["none"]
        elif isinstance(value, bool):
            encoded = ["bool", value]
        elif isinstance(value, int):
            encoded = ["int", str(value)]
        elif isinstance(value, float):
            encoded = ["float", repr(value)]
        elif isinstance(value, str):
            encoded = ["str", base64.b64encode(value.encode("utf-8", "surrogatepass")).decode()]
        elif isinstance(value, list):
            encoded = ["list", [node(v) for v in value]]
        elif isinstance(value, dict):
            encoded = ["dict", [[node(k), node(v)] for k, v in value.items()]]
        else:
            raise TypeError(type(value).__name__)
        nodes[index] = encoded
        return index

    return {"root": node(value), "nodes": nodes}


async def capture_source_json(output):
    """Observe actual JSON decoding and whole source searches, entirely offline."""
    from pathlib import Path

    records = []
    good = {"title": "X", "url": "https://x.test", "description": "d"}
    post = {"title": "X", "url": "https://x.test", "permalink": "/x"}
    suggestion = ["edge", ["edge"], [], []]
    rich = {"query": {"pages": {"1": {"title": "X", "extract": "d"}}}}

    def packed(value):
        return json.dumps(value, ensure_ascii=True, separators=(",", ":")).encode()

    def empty_native(error):
        return {
            "status": "error",
            "error_message": error,
            "results": [],
            "answers": [],
            "corrections": [],
            "infoboxes": [],
        }

    async def add(name, engine, bodies, category=None, native_error=None, recipe=None):
        events = []
        original = httpx.Response.json

        def observed_json(reply, *args, **kwargs):
            try:
                value = original(reply, *args, **kwargs)
            except Exception as exc:
                events.append({"exception_class": type(exc).__name__, "message": str(exc)})
                raise
            events.append({"value": source_tree(value)})
            return value

        with patch.object(httpx.Response, "json", observed_json):
            record = await capture(
                "json." + name,
                engine,
                {"api_key": "offline-fake-token"} if engine == "brave" else {},
                "edge",
                {"categories": [category]} if category else None,
                [response(body) for body in bodies],
            )
        actual = record.pop("output")
        record["source_output"] = source_tree(actual)
        record["decoder_events"] = events
        record["native_expectation"] = empty_native(native_error) if native_error else actual
        record["native_delta"] = native_error
        if recipe:
            record["responses"][0].pop("body_b64")
            record["responses"][0]["body_recipe"] = recipe
        records.append(record)

    # Every decoder stage receives every Python byte-encoding form.
    documents = {
        "brave": packed({"web": {"results": [good]}}),
        "reddit": packed({"data": {"children": [{"data": post}]}}),
        "wiki_first": packed(suggestion),
        "wiki_rich": packed(rich),
    }
    encodings = [("utf8", "utf-8", b""), ("utf8_bom", "utf-8", b"\xef\xbb\xbf")]
    for bits in (16, 32):
        for order in ("le", "be"):
            codec = f"utf-{bits}-{order}"
            bom = {
                "utf-16-le": b"\xff\xfe",
                "utf-16-be": b"\xfe\xff",
                "utf-32-le": b"\xff\xfe\x00\x00",
                "utf-32-be": b"\x00\x00\xfe\xff",
            }[codec]
            encodings.extend([(codec.replace("-", ""), codec, b""), (codec.replace("-", "") + "_bom", codec, bom)])
    for stage, doc in documents.items():
        for name, codec, bom in encodings:
            encoded = bom + doc.decode().encode(codec)
            engine = "wikipedia" if stage.startswith("wiki") else stage
            bodies = [packed(suggestion), encoded] if stage == "wiki_rich" else [encoded]
            if stage == "wiki_first":
                bodies.append(packed(rich))
            await add(stage + ".encoding." + name, engine, bodies)

    malformed = {
        "object_key": b"{not-json",
        "trailing_comma": b"[1,]",
        "leading_zero": b"01",
        "fraction": b"1.",
        "exponent": b"1e+",
        "empty": b"",
        "zero2": b"\x00\x00",
        "utf8_overlong": b'"\xc0\x80"',
        "utf8_out_of_range": b'"\xf4\x90\x80\x80"',
        "utf16_truncated": b"\xff\xfe{\x00x",
        "utf32_truncated": b"\x00\x00\xfe\xff\x00",
        "escape": b'"\\uQ000"',
        "control": b'"\x01"',
        "constant_suffix": b"Infinityx",
    }
    for stage in documents:
        for name, body in malformed.items():
            engine = "wikipedia" if stage.startswith("wiki") else stage
            bodies = [packed(suggestion), body] if stage == "wiki_rich" else [body]
            await add(
                stage + ".malformed." + name,
                engine,
                bodies,
                native_error="Brave returned invalid JSON" if engine == "brave" else None,
            )

    # Recursive opaque floats round at decoding; source integers remain exact.
    values = [
        "1.234567890123456789",
        "1e3",
        "1e-7",
        "-0.0",
        "1e-9999",
        "-1e-9999",
        "999999999999999999999999999999999999999999999999999999",
    ]
    for literal in values:
        nested = '{"nested":[{"number":' + literal + '}],"type":"float","value":"Infinity"}'
        await add(
            "brave.recursive." + literal,
            "brave",
            [('{"infobox":{"results":[{"description":[' + nested + "]}]}}").encode()],
        )
        await add(
            "wiki.recursive." + literal,
            "wikipedia",
            [
                packed(suggestion),
                ('{"query":{"pages":{"1":{"title":"X","pageprops":{"wikibase-shortdesc":' + nested + "}}}}}").encode(),
            ],
        )

    for literal in ("1e309", "-1e309", "NaN", "Infinity", "-Infinity"):
        for engine in ("brave", "reddit", "wikipedia"):
            body = ('{"unused":' + literal + "}").encode()
            if engine == "reddit":
                body = (
                    '{"data":{"children":[{"data":{"over_18":true,"title":"X","score":' + literal + "}}]}}"
                ).encode()
            bodies = (
                [packed(suggestion), ('{"query":{"pages":{"1":{"title":"X","unused":' + literal + "}}}}").encode()]
                if engine == "wikipedia"
                else [body]
            )
            await add(engine + ".unused." + literal, engine, bodies)
        answer = ('{"infobox":{"results":[{"description":[' + literal + "]}]}}").encode()
        await add(
            "brave.opaque." + literal,
            "brave",
            [answer],
            native_error="Brave answer must contain only finite JSON numbers",
        )
        await add("brave.opaque_later_source_error." + literal, "brave", [answer[:-1] + b',"mixed":{"main":null}}'])
        wiki = ('{"query":{"pages":{"1":{"title":"X","pageprops":{"wikibase-shortdesc":[' + literal + "]}}}}}").encode()
        await add(
            "wiki.opaque." + literal,
            "wikipedia",
            [packed(suggestion), wiki],
            native_error="Wikipedia infobox must contain only finite JSON numbers",
        )
        await add(
            "wiki.opaque_later_source_error." + literal, "wikipedia", [packed(suggestion), wiki[:-3] + b',"2":null}}}']
        )
        score = ('{"data":{"children":[{"data":{"title":"X","score":' + literal + "}}]}}").encode()
        await add("reddit.score." + literal, "reddit", [score], native_error="Reddit score must be finite")
        date = ('{"data":{"children":[{"data":{"title":"X","created_utc":' + literal + "}}]}}").encode()
        await add("reddit.date." + literal, "reddit", [date])

    # The source decoder applies its digit cap to integers, not float lexemes.
    for name, literal in (
        ("positive4300", "9" * 4300),
        ("negative4300", "-" + "9" * 4300),
        ("positive4301", "9" * 4301),
        ("negative4301", "-" + "9" * 4301),
        ("leading_zero", "0" + "9" * 4301),
        ("exponent4301", "9" * 4301 + "e0"),
    ):
        for engine in ("brave", "reddit", "wikipedia"):
            body = ('{"unused":' + literal + "}").encode()
            if engine == "reddit":
                body = ('{"data":{"children":[]},"unused":' + literal + "}").encode()
            bodies = (
                [packed(suggestion), ('{"query":{"pages":{}},"unused":' + literal + "}").encode()]
                if engine == "wikipedia"
                else [body]
            )
            run = b"9" * (4300 if name.endswith("4300") else 4301)
            target = bodies[-1]
            at = target.index(run)
            recipe = {
                "kind": "repeat_number",
                "prefix_b64": base64.b64encode(target[:at]).decode(),
                "count": len(run),
                "suffix_b64": base64.b64encode(target[at + len(run) :]).decode(),
            }
            await add(
                engine + ".digits." + name,
                engine,
                bodies,
                native_error="Brave returned invalid JSON" if engine == "brave" and name == "leading_zero" else None,
            )
            records[-1]["responses"][-1].pop("body_b64")
            records[-1]["responses"][-1]["body_recipe"] = recipe

    unicode_cases = [
        ("brave.unused_surrogate", "brave", [{"unused": "\ud800"}], None, None),
        (
            "brave.title_surrogate",
            "brave",
            [{"web": {"results": [{**good, "title": "\ud800"}]}}],
            None,
            "Brave result title must contain only Unicode scalar values",
        ),
        (
            "brave.answer_surrogate",
            "brave",
            [{"infobox": {"results": [{"description": ["\ud800"]}]}}],
            None,
            "Brave answer must contain only Unicode scalar strings",
        ),
        (
            "brave.answer_surrogate_later_error",
            "brave",
            [{"infobox": {"results": [{"description": ["\ud800"]}]}, "mixed": {"main": None}}],
            None,
            None,
        ),
        ("wiki.request_surrogate", "wikipedia", [["edge", ["\ud800"]]], None, None),
        ("wiki.request_surrogate_pair", "wikipedia", [["edge", ["a\ud800\udfff"]], rich], None, None),
        (
            "wiki.extract_surrogate",
            "wikipedia",
            [suggestion, {"query": {"pages": {"1": {"title": "X", "extract": "\ud800"}}}}],
            None,
            "Wikipedia result content must contain only Unicode scalar values",
        ),
        (
            "wiki.extract_surrogate_truncated",
            "wikipedia",
            [suggestion, {"query": {"pages": {"1": {"title": "X", "extract": "x" * 200 + "\ud800"}}}}],
            None,
            None,
        ),
        (
            "wiki.infobox_key_surrogate",
            "wikipedia",
            [
                suggestion,
                {"query": {"pages": {"1": {"title": "X", "pageprops": {"wikibase-shortdesc": {"\ud800": "x"}}}}}},
            ],
            None,
            "Wikipedia infobox must contain only Unicode scalar strings",
        ),
        (
            "reddit.author_surrogate",
            "reddit",
            [{"data": {"children": [{"data": {**post, "author": "\ud800"}}]}}],
            None,
            "Reddit result content must contain only Unicode scalar values",
        ),
        (
            "reddit.author_surrogate_repr",
            "reddit",
            [{"data": {"children": [{"data": {**post, "author": ["\ud800", {"\udfff": "x"}]}}]}}],
            None,
            None,
        ),
        (
            "reddit.selftext_surrogate_truncated",
            "reddit",
            [{"data": {"children": [{"data": {**post, "selftext": "x" * 500 + "\ud800"}}]}}],
            None,
            None,
        ),
        (
            "reddit.url_surrogate",
            "reddit",
            [{"data": {"children": [{"data": {**post, "url": "https://x.test/\ud800"}}]}}],
            None,
            "Reddit result url must contain only Unicode scalar values",
        ),
        (
            "reddit.score_surrogate",
            "reddit",
            [{"data": {"children": [{"data": {**post, "score": "\ud800"}}]}}],
            None,
            None,
        ),
    ]
    for name, engine, bodies, category, error in unicode_cases:
        await add(name, engine, [packed(body) for body in bodies], category, error)
    for name, body in (
        ("literal_utf8_surrogate", b'{"unused":"\xed\xa0\x80"}'),
        ("escaped_pair", b'{"web":{"results":[{"title":"\\ud800\\udc00"}]}}'),
        ("literal_utf8_pair", b'{"unused":"\xed\xa0\x80\xed\xb0\x80"}'),
    ):
        await add("brave." + name, "brave", [body])

    urls = [
        "https://x.test/a?token=\ud800&ok=1",
        "https://x.test/a?token=first&token=\ud800&ok=1",
        "https://x.test/a?%74oken=\ud800&ok=1#frag",
        "https://x.test/a?token=\ud800;ok=1",
        "https://x.test/a?token=\ud800&&ok=1&",
        "https://x.test/a?token=\ud800#frag",
        "https://x.test/a?token=\ud800#\ud800",
        "https://x.test/a?ok=\ud800&token=x",
        "https://x.test/a?\ud800=x&token=secret",
        "https://[bad/a?token=\ud800&ok=1",
        "https://x.test:bad/a?token=\ud800&ok=1",
        "https://x.test:99999/a?token=\ud800&ok=1",
        "\x00\nhttps://x.test/a?token=\ud800&ok=1",
        "https://x.test/\ud800?token=secret",
        "https://\ud800.test/a?token=secret",
        "https://x.test/a?token=\ud800&ok=%ED%A0%80",
        "https://x.test/a?token=\ud800&ok=1#",
        "https://x.test/a?token=\ud800&token=&ok=",
        "https://x.test/a?TOKEN=\ud800&ok=1",
    ]
    refused = {6, 7, 8, 9, 13, 14, 18}
    for index, url in enumerate(urls):
        await add(
            "brave.sanitizer." + str(index),
            "brave",
            [packed({"results": [{"image": {"src": url}}]})],
            "images",
            "Brave result media.url must contain only Unicode scalar values" if index in refused else None,
        )

    for literal in ("5e-324", "2.2250738585072014e-308", "1.7976931348623157e308", "9007199254740993.0"):
        body = ('{"infobox":{"results":[{"description":[{"n":' + literal + "}]}]}}").encode()
        await add("brave.float_extreme." + literal, "brave", [body])
    markers = [
        {"$serde_json::private::Number": "NaN"},
        {"type": "float", "value": "inf"},
        {"__source_number__": "1e309"},
        {"root": 0, "nodes": [["float", "nan"]]},
    ]
    await add("brave.marker_objects", "brave", [packed({"infobox": {"results": [{"description": markers}]}})])
    await add(
        "wiki.marker_objects",
        "wikipedia",
        [
            packed(suggestion),
            packed({"query": {"pages": {"1": {"title": "X", "pageprops": {"wikibase-shortdesc": markers}}}}}),
        ],
    )
    await add(
        "brave.original_error_before_opaque",
        "brave",
        [packed({"web": {"results": [None]}, "infobox": {"results": [{"description": [float("inf")]}]}})],
    )
    await add(
        "wiki.original_error_before_opaque",
        "wikipedia",
        [
            packed(suggestion),
            packed(
                {
                    "query": {
                        "pages": {"0": None, "1": {"title": "X", "pageprops": {"wikibase-shortdesc": [float("inf")]}}}
                    }
                }
            ),
        ],
    )
    for order in ("before", "after"):
        bad = {**post, "author": "\ud800"}
        error = {**post, "score": {}}
        children = [{"data": bad}, {"data": error}] if order == "after" else [{"data": error}, {"data": bad}]
        await add("reddit.unicode_original_error_" + order, "reddit", [packed({"data": {"children": children}})])
    for field, value, diagnostic in (
        ("selftext", "\ud800", "Reddit result content must contain only Unicode scalar values"),
        ("subreddit", "\ud800", "Reddit result category must contain only Unicode scalar values"),
        ("thumbnail", "\ud800", "Reddit result thumbnail must contain only Unicode scalar values"),
    ):
        await add(
            "reddit.unicode_field." + field,
            "reddit",
            [packed({"data": {"children": [{"data": {**post, field: value}}]}})],
            native_error=diagnostic,
        )
    for codec in ("utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be"):
        for field in ("unused", "title"):
            value = {"unused": "\ud800"} if field == "unused" else {"web": {"results": [{**good, "title": "\ud800"}]}}
            body = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode(codec, "surrogatepass")
            await add(
                "brave.raw_surrogate." + codec + "." + field,
                "brave",
                [body],
                native_error="Brave result title must contain only Unicode scalar values" if field == "title" else None,
            )

    for size in (4194304, 4194305):
        await add(
            "brave.byte_limit." + str(size),
            "brave",
            [b" " * (size - 2) + b"{}"],
            native_error="Brave response exceeds JSON byte limit (4194304 bytes)" if size > 4194304 else None,
            recipe={"kind": "padding_object", "bytes": size},
        )
    for depth in (100, 101):
        body = b'{"unused":' + b"[" * (depth - 1) + b"0" + b"]" * (depth - 1) + b"}"
        await add(
            "brave.depth_limit." + str(depth),
            "brave",
            [body],
            native_error="Brave response exceeds JSON depth limit (100 levels)" if depth > 100 else None,
        )

    for index, url in enumerate(
        ("?token=\ud800", "//?token=\ud800", "?token=\ud800#frag", "scheme:?token=\ud800", "/?token=\ud800")
    ):
        await add(
            "brave.sanitizer_relative." + str(index),
            "brave",
            [packed({"results": [{"image": {"src": url}}]})],
            "images",
        )

    Path(output).write_text(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A04",
                "observation_encoding": "flat_fully_typed_python_tree_utf8_surrogatepass_base64",
                "records": records,
            },
            ensure_ascii=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    )


parser = argparse.ArgumentParser()
parser.add_argument("--nonfinite-output")
parser.add_argument("--representation-output")
parser.add_argument("--source-json-output")
args = parser.parse_args()
if args.source_json_output:
    asyncio.run(capture_source_json(args.source_json_output))
elif args.nonfinite_output:
    asyncio.run(capture_nonfinite(args.nonfinite_output))
elif args.representation_output:
    asyncio.run(capture_representation_boundaries(args.representation_output))
else:
    asyncio.run(main())
