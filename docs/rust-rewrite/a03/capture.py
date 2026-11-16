"""A03 isolated Python reference oracle. MockTransport only; retire under G08."""

import argparse
import asyncio
import copy
import dataclasses
import importlib.metadata
import json
import math
import subprocess
import sys
from pathlib import Path

import httpx

from engines.github import GitHubAdapter
from engines.hackernews import HackerNewsAdapter
from engines.huggingface import HuggingFaceAdapter
from engines.stackexchange import StackExchangeAdapter

BASELINE = "35d97ccf8c9b2d8246373b2024cc8d7a457705a3"
ENGINES = {
    "github": GitHubAdapter,
    "huggingface": HuggingFaceAdapter,
    "stackexchange": StackExchangeAdapter,
    "hackernews": HackerNewsAdapter,
}
SOURCES = [
    "engines/github.py",
    "engines/huggingface.py",
    "engines/stackexchange.py",
    "engines/hackernews.py",
    "slopsearx/adapter.py",
    "slopsearx/payload.py",
    "slopsearx/config.py",
    "tests/test_github.py",
    "tests/test_stackexchange_adapter.py",
    "tests/test_adapters.py",
    "tests/test_tier1_adapters.py",
    "tests/test_config.py",
    "tests/test_capabilities.py",
    "uv.lock",
]


def scenarios():
    rows = []

    def add(engine, name, body=None, *, config=None, categories=None, status=200, raw=None, fault=None, denied=False):
        rows.append(
            {
                "case": engine + ":" + name,
                "engine": engine,
                "config": config or {},
                "query": "café / 🐙 & x+y?",
                "params": {
                    "categories": categories or [],
                    "page": 7,
                    "pageno": 7,
                    "language": "fr",
                    "safesearch": 2,
                    "time_range": "week",
                    "media_type": "image",
                    "max_results": 1,
                },
                "upstream": {
                    "status": status,
                    "body": raw if raw is not None else json.dumps(body, ensure_ascii=False),
                    "fault": fault,
                },
                "admission_denied": denied,
            }
        )

    repo = {
        "html_url": "https://github.com/octo/repo",
        "full_name": "octo/repo",
        "name": "repo",
        "description": "A repository",
        "language": "Rust",
        "stargazers_count": 42,
        "topics": ["search", "api"],
    }
    issue = {
        "html_url": "https://github.com/octo/repo/issues/1",
        "title": "A bug",
        "state": "open",
        "body": " \x1c  café 🐙 " + "x" * 305 + "\x1f",
        "labels": [{"name": "bug"}, "ignored", {"name": "help"}],
        "created_at": "2024-01-01T00:00:00Z",
    }
    code = {
        "repository": {"full_name": "octo/repo"},
        "path": "src/lib.rs",
        "html_url": "https://ignored.example/branch",
        "text_matches": [{"fragment": "snippet 🐙 " + "x" * 310}, {"fragment": "ignored second"}],
    }
    add("github", "repositories", {"items": [repo, {**repo, "full_name": "second/repo"}]}, config={"max_results": 1})
    add("github", "issues", {"items": [issue]}, categories=["github:issues"])
    add("github", "prs", {"items": [issue]}, categories=["github:prs"])
    add(
        "github",
        "code-precedence",
        {"items": [code]},
        categories=["github:issues", "github:code"],
        config={"api_key": "fixture-token"},
    )
    add("github", "code-missing-token", {"items": []}, categories=["github:code"])
    add(
        "github",
        "code-whitespace-token",
        {"items": []},
        categories=["github:code"],
        config={"api_key": " \x1c\u2003\x1f "},
    )
    add("github", "python-strip-token", {"items": []}, config={"api_key": "\x1c\u2003fixture-token\x1f"})
    add("github", "custom-base-query", {"items": [repo]}, config={"base_url": "https://fixture.invalid/prefix?old=yes"})
    add(
        "github",
        "base-endpoint-influences-parser",
        {"items": [code]},
        config={"base_url": "https://fixture.invalid/search/code", "api_key": "fixture-token"},
    )
    add(
        "github",
        "base-endpoint-influences-auth",
        {"items": []},
        config={"base_url": "https://fixture.invalid/search/code"},
    )
    for field, value in [
        ("full_name", None),
        ("html_url", False),
        ("language", 7),
        ("topics", [1]),
        ("description", ["nested", {"x": True}]),
    ]:
        add("github", "repo-" + field, {"items": [{**repo, field: value}]})
    for name, value in [
        ("bool", True),
        ("float", 1.2300),
        ("text", " \u0661_\u0662.\u0665 "),
        ("huge-int", 10**40 + 123),
        ("null", None),
        ("invalid", []),
        ("nonfinite", "nan"),
    ]:
        add("github", "stars-" + name, {"items": [{**repo, "stargazers_count": value}]})
    add("github", "missing-fields", {"items": [{}]})
    add(
        "github",
        "code-no-repo",
        {"items": [{"path": "file", "html_url": "https://example.invalid/file"}]},
        categories=["github:code"],
        config={"api_key": "fixture-token"},
    )
    add(
        "github",
        "code-null-path",
        {"items": [{**code, "path": None, "text_matches": []}]},
        categories=["github:code"],
        config={"api_key": "fixture-token"},
    )
    add(
        "github",
        "code-bad-repository",
        {"items": [{**code, "repository": [1]}]},
        categories=["github:code"],
        config={"api_key": "fixture-token"},
    )
    add("github", "issue-label-without-name", {"items": [{**issue, "labels": [{}]}]}, categories=["github:issues"])
    add(
        "github",
        "issue-empty-body-state-null",
        {"items": [{**issue, "body": None, "state": None, "labels": []}]},
        categories=["github:issues"],
    )
    add("github", "rate-limit-text", {}, status=403, raw="RATE LIMIT exceeded")
    for status in [401, 403, 422, 429, 500, 302]:
        add("github", "http-" + str(status), {}, status=status)

    model = {
        "modelId": "org/model",
        "id": "fallback",
        "pipeline_tag": "text-generation",
        "library_name": "transformers",
        "downloads": 500000,
        "likes": 1200,
        "description": "description 🐙 " + "x" * 210,
    }
    dataset = {"id": "org/dataset", "downloads": 77, "likes": 3, "description": "Dataset"}
    paper = {
        "id": "1234.5678",
        "title": "A paper",
        "upvotes": 12,
        "authors": [{"name": "First"}, {"name": "Ignored"}],
        "publishedAt": "2024-01-01",
    }
    add("huggingface", "models-unsliced", [model, {**model, "modelId": "org/second"}], config={"max_results": 1})
    add("huggingface", "datasets-precedence", [dataset], categories=["huggingface:papers", "huggingface:datasets"])
    add("huggingface", "papers", [paper], categories=["huggingface:papers"])
    add("huggingface", "optional-token", [], config={"api_key": "hf-fixture"})
    add("huggingface", "whitespace-token-not-stripped", [], config={"api_key": "   "})
    add("huggingface", "custom-base-query", [model], config={"base_url": "https://fixture.invalid/prefix?old=yes"})
    add(
        "huggingface",
        "endpoint-influences-parser",
        [model],
        categories=["huggingface:datasets"],
        config={"base_url": "https://fixture.invalid/models"},
    )
    for field, value in [
        ("modelId", None),
        ("pipeline_tag", 4),
        ("downloads", {"n": 2}),
        ("likes", None),
        ("description", None),
    ]:
        add("huggingface", "model-" + field, [{**model, field: value}])
    add("huggingface", "model-fallback-id", [{"id": "fallback"}])
    for name, value in [
        ("bool", True),
        ("float", 1.2300),
        ("text", " \u0661_\u0662.\u0665 "),
        ("nonfinite", "inf"),
        ("invalid", []),
    ]:
        add("huggingface", "likes-" + name, [{**model, "likes": value}])
    add("huggingface", "dataset-missing", [{}], categories=["huggingface:datasets"])
    add("huggingface", "dataset-null-id", [{**dataset, "id": None}], categories=["huggingface:datasets"])
    add("huggingface", "paper-fallback-url-title", [{"id": "123", "upvotes": True}], categories=["huggingface:papers"])
    add("huggingface", "paper-null-url", [{**paper, "url": None}], categories=["huggingface:papers"])
    add("huggingface", "paper-bad-first-author", [{**paper, "authors": [None]}], categories=["huggingface:papers"])
    add(
        "huggingface", "paper-numeric-author", [{**paper, "authors": [{"name": 42}]}], categories=["huggingface:papers"]
    )
    for status in [401, 403, 429, 500, 302]:
        add("huggingface", "http-" + str(status), [], status=status)

    hit = {
        "title": "A story",
        "url": "https://example.invalid/story",
        "objectID": "123",
        "points": 150,
        "author": "author",
        "created_at": "2024-01-01T00:00:00Z",
        "num_comments": 30,
    }
    add(
        "hackernews",
        "stories-unsliced",
        {"hits": [hit, {**hit, "url": None, "objectID": "456", "num_comments": 0}]},
        config={"max_results": 1},
    )
    add("hackernews", "missing-fields", {"hits": [{}]})
    add("hackernews", "custom-base-query", {"hits": []}, config={"base_url": "https://fixture.invalid/search?old=yes"})
    for field, value in [
        ("title", None),
        ("url", 5),
        ("objectID", {"x": True}),
        ("author", None),
        ("created_at", 42),
        ("num_comments", [1]),
    ]:
        add("hackernews", "hit-" + field, {"hits": [{**hit, field: value}]})
    for name, value in [
        ("bool", True),
        ("float", -0.0),
        ("text", " +1_000.5 "),
        ("huge-int", 10**40),
        ("null", None),
        ("invalid", {}),
        ("nonfinite", "nan"),
    ]:
        add("hackernews", "points-" + name, {"hits": [{**hit, "points": value}]})
    for status in [400, 401, 429, 500, 302]:
        add("hackernews", "http-" + str(status), {}, status=status)

    item = {
        "link": "https://stackoverflow.com/questions/123",
        "title": "Question",
        "body_markdown": "Body 🐙 " + "x" * 505,
        "score": 42,
        "creation_date": 1700000000,
    }
    add("stackexchange", "questions-sliced", {"items": [item, {**item, "title": "Second"}]}, config={"max_results": 1})
    add("stackexchange", "default-site", {"items": []})
    add(
        "stackexchange",
        "serverfault-first",
        {"items": []},
        categories=["stackexchange:serverfault", "stackexchange:code"],
    )
    add("stackexchange", "code-first", {"items": []}, categories=["stackexchange:code", "stackexchange:serverfault"])
    add("stackexchange", "header-key", {"items": []}, config={"api_key": "fixture-se-key"})
    add("stackexchange", "strip-key", {"items": []}, config={"api_key": "\x1c\u2003fixture-se-key\x1f"})
    add("stackexchange", "whitespace-key", {"items": []}, config={"api_key": "\x1c\u2003\x1f"})
    add(
        "stackexchange",
        "custom-base-query",
        {"items": []},
        config={"base_url": "https://fixture.invalid/prefix?old=yes"},
    )
    for status in [400, 401, 403, 429, 500, 302]:
        add("stackexchange", "http-" + str(status), {}, status=status, config={"api_key": "fixture-se-key"})
    for name, value in [
        ("none", None),
        ("zero", 0),
        ("bool", True),
        ("fraction", 1.25),
        ("negative-half", -0.0000005),
        ("second-half", 1.0000005),
        ("min-year", -62135596800),
        ("max-year", 253402300799),
        ("too-large", 10**40),
        ("text", "1"),
    ]:
        add("stackexchange", "timestamp-" + name, {"items": [{**item, "creation_date": value}]})
    for field, value in [
        ("link", None),
        ("title", None),
        ("body_markdown", None),
        ("score", None),
        ("score", True),
        ("score", " \u0661_\u0662.\u0665 "),
    ]:
        add("stackexchange", "field-" + field + "-" + str(type(value).__name__), {"items": [{**item, field: value}]})
    add("stackexchange", "missing-fields", {"items": [{}]})

    for engine in ENGINES:
        collection = "hits" if engine == "hackernews" else "items"
        for name, value in [
            ("empty", []),
            ("null", None),
            ("empty-object", {}),
            ("empty-text", ""),
            ("object", {"x": 1}),
            ("text", "x"),
            ("bad-row", [None]),
        ]:
            add(engine, "collection-" + name, value if engine == "huggingface" else {collection: value})
        if engine != "huggingface":
            add(engine, "missing-collection", {})
            add(engine, "wrong-root", [])
        add(engine, "malformed-json", raw="{bad json")
        add(engine, "timeout", fault="timeout")
        add(engine, "connect-error", fault="connect")
        add(engine, "caller-cancelled", fault="cancel")
        add(engine, "quota-denied", denied=True)
        add(engine, "empty-base-override", config={"base_url": ""})
        add(engine, "malformed-base", config={"base_url": "not a URL"})
        add(
            engine,
            "zero-results-setting",
            []
            if engine == "huggingface"
            else {collection: [item if engine == "stackexchange" else hit if engine == "hackernews" else repo]},
            config={"max_results": 0},
        )
        for name, value in [("negative", -1), ("float", 1.5), ("text", "2"), ("null", None), ("container", [1])]:
            add(engine, "max-results-" + name, [], config={"max_results": value})
        for name, value in [("zero", 0), ("fractional", 1.5), ("negative", -1), ("overflow", 1e300)]:
            add(engine, "timeout-setting-" + name, [], config={"timeout_ms": value})
    # Additive boundary evidence; original 218 observations remain unchanged.
    add("huggingface", "zero-limit-does-not-slice-models", [model], config={"max_results": 0})
    add("stackexchange", "zero-limit-does-not-inspect-bad-row", {"items": [None]}, config={"max_results": 0})
    add("stackexchange", "slice-does-not-inspect-bad-second-row", {"items": [item, None]}, config={"max_results": 1})
    add("github", "second-row-invalid-title", {"items": [repo, {**repo, "full_name": None}]})
    add("huggingface", "second-row-invalid-title", [model, {**model, "modelId": None}])
    add("hackernews", "second-row-invalid-title", {"hits": [hit, {**hit, "title": None}]})
    for engine in ["github", "huggingface", "stackexchange"]:
        add(
            engine,
            "invalid-header-secret",
            [] if engine == "huggingface" else {"items": []},
            config={"api_key": "fixture\nsecret"},
        )
    # Independent source-review cases: preserve the previous 227 full rows.
    for label, field, value, categories in [
        ("topics-string", "topics", "rust", []),
        ("topics-object", "topics", {"rust": True, "search": False}, []),
        ("labels-string", "labels", "bug", ["github:issues"]),
        ("labels-object", "labels", {"bug": True}, ["github:issues"]),
    ]:
        body = {
            "items": [
                {
                    "html_url": "https://example.invalid/repo",
                    "full_name": "octo/repo",
                    "title": "Issue",
                    "stargazers_count": 1,
                    "body": "body",
                    field: value,
                }
            ]
        }
        add("github", "independent-" + label, body, config={"max_results": 1}, categories=categories)
    add("stackexchange", "zero-limit-string-collection", {"items": "nonempty"}, config={"max_results": 0})
    return rows


def safe(value):
    if isinstance(value, float) and not math.isfinite(value):
        return {"$source_float": str(value)}
    if isinstance(value, dict):
        return {k: safe(v) for k, v in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [safe(v) for v in (sorted(value) if isinstance(value, set) else value)]
    return value


def invalid_result(row):
    return (
        any(not isinstance(row[k], str) for k in ["url", "title", "content", "engine", "category"])
        or (row["published_date"] is not None and not isinstance(row["published_date"], str))
        or not math.isfinite(row["score"])
    )


async def observe(case):
    requests = []
    spec = case["upstream"]

    async def handler(request):
        requests.append(
            {
                "method": request.method,
                "url": str(request.url),
                "raw_target": request.url.raw_path.decode("ascii"),
                "headers": dict(request.headers),
            }
        )
        if spec["fault"] == "timeout":
            raise httpx.ReadTimeout("fixture timeout", request=request)
        if spec["fault"] == "connect":
            raise httpx.ConnectError("fixture connect failure", request=request)
        if spec["fault"] == "cancel":
            raise asyncio.CancelledError()
        return httpx.Response(
            spec["status"], content=spec["body"].encode(), headers={"content-type": "application/json"}, request=request
        )

    class Denied:
        async def acquire(self, _name):
            return False

    adapter = ENGINES[case["engine"]](
        copy.deepcopy(case["config"]), rate_limiter=Denied() if case["admission_denied"] else None
    )
    transport = httpx.MockTransport(handler)
    adapter.set_http_transport(transport)
    try:
        response = await adapter.search(case["query"], copy.deepcopy(case["params"]))
        results = [dataclasses.asdict(row) for row in response.results]
        observation = {
            "response": safe(
                {"status": response.status.value, "results": results, "error_message": response.error_message}
            ),
            "source_exception": None,
        }
        if response.status.value == "ok" and any(invalid_result(row) for row in results):
            observation["native_disposition"] = "typed_result_or_finite_bound_refusal"
        else:
            observation["native_disposition"] = "source_response"
    except (Exception, asyncio.CancelledError) as exc:
        observation = {
            "response": None,
            "source_exception": {"type": type(exc).__name__, "message": str(exc)},
            "native_disposition": "caller_cancelled_no_response"
            if isinstance(exc, asyncio.CancelledError)
            else "source_parser_exception_classified",
        }
    finally:
        await adapter.shutdown()
        await transport.aclose()
    if any(
        label in case["case"] for label in [":max-results-", ":timeout-setting-negative", ":timeout-setting-overflow"]
    ):
        observation["native_disposition"] = "typed_config_bound_refusal"
    elif case["case"].endswith(":timeout-setting-zero"):
        observation["native_disposition"] = "real_zero_deadline_not_mock_timing"
    elif case["case"].endswith(":timeout-setting-fractional"):
        observation["native_disposition"] = "fractional_deadline_not_mock_timing"
    observation["requests"] = requests
    return case | observation


async def main(output):
    rows = [await observe(case) for case in scenarios()]
    assert len({row["case"] for row in rows}) == len(rows)
    sources = {p: subprocess.check_output(["git", "hash-object", p]).decode().strip() for p in SOURCES}
    assert all(
        subprocess.check_output(["git", "rev-parse", BASELINE + ":" + p]).decode().strip() == h
        for p, h in sources.items()
    )
    result = {
        "schema_version": 1,
        "plan_id": "A03",
        "baseline": BASELINE,
        "python": "3.14.7",
        "httpx": importlib.metadata.version("httpx"),
        "provenance": (
            "Actual unmodified adapter.search with injected caller-owned MockTransport, synthetic "
            "inputs; no provider/network calls. Source exceptions are observations, not fabricated "
            "AdapterResponses."
        ),
        "normalization": (
            "Latency omitted; source engines sets sorted; nonfinite float scalars tagged losslessly. "
            "Implicit HTTPX headers retained as provenance; engine-owned "
            "headers/auth/method/raw-target/count asserted by native replay. Native dispositions label "
            "explicit typed/config/deadline boundaries, never rewrite source outputs."
        ),
        "sources": sources,
        "cases": rows,
    }
    Path(output).write_text(json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 14, 7) or importlib.metadata.version("httpx") != "0.28.1":
        raise RuntimeError("Pinned Python3.14.7/HTTPX0.28.1 required")
    asyncio.run(main(args.output))
