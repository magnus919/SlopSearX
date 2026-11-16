"""Actual whole-source long-query controls; offline, temporary G08 oracle."""

import argparse
import asyncio
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("a05_oracle", Path(__file__).with_name("export.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


async def main(output):
    rows = []
    for size in [501, 4097, 16385]:
        query = ("q*~ Café😀 +%&" * (size // 12 + 1))[:size]
        for mode in ["google", "ddg-text", "ddg-images"]:
            engine = "google" if mode == "google" else "duckduckgo"
            body = oracle.G["SAMPLE_HTML"] if mode == "google" else oracle.PRIMARY
            if mode == "ddg-images":
                body = '<a class="tile" href="https://example.test/image"><img src="https://example.test/p.png"></a>'
            exchanges = [] if engine == "google" else [oracle.ex(oracle.HOME)]
            exchanges.append(oracle.ex(body))
            case = dict(
                id=f"long_query_{mode}_{size}",
                engine=engine,
                query=query,
                max_results=10,
                exchanges=exchanges,
                provenance="actual original adapter; long query, exact UTF8 quote_plus and image fallback",
            )
            if mode == "ddg-images":
                case["categories"] = ["images"]
            await oracle.capture(case)
            rows.append(case)
    baseline = json.loads((Path(__file__).resolve().parents[4] / "docs/rust-rewrite/a05/corpus.json").read_text())
    output.write_text(
        json.dumps(
            dict(source_commit=baseline["source_commit"], source_files=baseline["source_files"], cases=rows),
            ensure_ascii=False,
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    asyncio.run(main(parser.parse_args().output))
