# ruff: noqa: E402
# Exact source guards and local root must precede backend imports.
"""Offline instrumentation of the actual old regression tests; retired G08."""

import asyncio
import contextvars
import hashlib
import json
import os
import pathlib
import sys
import unicodedata

import httpx
import pytest

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
import engines.internetarchive as archive_module
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

classes = [OpenLibraryAdapter, MusicBrainzAdapter, TMDBAdapter, NominatimAdapter, InternetArchiveAdapter]
current = contextvars.ContextVar("capture", default=None)
records = []
current_node = None
test_results = []
helper_assertions = []
capture_helpers = True
helper_nodes = {
    "tests/test_internetarchive_adapter.py::TestInternetArchiveAdapterHelpers::test_is_domain_query_detects_domain",
    "tests/test_internetarchive_adapter.py::TestInternetArchiveAdapterHelpers::test_is_domain_query_rejects_text",
    "tests/test_tier1_adapters.py::TestInternetArchiveAdapter::test_is_domain_query_detects_domain",
    "tests/test_tier1_adapters.py::TestInternetArchiveAdapter::test_is_domain_query_rejects_regular_text",
}
original_domain = archive_module._is_domain_query


def capture_domain(value):
    result = original_domain(value)
    if capture_helpers and current_node and current_node in helper_nodes:
        helper_assertions.append(dict(node=current_node, value=value, output=result))
    return result


archive_module._is_domain_query = capture_domain


def clean(s, keys):
    for key in keys:
        if key and key.strip():
            s = s.replace(key, "<synthetic-key>")
    return s


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


consumer_records = []
import slopsearx.formatter as formatter_module

original_format_json = formatter_module.format_json


def capture_format_json(*args, **kwargs):
    result = original_format_json(*args, **kwargs)
    if current_node and current_node.startswith("tests/test_openlibrary_url_contract.py::"):
        consumer_records.append(dict(node=current_node, output=result))
    return result


formatter_module.format_json = capture_format_json


class Plugin:
    def pytest_runtest_setup(self, item):
        global current_node
        current_node = item.nodeid

    def pytest_runtest_logreport(self, report):
        if report.when == "call":
            test_results.append(dict(node=report.nodeid, outcome=report.outcome))


original_send = httpx.AsyncClient.send


async def send(client, req, *args, **kwargs):
    case = current.get()
    if case is None:
        return await original_send(client, req, *args, **kwargs)
    keys = case.pop("_keys_tmp") if "_keys_tmp" in case else case["_keys"]
    case["_keys"] = keys
    case["requests"].append(
        dict(
            method=req.method,
            url=clean(str(req.url), keys),
            body=req.content.decode(),
            headers={
                k: clean(v, keys)
                for k, v in req.headers.items()
                if k in ["user-agent", "accept", "content-type", "authorization"]
            },
        )
    )
    try:
        response = await original_send(client, req, *args, **kwargs)
        case["responses"].append(dict(status=response.status_code, wire_body=response.content.decode()))
        return response
    except httpx.TimeoutException:
        case["responses"].append(dict(status=200, exception="timeout"))
        raise
    except Exception as exc:
        case["responses"].append(dict(status=200, exception=type(exc).__name__, diagnostic=clean(str(exc), keys)))
        raise


for cls in classes:
    original = cls.search

    # Keep the actual result in the wrapper itself rather than serializing/rebuilding.
    def make(original):
        async def search(adapter, query, params=None):
            cfg = dict(adapter.config)
            keys = [v for k, v in cfg.items() if "key" in k.lower() and isinstance(v, str)]
            env = os.environ.get("ENGINE_TMDB_API_KEY")
            if env:
                keys.append(env)
            public = {
                k: ("synthetic-tmdb-key" if "key" in k.lower() and isinstance(v, str) and v.strip() else v)
                for k, v in cfg.items()
            }
            if env:
                public["environment_api_key"] = "synthetic-tmdb-key" if env.strip() else env
            case = dict(
                id=current_node + "#" + str(sum(r["node"] == current_node for r in records)),
                node=current_node,
                engine=adapter.name,
                query=query,
                config=public,
                responses=[],
                requests=[],
                _keys=keys,
            )
            token = current.set(case)
            try:
                result = await original(adapter, query, params)
                case["output"] = output(result)
                if case["output"]["error_message"]:
                    case["output"]["error_message"] = clean(case["output"]["error_message"], keys)
                return result
            except Exception as exc:
                case["output"] = dict(raised=type(exc).__name__, error_message=clean(str(exc), keys))
                raise
            finally:
                current.reset(token)
                case.pop("_keys", None)
                records.append(case)

        return search

    cls.search = make(original)
httpx.AsyncClient.send = send
args = [
    "-q",
    "--no-cov",
    "-p",
    "no:cacheprovider",
    "tests/test_internetarchive_adapter.py",
    "tests/test_openlibrary_url_contract.py",
    "tests/test_tmdb.py",
    "tests/test_new_engines.py::TestMusicBrainzAdapter",
    "tests/test_new_engines.py::TestOpenLibraryAdapter",
    "tests/test_new_engines.py::TestTMDBAdapter",
    "tests/test_new_engines.py::TestNominatimAdapter",
    "tests/test_tier1_adapters.py::TestInternetArchiveAdapter",
    "tests/test_payload.py::TestMediaPayload::test_tmdb_payload",
]
for key in list(os.environ):
    if key.startswith("ENGINE_"):
        os.environ.pop(key)
code = pytest.main(args, plugins=[Plugin()])
if code != 0:
    raise RuntimeError("original A09 regression execution failed")
capture_helpers = False
original_adapter_invocations = len(records)
if len(test_results) != 56 or not all(row["outcome"] == "passed" for row in test_results):
    raise RuntimeError("original A09 parameter results incomplete or failed")


async def replay_helper_assertions():
    global current_node
    for assertion in helper_assertions:
        current_node = assertion["node"]
        body = [["timestamp"]] if assertion["output"] else {"response": {"docs": []}}
        adapter = InternetArchiveAdapter({"enabled": True})
        adapter.set_http_transport(httpx.MockTransport(lambda req: httpx.Response(200, json=body)))
        try:
            await adapter.search(assertion["value"])
        finally:
            await adapter.shutdown()
        records[-1]["helper_assertion"] = assertion


asyncio.run(replay_helper_assertions())

p = ROOT / "docs/rust-rewrite/a09/regressions.json"
p.write_text(
    json.dumps(
        dict(
            schema_version=1,
            python=sys.version,
            unicode=unicodedata.unidata_version,
            producer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
            results=test_results,
            consumer_records=consumer_records,
            original_adapter_invocations=original_adapter_invocations,
            helper_derived_adapter_invocations=len(helper_assertions),
            helper_assertions=helper_assertions,
            records=records,
        ),
        ensure_ascii=True,
        indent=2,
    )
    + "\n"
)
print(len(records), hashlib.sha256(p.read_bytes()).hexdigest())
