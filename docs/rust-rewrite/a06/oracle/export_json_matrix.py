"""Actual offline scholarly source operations; no native-output normalization."""

import asyncio
import hashlib
import json
import runpy
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))
VALUES = [None, False, True, 0, 1, "", " x ", [], {}, [None], {"key": 1}]


def cases():
    for engine, collection, fields in [
        (
            "openalex",
            "results",
            ["title", "id", "doi", "publication_date", "relevance_score", "abstract_inverted_index"],
        ),
        (
            "semanticscholar",
            "data",
            ["title", "url", "abstract", "citationCount", "publicationDate", "externalIds", "authors"],
        ),
    ]:
        for index, value in enumerate(VALUES + [[{}], ["x"], [{}, None]]):
            yield engine, f"collection_{index}", {collection: value}
        for field in fields:
            for index, value in enumerate(VALUES):
                yield engine, f"{field}_{index}", {collection: [{field: value}]}
    for index, value in enumerate(
        [
            {"b": [0], "a": [0]},
            {"w": "10"},
            {"w": {"b": 1, "a": 0}},
            {"a": [True, False]},
            {"a": [None]},
            {"a": [[1], [0]]},
            {"a": [{}, {}]},
            {"a": [1, "0"]},
            {"a": []},
            {"a": [1.5, 0.5]},
        ]
    ):
        yield "openalex", f"inverted_{index}", {"results": [{"abstract_inverted_index": value}]}
    for index, value in enumerate(
        [
            [{"name": "A"}, {"name": "B"}, {"name": "C"}, {"name": 1}],
            [{"name": 1}],
            [{"name": None}],
            [{}, {"name": "B"}],
            [None, False, [], {}, {"name": "A"}],
            [{"name": "A"}, {"name": "B"}, {"name": "C"}, {"name": {"nested": [1]}}],
        ]
    ):
        yield "semanticscholar", f"authors_operation_{index}", {"data": [{"authors": value}]}
    for index, ids in enumerate(
        [
            {"DOI": []},
            {"DOI": {"nested": [1, True]}},
            {"ArXiv": [1, False]},
            {"PubMed": 1, "PubMedCentral": ["X"]},
            {"ArXiv": {"nested": 2}},
        ]
    ):
        yield "semanticscholar", f"identifier_{index}", {"data": [{"externalIds": ids}]}
    # Source errors in later records must outrank a pending native refusal for
    # earlier source-success title/date/url values; never commit partial rows.
    for engine, collection, bad in [
        ("openalex", "results", {"abstract_inverted_index": {"word": None}}),
        ("semanticscholar", "data", {"authors": [{"name": None}]}),
    ]:
        for field in (
            ["title", "id", "publication_date"] if engine == "openalex" else ["title", "url", "publicationDate"]
        ):
            yield engine, f"deferred_{field}_late_source_error", {collection: [{field: {"opaque": 1}}, bad]}


async def main():
    records = []
    for engine, name, body in cases():
        records.append(
            await BASE["capture"](
                {"id": f"{engine}.matrix.{name}", "engine": engine, "query": "test", "responses": [BASE["reply"](body)]}
            )
        )
    print(
        json.dumps(
            {
                "schema_version": 1,
                "plan_id": "A06",
                "normalization": (
                    "latency_ms omitted; sets sorted; enum values serialized; no other output normalization"
                ),
                "source_sha256": {
                    p: hashlib.sha256((BASE["ROOT"] / p).read_bytes()).hexdigest()
                    for p in ["engines/openalex.py", "engines/semanticscholar.py", "slopsearx/payload.py"]
                },
                "records": records,
            },
            ensure_ascii=False,
            indent=2,
            allow_nan=False,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
