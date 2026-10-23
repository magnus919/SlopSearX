"""Whole-source OpenAlex abstract ordering controls; offline MockTransport only."""

import argparse
import asyncio
import base64
import json
import runpy
from pathlib import Path

from engines.openalex import _reconstruct_abstract

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))


def cases():
    controls = [
        {"b": [0], "a": [0]},
        {"b": [False], "a": [0.0]},
        {"b": [True], "a": [1]},
        {"big": [9007199254740993], "float": [9007199254740992.0]},
        {"negative": [-9007199254740993], "float": [-9007199254740992.0]},
        {"huge": [10**400], "inf": [float("inf")]},
        {"huge": [-(10**400)], "inf": [float("-inf")]},
        {"a": [float("nan")], "b": [0], "c": [-1]},
        {"c": [1], "b": [float("nan")], "a": [0]},
        {"z": [[1, 2]], "a": [[1, 2]]},
        {"z": [[1, 2]], "a": [[1]]},
        {"z": [[1, "x"]], "a": [[1, 2]]},
        {"z": [{"a": 1}], "a": [{"a": 1}]},
        {"z": [{"a": 1}], "a": [{"a": 2}]},
        {"z": [None], "a": [None]},
        {"z": [None], "a": [0]},
        {"z": ["2"], "a": ["10"]},
        {"z": "ba", "a": "ab"},
        {"z": {"2": None, "1": None}, "a": ["1"]},
        {"z": [], "a": []},
    ]
    for size in (63, 64, 65, 127, 128, 129, 257):
        controls.append({f"w{i:03}": [float("nan") if i % 11 == 0 else (size - i) % 17] for i in range(size)})
        controls.append({f"w{i:03}": [[float("nan") if i % 11 == 0 else (size - i) % 17, i % 3]] for i in range(size)})
    # Raw json NaN constants share CPython decoder identity. Quoted strings
    # remain strings; no relevance-score float coercion participates in sorting.
    controls.extend(
        [
            {"z": [float("nan")], "a": [float("nan")]},
            {"z": [[float("nan")]], "a": [[float("nan")]]},
            {"z": [[float("nan"), 1]], "a": [[float("nan"), 0]]},
            {"z": [{"x": float("nan")}], "a": [{"x": float("nan")}]},
            {"z": ["NaN"], "a": [float("nan")]},
            {"z": [["NaN"]], "a": [[float("nan")]]},
            {"z": [{"x": "NaN"}], "a": [{"x": float("nan")}]},
            {"z": ["nan"], "a": ["NaN"]},
        ]
    )
    largest = float.fromhex("0x1.fffffffffffffp+1023")
    numeric = [
        (2**53 - 1, float(2**53)),
        (2**53, float(2**53)),
        (2**53 + 1, float(2**53)),
        (-(2**53 + 1), -float(2**53)),
        (2**63 - 1, float(2**63)),
        (2**63, float(2**63)),
        (2**63 + 1, float(2**63)),
        (-(2**63 + 1), -float(2**63)),
        (int(largest) - 1, largest),
        (int(largest), largest),
        (int(largest) + 1, largest),
        (-int(largest) - 1, -largest),
        (-int(largest), -largest),
        (-int(largest) + 1, -largest),
        (0, 0.1),
        (0, -0.1),
        (-1, -0.1),
        (1, 1.5),
        (-1, -1.5),
        (0, -0.0),
        (0, 0.0),
        (10**4299, largest),
        (-(10**4299), -largest),
    ]
    for left, right in numeric:
        controls.append({"left": [left], "right": [right]})
        controls.append({"right": [right], "left": [left]})
    for size in (3, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 256, 257, 511, 512, 513, 1024):
        state = 0x5A17
        random_positions = []
        for _ in range(size):
            state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
            random_positions.append(state % 97)
        patterns = [
            list(range(size, 0, -1)),
            random_positions,
            [((i // 32) % 2) * 1000 + i % 32 for i in range(size)],
            [float("nan") if i % 13 == 0 else random_positions[i] for i in range(size)],
        ]
        for pattern in patterns:
            controls.append({f"s{i:04}": [position] for i, position in enumerate(pattern)})
    for bad_at in (0, 1, 31, 32, 63, 64, 127, 128):
        controls.append({f"bad{i:03}": ["bad" if i == bad_at else i % 17] for i in range(129)})
        controls.append({f"nested{i:03}": [[i % 3, "bad" if i == bad_at else i % 17]] for i in range(129)})
    for index, inverted in enumerate(controls):
        yield {
            "id": f"openalex.order.{index}",
            "engine": "openalex",
            "query": "test",
            "responses": [
                BASE["reply"]({"results": [{"title": "Title", "id": "W1", "abstract_inverted_index": inverted}]})
            ],
        }


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--full-order", action="store_true")
    args = parser.parse_args()
    if args.full_order:
        records = []
        for case in cases():
            data = json.loads(base64.b64decode(case["responses"][0]["body_b64"]))
            try:
                outcome = {"abstract": _reconstruct_abstract(data["results"][0]["abstract_inverted_index"])}
            except Exception as error:  # noqa: BLE001 - actual source outcome
                outcome = {"error": {"type": type(error).__name__, "message": str(error)}}
            records.append({"id": case["id"], **outcome})
        scope = "direct actual-source full abstract/order operation; not complete adapter outputs"
    else:
        records = [await BASE["capture"](case) for case in cases()]
        scope = "complete actual-source OpenAlex abstract ordering outcomes"
    print(
        json.dumps(
            {"schema_version": 1, "scope": scope, "records": records}, ensure_ascii=False, indent=2, allow_nan=False
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
