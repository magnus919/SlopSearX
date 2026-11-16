"""Actual original HTML adapters, offline HTTPX header selection; temporary G08 oracle.

Retains complete raw exchange bytes, ordered duplicate header pairs, requests,
resolved configuration, exceptions and normalized outcomes. No provider calls.
"""

import argparse
import asyncio
import hashlib
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("a05_export", Path(__file__).with_name("export.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


async def main(output):
    baseline = json.loads((oracle.ROOT / "docs/rust-rewrite/a05/corpus.json").read_text())
    for source in baseline["source_files"]:
        if hashlib.sha256((oracle.ROOT / source["path"]).read_bytes()).hexdigest() != source["sha256"]:
            raise SystemExit("A05 source drift: " + source["path"])
    rows = []
    variants = [
        ("quoted", [["content-type", 'text/html; charset="cp437"']], "cp437"),
        ("extended", [["content-type", "text/html; charset*=us-ascii''cp437"]], "cp437"),
        ("continued", [["content-type", "text/html; charset*0*=us-ascii''cp; charset*1*=437"]], "cp437"),
        ("mixed_case", [["Content-Type", "text/html; CHARSET=CP437"]], "cp437"),
        (
            "duplicate_first",
            [["content-type", "text/html; charset=cp437"], ["content-type", "text/html; charset=utf-8"]],
            "cp437",
        ),
        (
            "duplicate_reverse",
            [["content-type", "text/html; charset=utf-8"], ["content-type", "text/html; charset=cp437"]],
            "utf-8",
        ),
        ("param_duplicate", [["content-type", "text/html; charset=cp437; charset=utf-8"]], "cp437"),
        ("unknown", [["content-type", "text/html; charset=unknown-a05"]], "utf-8"),
        ("module_dotted", [["content-type", "text/html; charset=utf.8"]], "utf-8"),
        ("strict_fallback", [["content-type", "text/html; charset*=utf-7''utf-7%2B%21"]], "utf-7"),
        ("nonascii_parameter", [["content-type", "text/html; charset*=utf-8''cp%CE%B1437"]], "utf-8"),
        ("empty", [["content-type", "text/html; charset=cp437"]], "empty"),
        ("integer_limit", [["content-type", "text/html; charset*" + "9" * 4301 + "=utf-8"]], "utf-8"),
    ]
    for engine in ["duckduckgo", "google"]:
        for name, headers, encoding in variants:
            body = (
                (
                    '<div class="result"><a class="result__a" href="https://example.t'
                    'est/cafe">Café</a><span class="result__snippet">Résumé</span></d'
                    "iv>"
                )
                if engine == "duckduckgo"
                else (
                    '<div class="g"><a href="https://example.test/cafe"><h3>Café</h3>'
                    '</a><span class="aCOpRe">Résumé</span></div>'
                )
            )
            raw = b"" if encoding == "empty" else body.encode(encoding)
            exchanges = [] if engine == "google" else [oracle.ex(oracle.HOME)]
            exchanges.append(dict(status=200, headers=headers, body_hex=raw.hex()))
            # Preserve an actual available source fallback rather than guessing
            # whether selection/parser failure or a valid empty body consumes it.
            if engine == "duckduckgo":
                exchanges.append(oracle.ex(oracle.LITE))
            case = dict(
                id=f"charset_pipeline_{engine}_{name}",
                engine=engine,
                query="test query",
                max_results=10,
                exchanges=exchanges,
                provenance="actual original adapter HTTPX selector/header/body/parse pipeline; offline synthetic input",
            )
            await oracle.capture(case)
            rows.append(case)
    baseline = json.loads((oracle.ROOT / "docs/rust-rewrite/a05/corpus.json").read_text())
    packet = dict(
        source_commit=baseline["source_commit"],
        source_files=baseline["source_files"],
        oracle_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        packages=baseline["packages"],
        cases=rows,
    )
    output.write_text(json.dumps(packet, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    asyncio.run(main(parser.parse_args().output))
