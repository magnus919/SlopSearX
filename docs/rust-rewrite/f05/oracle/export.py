#!/usr/bin/env python3
"""Capture F05 DTO and identity goldens from the pinned Python source tree.

Developer-only tool. All cases are local pure-function observations; this
program does not construct a runtime, connect to a store, or call providers.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import os
import struct
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
BASELINE = "16f03be656f34d90edd8285f9c81dfb1c4626b0a"
SOURCE_PATHS = [
    "slopsearx/adapter.py",
    "slopsearx/service.py",
    "slopsearx/payload.py",
    "slopsearx/formatter.py",
    "slopsearx/publication_metadata.py",
    "slopsearx/scholarly.py",
    "slopsearx/mcp/result_serialization.py",
    "slopsearx/retrieval_url.py",
    "slopsearx/snapshot.py",
    "slopsearx/artifacts.py",
    "slopsearx/staged.py",
    "slopsearx/saved_models.py",
    "tests/test_result_contract.py",
    "tests/test_payload.py",
    "tests/test_media_contract.py",
    "tests/test_mcp_snapshots.py",
    "tests/test_searxng_api_contract.py",
]
SAFE_ENV = {"PATH", "LANG", "LC_ALL", "TMPDIR", "SYSTEMROOT"}
_PRIVATE_HOME: tempfile.TemporaryDirectory[str] | None = None


def clean_environment() -> None:
    global _PRIVATE_HOME
    env = {key: value for key, value in os.environ.items() if key in SAFE_ENV}
    _PRIVATE_HOME = tempfile.TemporaryDirectory(prefix="ssx-f05-oracle-")
    env["HOME"] = _PRIVATE_HOME.name
    env["XDG_CONFIG_HOME"] = str(Path(_PRIVATE_HOME.name) / ".config")
    env["XDG_CACHE_HOME"] = str(Path(_PRIVATE_HOME.name) / ".cache")
    os.environ.clear()
    os.environ.update(env)


def git_env() -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if key in SAFE_ENV}
    env.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "core.fsmonitor",
            "GIT_CONFIG_VALUE_0": "false",
        }
    )
    return env


def git(*args: str, cwd: Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, env=git_env(), text=True).strip()


def verify_source(source: Path, baseline: str) -> dict[str, str]:
    if git("rev-parse", "HEAD", cwd=source) != baseline:
        raise SystemExit("source root is not the pinned baseline")
    if git("status", "--porcelain", "--untracked-files=all", cwd=source):
        raise SystemExit("source root must be clean, including untracked files")
    for package in (source / "slopsearx", source / "engines"):
        if package.exists() and any(package.rglob("*.pyc")):
            raise SystemExit(f"ignored Python bytecode found under {package.relative_to(source)}")
    return {path: git("rev-parse", f"{baseline}:{path}", cwd=source) for path in SOURCE_PATHS}


def as_result(adapter: Any, raw: dict[str, Any]) -> Any:
    media = raw.get("media")
    if isinstance(media, dict):
        media = adapter.build_media(
            media.get("media_type", ""),
            url=media.get("url"),
            thumbnail=media.get("thumbnail"),
            source=media.get("source"),
            width=media.get("width"),
            height=media.get("height"),
            duration=media.get("duration"),
        )
    return adapter.SearchResult(
        url=raw.get("url", ""),
        title=raw.get("title", ""),
        content=raw.get("content", ""),
        engine=raw.get("engine", ""),
        engines=set(raw.get("engines", [])),
        score=raw.get("score", 0.0),
        position=raw.get("position", 0),
        category=raw.get("category", "general"),
        published_date=raw.get("published_date"),
        thumbnail=raw.get("thumbnail"),
        img_src=raw.get("img_src"),
        tier=raw.get("tier", 1),
        media=media,
        payload=raw.get("payload"),
        work_group=raw.get("work_group"),
    )


def canonical_input(value: Any, profile: str) -> dict[str, str]:
    # Preserve lexical numeric types and integers beyond native JSON integer
    # widths; Rust reads the exact Python-parsed source string.
    return {"value_json": json.dumps(value, ensure_ascii=False, separators=(",", ":")), "profile": profile}


def capture(source: Path) -> list[dict[str, Any]]:
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(source))
    from slopsearx import adapter, artifacts, formatter, saved_models, service, staged
    from slopsearx.mcp import result_serialization
    from slopsearx.payload import payload_serialized_size

    raw = {
        "url": "https://example.test/paper?ok=1",
        "title": 'Café "record"',
        "content": "line one\nline two",
        "engine": "arxiv",
        "engines": ["zeta", "arxiv"],
        "score": 0.75,
        "position": 2,
        "category": "science",
        "published_date": "2024-02-29",
        "thumbnail": None,
        "img_src": None,
        "tier": 2,
        "media": {
            "media_type": "image",
            "url": "https://img.example.test/a?size=small",
            "thumbnail": "https://img.example.test/t",
            "source": "https://source.example.test/",
            "width": 0,
            "height": 480,
            "duration": None,
        },
        "payload": {
            "domain": "science",
            "type": "paper",
            "schema_version": 1,
            "data": {"title": "Café", "values": [1, 2]},
            "provenance": {
                "engine": "arxiv",
                "adapter_fields": ["title"],
                "normalized_fields": [],
                "inferred_fields": [],
            },
        },
        "work_group": {
            "paper": {
                "doi": "https://doi.org/10.5555/UPPER",
                "journal": "Journal",
                "authors": ["Ada Example"],
                "volume": 0,
            },
            "private": "not public",
        },
    }
    result = as_result(adapter, raw)
    media_built = adapter.build_media(
        "image",
        url="https://img.example.test/a?token=secret&size=small",
        thumbnail="https://img.example.test/t?api_key=secret",
        source="https://source.example.test/?key=secret",
    )
    media_normalized = adapter.build_media(
        "video",
        url="https://img.example.test/a?a=hello%20world&tag=x&tag=y&empty=&token=secret&%61pi_key=secret#frag",
    )
    media_encoding = adapter.build_media("video", url="https://img.example.test/a?space=hello%20world&star=*&tilde=~")
    media_fragment = adapter.build_media("image", url="https://img.example.test/a#frag?token=secret")
    malformed_media_url = "http://[bad?token=secret"
    media_malformed = adapter.build_media("image", url=malformed_media_url)
    stored = service.search_result_to_dict(result)

    def payload_at_size(target: int) -> dict[str, Any]:
        payload = {"number": 1e6, "unicode": "", "pad": ""}
        difference = target - payload_serialized_size(payload)
        if difference < 0:
            raise AssertionError("payload base exceeds requested size")
        if difference % 2:
            payload["pad"] = "x"
            difference -= 1
        payload["unicode"] = "é" * (difference // 2)
        if payload_serialized_size(payload) != target:
            raise AssertionError(f"could not construct payload of exactly {target} bytes")
        return payload

    boundary_payloads = [payload_at_size(16_384), payload_at_size(16_385)]
    boundary_expected = [
        service.search_result_to_dict(as_result(adapter, {"payload": payload})) for payload in boundary_payloads
    ]
    legacy = {
        "url": "u",
        "score": "1.25",
        "position": "-2",
        "tier": "7",
        "engines": "{'arxiv', 'github'}",
        "media": {"media_type": "image", "url": "https://x.test/?token=secret", "height": None},
    }
    legacy_restored = service.search_result_from_dict(legacy)
    legacy_expected = service.search_result_to_dict(legacy_restored)
    missing = service.search_result_to_dict(
        service.search_result_from_dict({"engines": None, "media": {"media_type": "unknown"}})
    )
    lossless_result_input = {
        "payload": {
            "$serde_json::private::Number": "123",
            "nested": {"n": 18446744073709551617},
        }
    }
    lossless_result_expected = service.search_result_to_dict(service.search_result_from_dict(lossless_result_input))
    lossless_response_input = {
        "answers": [{"$serde_json::private::Number": "456", "n": 18446744073709551617}],
        "infoboxes": [{"nested": [-9223372036854775809]}],
    }
    lossless_response_expected = service.search_response_to_payload(
        service.search_response_from_payload(lossless_response_input)
    )
    legacy_response_input = {
        "query": 7,
        "results": None,
        "scope": {
            "routing_rule": 0,
            "routing_fallback": "false",
            "routing_budget_applied": [],
            "jev_scores": {"rank": "1.25"},
        },
        "engine_outcomes": [
            {
                "engine": 3,
                "status": None,
                "result_count": "2",
                "latency_ms": "3.5",
                "message": "legacy",
            }
        ],
        "response_time_ms": "12",
        "cached": "false",
        "partial": 0,
        "dispatched_engine_count": "4",
    }
    legacy_response_expected = service.search_response_to_payload(
        service.search_response_from_payload(legacy_response_input)
    )

    response = service.SearchResponse(
        query="Café",
        results=[result],
        scope=service.ScopeDecision(selected_engines=["arxiv"], resolved_categories=["science"]),
        engine_outcomes=[service.EngineOutcome(engine="arxiv", status="ok", result_count=1, latency_ms=1.25)],
        suggestions=["Café"],
        answers=[],
        corrections=[],
        infoboxes=[],
        query_id="query-fixture",
        cached=True,
        response_time_ms=17,
        partial=False,
        deadline_exceeded=False,
        all_unresponsive=False,
        empty_engines=[],
        cached_error=False,
        ranking_explanation="tier_then_cross_engine_presence",
        dispatched_engine_count=1,
    )
    response_encoded = service.search_response_to_payload(response)
    response_roundtrip = service.search_response_to_payload(service.search_response_from_payload(response_encoded))
    http_projection = formatter._result_to_searxng(result)

    def http_projection_input(value: dict[str, Any]) -> dict[str, Any]:
        source_projection = formatter._result_to_searxng(as_result(adapter, value))
        return {
            "result": value,
            "metadata": {
                "pubdate": source_projection["pubdate"],
                "parsed_url": source_projection["parsed_url"],
            },
        }

    yaml_text = formatter.format_yaml_markdown([result], "Café", engine_count=1, responsive_count=1)
    yaml_header = yaml_text.split("---\n", 1)[0]
    yaml_value = __import__("yaml").safe_load(yaml_header)

    card_raw = {**raw, "url": "https://127.0.0.1/private"}
    card_result = as_result(adapter, card_raw)
    card = result_serialization._result_to_dict(card_result, result_id="snapshot-fixture:0")
    lossless_public_raw = {
        "url": "https://example.test/lossless",
        "title": "Lossless payload",
        "engine": "fixture",
        "engines": ["fixture"],
        "score": 0.5,
        "payload": {
            "$serde_json::private::Number": "789",
            "nested": {"n": 18446744073709551617},
        },
    }
    lossless_public_result = as_result(adapter, lossless_public_raw)
    lossless_http = formatter._result_to_searxng(lossless_public_result)
    lossless_mcp = result_serialization._result_to_dict(lossless_public_result, result_id="snapshot-fixture:1")
    canonical_value = {"quote": '"\\\n', "café": "Straße", "a": [None, True, 1]}
    canonical_json = __import__("json")
    canonical_cases = []
    for profile, options in [
        ("ascii_compact", {"ensure_ascii": True, "separators": (",", ":"), "sort_keys": True}),
        ("utf8_compact", {"ensure_ascii": False, "separators": (",", ":"), "sort_keys": True}),
        ("ascii_spaced", {"ensure_ascii": True, "separators": (", ", ": "), "sort_keys": True}),
    ]:
        canonical_cases.append(
            {
                "id": f"canonical-{profile}",
                "kind": "canonical_json",
                "input": canonical_input(canonical_value, profile),
                "expected": canonical_json.dumps(canonical_value, **options),
            }
        )
    float_bits = []
    state = 0x6A09E667F3BCC909
    while len(float_bits) < 256:
        state = (state * 6364136223846793005 + 1442695040888963407) & ((1 << 64) - 1)
        value = struct.unpack(">d", state.to_bytes(8, "big"))[0]
        if __import__("math").isfinite(value):
            float_bits.append(value)
    canonical_cases.extend(
        [
            {
                "id": "canonical-float-bit-patterns-256",
                "kind": "canonical_json",
                "input": canonical_input(float_bits, "ascii_compact"),
                "expected": canonical_json.dumps(float_bits, separators=(",", ":"), sort_keys=True),
            },
            {
                "id": "canonical-negative-zero-float",
                "kind": "canonical_json",
                "input": canonical_input(-0.0, "ascii_compact"),
                "expected": canonical_json.dumps(-0.0, separators=(",", ":"), sort_keys=True),
            },
            {
                "id": "canonical-zero-integer",
                "kind": "canonical_json",
                "input": canonical_input(0, "ascii_compact"),
                "expected": canonical_json.dumps(0, separators=(",", ":"), sort_keys=True),
            },
            {
                "id": "canonical-float-boundaries",
                "kind": "canonical_json",
                "input": canonical_input(
                    [-0.0, 1e-5, 1e-4, 1e15, 1e16, 1.7976931348623157e308, 5e-324], "ascii_compact"
                ),
                "expected": canonical_json.dumps(
                    [-0.0, 1e-5, 1e-4, 1e15, 1e16, 1.7976931348623157e308, 5e-324],
                    separators=(",", ":"),
                    sort_keys=True,
                ),
            },
            {
                "id": "canonical-large-integers",
                "kind": "canonical_json",
                "input": canonical_input(
                    {"above_u64": 18446744073709551617, "below_i64": -9223372036854775809}, "ascii_compact"
                ),
                "expected": canonical_json.dumps(
                    {"above_u64": 18446744073709551617, "below_i64": -9223372036854775809},
                    separators=(",", ":"),
                    sort_keys=True,
                ),
            },
            {
                "id": "unicode-casefold-16",
                "kind": "unicode_casefold",
                "input": {"value": "Straße ẞ ﬃ İ \U0001e030"},
                "expected": "Straße ẞ ﬃ İ \U0001e030".casefold(),
            },
        ]
    )
    comp_parts = ["snap-α", 'result:"one"', "line\nthree"]
    canonical_cases.extend(
        [
            {
                "id": "composite-artifact-unicode-escapes",
                "kind": "composite_id",
                "input": comp_parts,
                "expected": artifacts.composite_artifact_id(*comp_parts),
            },
            {
                "id": "manifest-id-stable-order",
                "kind": "manifest_id",
                "input": {"z": 1, "a": {"café": True}},
                "expected": artifacts.manifest_artifact_id({"z": 1, "a": {"café": True}}),
            },
            {
                "id": "staged-plan-digest-unicode",
                "kind": "plan_digest",
                "input": {"z": 1, "a": {"café": "Straße", "items": []}},
                "expected": staged.plan_digest({"z": 1, "a": {"café": "Straße", "items": []}}),
            },
            {
                "id": "saved-stable-id-unicode",
                "kind": "saved_identity",
                "input": ["tenant-α", {"engine": "npm", "query": "Café"}],
                "expected": saved_models.stable_id("tenant-α", {"engine": "npm", "query": "Café"}),
            },
            {
                "id": "saved-identity-zero-parts",
                "kind": "saved_identity",
                "input": [],
                "expected": saved_models.stable_id(),
            },
            {
                "id": "saved-identity-one-object",
                "kind": "saved_identity",
                "input": [{"query": "Café", "engines": ["z", "a"]}],
                "expected": saved_models.stable_id({"query": "Café", "engines": ["z", "a"]}),
            },
            {
                "id": "saved-identity-one-array",
                "kind": "saved_identity",
                "input": [["a", 1, None]],
                "expected": saved_models.stable_id(["a", 1, None]),
            },
            {
                "id": "saved-identity-multiple-parts",
                "kind": "saved_identity",
                "input": ["tenant-α", {"query": "Café"}, 3],
                "expected": saved_models.stable_id("tenant-α", {"query": "Café"}, 3),
            },
        ]
    )
    formatter_cases = []
    for index, published_date in enumerate(
        [
            "2026-10-15T12:34:56Z",
            "2026-10-15T12:34:56+02:30",
            "2024-02-29",
            "2025-02-29",
            "2026-02-31",
            "2026-10-15garbage",
            "123456789é",
            "2026-10-15T00:00:00-05:00",
        ]
    ):
        value = {"url": "https://example.test/item", "score": 0.5, "published_date": published_date}
        formatter_cases.append(
            {
                "id": f"http-json-result-date-{index}",
                "kind": "http_json_result",
                "input": http_projection_input(value),
                "expected": formatter._result_to_searxng(as_result(adapter, value)),
            }
        )
    for index, url in enumerate(
        [
            "HTTPS://example.test/a;p?q=1#f",
            "https://[invalid/a",
            "https://example.test/a;b/c;d?q#f",
            "https://example.test/#f?token=x",
        ]
    ):
        value = {"url": url, "score": 0.5}
        formatter_cases.append(
            {
                "id": f"http-json-result-url-{index}",
                "kind": "http_json_result",
                "input": http_projection_input(value),
                "expected": formatter._result_to_searxng(as_result(adapter, value)),
            }
        )
    cases = [
        {"id": "storage-result-write-rich", "kind": "storage_result_write", "input": raw, "expected": stored},
        {
            "id": "storage-result-write-integer-score-zero",
            "kind": "storage_result_write",
            "input": {"score": 0},
            "expected": service.search_result_to_dict(as_result(adapter, {"score": 0})),
        },
        {
            "id": "storage-result-write-payload-boundary-includes",
            "kind": "storage_result_write",
            "input": {"payload": boundary_payloads[0]},
            "expected": boundary_expected[0],
        },
        {
            "id": "storage-result-write-payload-boundary-omits",
            "kind": "storage_result_write",
            "input": {"payload": boundary_payloads[1]},
            "expected": boundary_expected[1],
        },
        {
            "id": "media-build-sanitizes-sensitive-query",
            "kind": "media_build",
            "input": {
                "media_type": "image",
                "url": "https://img.example.test/a?token=secret&size=small",
                "thumbnail": "https://img.example.test/t?api_key=secret",
                "source": "https://source.example.test/?key=secret",
            },
            "expected": adapter.media_to_dict(media_built),
        },
        {
            "id": "media-build-normalizes-retained-query",
            "kind": "media_build",
            "input": {
                "media_type": "video",
                "url": "https://img.example.test/a?a=hello%20world&tag=x&tag=y&empty=&token=secret&%61pi_key=secret#frag",
            },
            "expected": adapter.media_to_dict(media_normalized),
        },
        {
            "id": "media-build-python-query-encoding",
            "kind": "media_build",
            "input": {"media_type": "video", "url": "https://img.example.test/a?space=hello%20world&star=*&tilde=~"},
            "expected": adapter.media_to_dict(media_encoding),
        },
        {
            "id": "media-build-question-mark-in-fragment",
            "kind": "media_build",
            "input": {"media_type": "image", "url": "https://img.example.test/a#frag?token=secret"},
            "expected": adapter.media_to_dict(media_fragment),
        },
        {
            "id": "media-build-malformed-ipv6-fallback",
            "kind": "media_build",
            "input": {"media_type": "image", "url": malformed_media_url},
            "expected": adapter.media_to_dict(media_malformed),
        },
        {
            "id": "storage-result-read-lossless-payload",
            "kind": "storage_result_read",
            "input": lossless_result_input,
            "expected": lossless_result_expected,
        },
        {
            "id": "storage-response-read-lossless-large-integers",
            "kind": "storage_response_read",
            "input": lossless_response_input,
            "expected": lossless_response_expected,
        },
        {
            "id": "storage-response-read-legacy-coercions",
            "kind": "storage_response_read",
            "input": legacy_response_input,
            "expected": legacy_response_expected,
        },
        {
            "id": "storage-result-read-legacy-engines",
            "kind": "storage_result_read",
            "input": legacy,
            "expected": legacy_expected,
        },
        {
            "id": "storage-result-read-missing-null",
            "kind": "storage_result_read",
            "input": {"engines": None, "media": {"media_type": "unknown"}},
            "expected": missing,
        },
        {
            "id": "storage-response-write-full",
            "kind": "storage_response_write",
            "input": response_encoded,
            "expected": response_encoded,
        },
        {
            "id": "storage-response-read-roundtrip",
            "kind": "storage_response_read",
            "input": response_encoded,
            "expected": response_roundtrip,
        },
        {
            "id": "http-json-result-rich",
            "kind": "http_json_result",
            "input": http_projection_input(raw),
            "expected": http_projection,
        },
        {
            "id": "http-json-result-lossless-payload",
            "kind": "http_json_result",
            "input": http_projection_input(lossless_public_raw),
            "expected": lossless_http,
        },
        {
            "id": "mcp-result-card-lossless-payload",
            "kind": "mcp_card",
            "input": {
                "result": lossless_public_raw,
                "snapshot_id": "snapshot-fixture",
                "index": 1,
                "retrieval": lossless_mcp["retrieval"],
            },
            "expected": lossless_mcp,
        },
        {"id": "http-yaml-result-rich", "kind": "http_yaml_result", "input": raw, "expected": yaml_value["results"][0]},
        {
            "id": "mcp-result-card-rich",
            "kind": "mcp_card",
            "input": {
                "result": card_raw,
                "snapshot_id": "snapshot-fixture",
                "index": 0,
                "retrieval": card["retrieval"],
            },
            "expected": card,
        },
    ]
    # Accepted legacy coercions are observations of the actual pinned readers.
    # Input object insertion order is observable when Python str(dict) is used.
    numeric_results = [
        ("fractional-and-bool", {"position": 2.75, "tier": -2.75, "score": True}),
        ("whitespace", {"position": "\v 2 \f", "tier": "\u2003-2\u2002", "score": "\v 2.5 \f"}),
        ("unicode-underscores", {"position": "٢", "tier": " １２ ", "score": "١_٢.٥"}),
        ("bool-false", {"position": False, "tier": True, "score": False}),
    ]
    for label, value in numeric_results:
        cases.append(
            {
                "id": f"storage-result-read-numeric-{label}",
                "kind": "storage_result_read",
                "input": value,
                "expected": service.search_result_to_dict(service.search_result_from_dict(value)),
            }
        )
    string_result = {
        "url": True,
        "title": False,
        "content": None,
        "engine": [True, None, {"z": False, "a": "\u2028"}],
        "engines": [True, None, [True, "a'b", 'a"b', "\t\n\r\x85\u200b\ue000\u0378😀"], {"z": True, "a": None}],
    }
    cases.append(
        {
            "id": "storage-result-read-python-str-json-types",
            "kind": "storage_result_read",
            "input": string_result,
            "expected": service.search_result_to_dict(service.search_result_from_dict(string_result)),
        }
    )
    response_values = [
        (
            "fractional-bool",
            {
                "response_time_ms": 2.75,
                "dispatched_engine_count": True,
                "engine_outcomes": [{"result_count": -2.75, "latency_ms": True}],
                "scope": {"jev_scores": {"rank": False}},
            },
        ),
        (
            "whitespace",
            {
                "response_time_ms": " 2 ",
                "dispatched_engine_count": " -2 ",
                "engine_outcomes": [{"result_count": " 2 ", "latency_ms": " 2.5 "}],
                "scope": {"jev_scores": {"rank": " ١_٢.٥ "}},
            },
        ),
        (
            "python-str-json-types",
            {
                "query": True,
                "query_id": None,
                "ranking_explanation": {"z": True, "a": [None, "a'b", 'a"b', "\u200b\ue000\u0378😀"]},
                "suggestions": [True, None, {"z": 1, "a": False}],
                "corrections": [[True, None]],
                "empty_engines": [[True, None, {"z": 1, "a": False}]],
                "engine_outcomes": [{"engine": True, "status": False}],
                "scope": {
                    "routing_rule": True,
                    "excluded_engines": [{"engine": True, "reason": {"z": 1, "a": False}, "stage": [True, None]}],
                },
            },
        ),
    ]
    decimal_zeroes = [code for code in range(0x110000) if chr(code).isdecimal() and int(chr(code)) == 0]
    response_values.append(
        (
            "unicode16-decimal-digits",
            {
                "engine_outcomes": [
                    {
                        "engine": f"unicode-{index}",
                        "result_count": chr(zero + 7),
                        "latency_ms": chr(zero + 4) + "." + chr(zero + 5),
                    }
                    for index, zero in enumerate(decimal_zeroes)
                ],
                "scope": {
                    "jev_scores": {
                        f"unicode-{index}": chr(zero + 4) + "." + chr(zero + 5)
                        for index, zero in enumerate(decimal_zeroes)
                    }
                },
            },
        )
    )
    for label, value in response_values:
        cases.append(
            {
                "id": f"storage-response-read-{label}",
                "kind": "storage_response_read",
                "input": value,
                "expected": service.search_response_to_payload(service.search_response_from_payload(value)),
            }
        )
    for label, value in [
        ("newline-key", "https://x.test/a?to\nken=secret&size=small"),
        ("tab-key", "https://x.test/a?to\tken=secret&size=small"),
        ("carriage-return-key", "https://x.test/a?to\rken=secret&size=small"),
        ("leading-controls", "\x00 \tHTTPS://x.test/a?token=secret&size=small#"),
        ("no-query-original", "https://x.test/a\n"),
        ("empty-query-original", "https://x.test/a?\n#fragment"),
    ]:
        cases.append(
            {
                "id": f"media-build-urllib-preprocess-{label}",
                "kind": "media_build",
                "input": {"media_type": "image", "url": value},
                "expected": adapter.media_to_dict(adapter.build_media("image", url=value)),
            }
        )
    urllib_vectors = [
        ("relative-colon", "A/B:C?token=secret&size=small"),
        ("relative-colon-unicode", "Ä/B:C?token=secret&size=small"),
        ("single-slash", "http:/foo?token=s&x=1"),
        ("opaque-scheme", "http:foo?token=s&x=1"),
        ("empty-custom-authority", "custom://?token=s&x=1"),
        ("empty-relative-authority", "//?token=s&x=1"),
        ("four-slashes", "http:////x/a;?token=s&x=1"),
        ("relative-empty-params", "A/B;?token=s&x=1"),
        ("invalid-ipv6-host", "https://[bad]/a?token=s&x=1"),
        ("invalid-relative-ipv6-host", "//[bad]/a?token=s&x=1"),
        ("unmatched-close-bracket", "https://example.com]/a?token=s&x=1"),
        ("data-after-bracket", "https://[::1]x/a?token=s&x=1"),
        ("data-before-bracket", "https://prefix[::1]/a?token=s&x=1"),
        ("ipv4-in-brackets", "https://[127.0.0.1]/a?token=s&x=1"),
        ("valid-ipv6", "https://[::1]/a?token=s&x=1"),
        ("valid-ipv6-scope", "https://[fe80::1%scope]/a?token=s&x=1"),
        ("invalid-ipv6-scope", "https://[fe80::1%]/a?token=s&x=1"),
        ("valid-ipvfuture", "https://[vF.example]/a?token=s&x=1"),
        ("invalid-ipvfuture", "https://[vXYZ.example]/a?token=s&x=1"),
    ]
    for code in range(128, 0x110000):
        if any(char in unicodedata.normalize("NFKC", chr(code)) for char in "/?#@:"):
            urllib_vectors.append((f"nfkc-delimiter-{code:x}", f"https://x{chr(code)}y/a?token=s&x=1"))
    for label, value in urllib_vectors:
        cases.append(
            {
                "id": f"media-build-urllib-{label}",
                "kind": "media_build",
                "input": {"media_type": "image", "url": value},
                "expected": adapter.media_to_dict(adapter.build_media("image", url=value)),
            }
        )
    from slopsearx.publication_metadata import validate_paper_fields

    publication_vectors = [
        ("utf8-panic", {"doi": "aééé"}),
        ("unicode-casefold", {"doi": "10.1234/StraßeİΣﬃ"}),
        ("unicode-digits-prefix", {"doi": "  DOI:\u001c10.١٢٣٤/UPPER  "}),
        ("unicode-ignorecase-prefix", {"doi": "HTTPſ://DOİ.ORG/10.1234/UPPER"}),
        ("doi-char-boundary", {"doi": "10.1234/" + "é" * 504}),
        ("doi-char-over", {"doi": "10.1234/" + "é" * 505}),
        ("doi-no-double-prefix", {"doi": "https://doi.org/doi:10.1234/x"}),
        ("doi-invalid-digit", {"doi": "10.²²²²/x"}),
        ("list-char-boundary", {"authors": ["é" * 256], "tags": ["😀" * 257]}),
        ("volume-string-boundary", {"volume": "é" * 256}),
        ("volume-float", {"volume": 1.0}),
        ("volume-bool", {"volume": True}),
        ("volume-integer", {"volume": 10**255}),
        ("volume-over", {"volume": 10**256}),
        ("string-byte-boundary", {"journal": "é" * 1024, "publisher": "é" * 1025}),
        ("doi-whitespace", {"doi": "10.1234/a\u001fb"}),
    ]
    publication_vectors.extend(
        [
            ("volume-newlines", {"volume": "\n" * 256}),
            ("list-newlines", {"authors": ["\n" * 256]}),
            ("list-count-over", {"authors": [""] * 65}),
            ("doi-non-bmp", {"doi": "10.1234/𐐀"}),
        ]
    )
    decimal_zeroes = json.loads((ROOT / "contracts/unicode/python-text-16.0.0.json").read_text())["decimal_zeroes"]
    for zero in decimal_zeroes:
        publication_vectors.append(
            (f"decimal-{zero:x}", {"doi": "10." + "".join(chr(zero + i) for i in range(4)) + "/UPPER"})
        )
    for label, value in publication_vectors:
        cases.append(
            {
                "id": f"publication-{label}",
                "kind": "paper_fields",
                "input": value,
                "expected": validate_paper_fields(value),
            }
        )
    return cases + formatter_cases + canonical_cases


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--baseline", default=BASELINE)
    parser.add_argument("--output", type=Path, default=ROOT / "crates/xtask/fixtures/f05/corpus.json")
    args = parser.parse_args()
    if args.baseline != BASELINE:
        raise SystemExit("unexpected F05 source baseline")
    clean_environment()
    source = args.source_root.resolve()
    blobs = verify_source(source, args.baseline)
    cases = capture(source)
    payload = {
        "schema_version": 1,
        "baseline_sha": args.baseline,
        "provenance": {
            "sources": [{"path": path, "git_blob": blobs[path]} for path in SOURCE_PATHS],
            "oracle": {
                "path": "docs/rust-rewrite/f05/oracle/export.py",
                "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            },
            "scope": "Pinned Python function observations; offline and fixture-only; not Rust parity evidence.",
            "license": "MIT",
            "redaction": "Synthetic inputs only; no credentials, personal records, or provider responses.",
            "runtime": {
                "python": sys.version.split()[0],
                "unicode": unicodedata.unidata_version,
                "packages": {
                    package: importlib.metadata.version(package) for package in ("PyYAML", "httpx", "fastmcp", "mcp")
                },
            },
        },
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.output} ({len(cases)} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
