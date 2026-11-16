import argparse
import asyncio
import base64
import dataclasses
import json
from unittest.mock import patch

import httpx

from engines.brave import BraveAdapter
from engines.reddit import RedditAdapter
from engines.wikipedia import WikipediaAdapter
from tests.test_adapters import TestBraveAdapter, TestWikipediaAdapter
from tests.test_reddit import empty_listing, sample_listing

CONTEXT = []


def plain_result(r):
    return {
        "url": r.url,
        "title": r.title,
        "content": r.content,
        "engine": r.engine,
        "score": r.score,
        "position": r.position,
        "category": r.category,
        "published_date": r.published_date,
        "thumbnail": r.thumbnail,
        "img_src": r.img_src,
        "media": dataclasses.asdict(r.media) if r.media else None,
        "payload": r.payload,
    }


def plain_response(r):
    return {
        "status": r.status.value,
        "error_message": r.error_message,
        "results": [plain_result(x) for x in r.results],
        "answers": r.answers,
        "corrections": r.corrections,
        "infoboxes": r.infoboxes,
    }


async def capture(name, adapter, query, responses, params=None):
    seen = []
    bodies = [
        x.content
        if isinstance(x, httpx.Response)
        else (
            x
            if isinstance(x, bytes)
            else (
                b""
                if isinstance(x, BaseException)
                else json.dumps(x, separators=(",", ":"), ensure_ascii=False).encode()
            )
        )
        for x in responses
    ]
    n = 0

    def handler(req):
        nonlocal n
        seen.append(
            {
                "method": req.method,
                "url": str(req.url),
                "headers": {
                    k.lower(): v
                    for k, v in req.headers.items()
                    if k.lower() in ("accept", "user-agent", "authorization", "x-subscription-token")
                },
            }
        )
        item = responses[min(n, len(responses) - 1)]
        body = bodies[min(n, len(bodies) - 1)]
        n += 1
        if isinstance(item, BaseException):
            raise item
        return httpx.Response(
            item.status_code if isinstance(item, httpx.Response) else 200,
            content=body,
            headers={"content-type": "application/json"},
        )

    async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client:
        with patch("httpx.AsyncClient") as mocked:
            mocked.return_value.__aenter__.return_value = client
            result = await adapter.search(query, params)
    CONTEXT.append(
        {
            "id": name,
            "engine": adapter.name,
            "config": dict(adapter.config),
            "query": query,
            "params": params,
            "requests": seen,
            "responses": [
                {
                    "status": x.status_code
                    if isinstance(x, httpx.Response)
                    else (None if isinstance(x, BaseException) else 200),
                    "headers": {"content-type": "application/json"},
                    "body_b64": base64.b64encode(body).decode(),
                    "exception_type": type(x).__name__ if isinstance(x, BaseException) else None,
                }
                for x, body in zip(responses, bodies)
            ],
        }
    )
    return {
        "id": name,
        "query": query,
        "params": params,
        "request": seen,
        "responses": [
            {"body_b64": base64.b64encode(b).decode(), "body_utf8": b.decode("utf-8", errors="replace")} for b in bodies
        ],
        "output": plain_response(result),
    }


async def main():
    tb = TestBraveAdapter()
    tw = TestWikipediaAdapter()
    bweb = tb.sample_response.__wrapped__(tb)
    bimg = tb.sample_image_response.__wrapped__(tb)
    bnews = tb.sample_news_response.__wrapped__(tb)
    op = tw.opensearch_response.__wrapped__(tw)
    rich = tw.rich_query_response.__wrapped__(tw)
    records = []
    records += [
        await capture("brave.web.success", BraveAdapter({"api_key": "test-key"}), "test query", [bweb]),
        await capture(
            "brave.images.media",
            BraveAdapter({"api_key": "test-key"}),
            "raspberry pi",
            [bimg],
            {"categories": ["images"]},
        ),
        await capture(
            "brave.news.route", BraveAdapter({"api_key": "test-key"}), "breaking", [bnews], {"categories": ["news"]}
        ),
        await capture(
            "brave.videos.route", BraveAdapter({"api_key": "test-key"}), "tutorial", [bnews], {"categories": ["videos"]}
        ),
        await capture(
            "brave.web.fallback", BraveAdapter({"api_key": "test-key"}), "science", [bweb], {"categories": ["science"]}
        ),
        await capture("brave.429", BraveAdapter({"api_key": "test-key"}), "test", [httpx.Response(429)]),
        await capture("brave.403", BraveAdapter({"api_key": "test-key"}), "test", [httpx.Response(403)]),
        await capture(
            "brave.timeout", BraveAdapter({"api_key": "test-key"}), "test", [httpx.TimeoutException("timeout")]
        ),
        await capture("brave.missing_key", BraveAdapter({"api_key": ""}), "test", []),
    ]
    answer = {
        "infobox": {
            "results": [
                {"url": "https://example.test/entity", "title": "Entity", "description": "An entity description"}
            ]
        },
        "mixed": {
            "main": [
                {
                    "type": "answer",
                    "data": {
                        "url": "https://example.test/answer",
                        "title": "Answer",
                        "description": "An inline answer",
                    },
                }
            ]
        },
    }
    image = {
        "results": [
            {
                "title": "image",
                "page_url": "https://page.test/image",
                "thumbnail": {"src": "https://thumb.test/image.jpg"},
                "image": {"src": "https://cdn.test/image.jpg", "width": 640, "height": 480},
            }
        ]
    }
    video = {
        "results": [
            {
                "title": "video",
                "url": "https://page.test/video",
                "description": "clip",
                "thumbnail": {"src": "https://thumb.test/video.jpg"},
                "duration": 12.5,
            },
            {"title": "no media", "url": "https://page.test/no-media", "description": "text"},
        ]
    }
    records += [
        await capture("brave.answers", BraveAdapter({"api_key": "test-key"}), "entity", [answer]),
        await capture(
            "brave.image.full_media",
            BraveAdapter({"api_key": "test-key"}),
            "image",
            [image],
            {"categories": ["images"]},
        ),
        await capture(
            "brave.video.media", BraveAdapter({"api_key": "test-key"}), "video", [video], {"categories": ["videos"]}
        ),
    ]
    records += [
        await capture("wikipedia.two_stage", WikipediaAdapter(), "test query", [op, rich]),
        await capture("wikipedia.empty_titles", WikipediaAdapter(), "test query", [["test query", [], [], []]]),
        await capture(
            "wikipedia.missing_page",
            WikipediaAdapter(),
            "test query",
            [op, {"batchcomplete": "", "query": {"pages": {"-1": {"missing": "", "title": "Missing"}}}}],
        ),
        await capture("wikipedia.429", WikipediaAdapter(), "test", [httpx.Response(429)]),
    ]
    records.append(
        await capture(
            "wikipedia.contact_ua",
            WikipediaAdapter({"user_agent": "Project contact https://example.test/contact"}),
            "test query",
            [op, rich],
        )
    )
    records += [
        await capture(
            "reddit.empty_permalink_prefix",
            RedditAdapter(),
            "test query",
            [
                {
                    "data": {
                        "children": [
                            {
                                "kind": "t3",
                                "data": {
                                    "title": "Link",
                                    "url": "https://example.test/link",
                                    "selftext": "",
                                    "score": 1,
                                    "num_comments": 0,
                                    "author": "author",
                                    "subreddit": "python",
                                    "created_utc": 1700000000,
                                    "over_18": False,
                                    "thumbnail": "",
                                },
                            }
                        ]
                    }
                }
            ],
        ),
        await capture(
            "reddit.truthy_nsfw",
            RedditAdapter(),
            "test query",
            [
                {
                    "data": {
                        "children": [
                            {
                                "kind": "t3",
                                "data": {
                                    "title": "Numeric NSFW",
                                    "url": "https://example.test/nsfw",
                                    "selftext": "",
                                    "score": 1,
                                    "num_comments": 0,
                                    "author": "author",
                                    "subreddit": "python",
                                    "created_utc": 1700000000,
                                    "over_18": 1,
                                    "thumbnail": "",
                                },
                            }
                        ]
                    }
                }
            ],
        ),
        await capture("reddit.listing", RedditAdapter(), "test query", [sample_listing.__wrapped__()]),
        await capture("reddit.empty", RedditAdapter(), "test query", [empty_listing.__wrapped__()]),
        await capture(
            "reddit.oauth", RedditAdapter({"api_key": "oauth-token"}), "test query", [{"data": {"children": []}}]
        ),
        await capture(
            "reddit.oauth.custom",
            RedditAdapter({"base_url": "https://reddit.example.test/api", "api_key": "oauth-token"}),
            "test query",
            [{"data": {"children": []}}],
        ),
        await capture("reddit.429", RedditAdapter(), "test query", [httpx.Response(429)]),
        await capture("reddit.403", RedditAdapter(), "test query", [httpx.Response(403)]),
        await capture(
            "reddit.oauth.403", RedditAdapter({"api_key": "oauth-token"}), "test query", [httpx.Response(403)]
        ),
        await capture(
            "reddit.oauth.401", RedditAdapter({"api_key": "oauth-token"}), "test query", [httpx.Response(401)]
        ),
        await capture("reddit.500", RedditAdapter(), "test query", [httpx.Response(500)]),
        await capture("reddit.malformed_json", RedditAdapter(), "test query", [b"{not-json"]),
        await capture("reddit.malformed_children", RedditAdapter(), "test query", [{"data": {"children": {}}}]),
    ]
    for case_id, params in [
        ("reddit.subreddit", {"categories": ["reddit:subreddit"], "subreddit": "python"}),
        ("reddit.subreddit_missing", {"categories": ["reddit:subreddit"]}),
        ("reddit.subreddit_empty", {"categories": ["reddit:subreddit"], "subreddit": ""}),
        ("reddit.subreddit_null", {"categories": ["reddit:subreddit"], "subreddit": None}),
        ("reddit.subreddit_number", {"categories": ["reddit:subreddit"], "subreddit": 7}),
        ("reddit.subreddit_path_special", {"categories": ["reddit:subreddit"], "subreddit": "foo/bar"}),
        ("reddit.general_ignores_subreddit", {"categories": ["general"], "subreddit": "python"}),
    ]:
        records.append(await capture(case_id, RedditAdapter(), "test query", [{"data": {"children": []}}], params))
    for r in records:
        r["captured_by"] = "offline Python 3.14.7 adapter invocation via httpx.MockTransport"
    args = argparse.ArgumentParser()
    args.add_argument("--context-output")
    opts = args.parse_args()
    if opts.context_output:
        with open(opts.context_output, "w", encoding="utf-8") as out:
            json.dump({"schema_version": 1, "plan_id": "A04", "records": CONTEXT}, out, ensure_ascii=False, indent=2)
            out.write("\n")
    print(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A04",
                "provenance": (
                    "Offline Python 3.14.7 source observations from actual adapter.search "
                    "invocations through httpx.MockTransport. No provider calls; fake credentials only."
                ),
                "records": records,
            },
            ensure_ascii=False,
            indent=2,
        )
    )


asyncio.run(main())
