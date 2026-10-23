"""Recapture fixed Python float lexical operation controls, not adapter outputs."""

import json
import platform
import struct
from pathlib import Path


def main():
    source = Path(__file__).resolve().parents[1] / "source-float-lexical.json"
    inputs = [row["text"] for row in json.loads(source.read_text())["records"]]
    records = []
    for text in inputs:
        try:
            outcome = {"bits": struct.pack(">d", float(text)).hex()}
        except ValueError as error:
            outcome = {"error": str(error)}
        records.append({"text": text, **outcome})
    print(
        json.dumps(
            {
                "schema_version": 1,
                "scope": (
                    "actual CPython float lexical conversion bits/errors; "
                    "operation controls, not complete adapter outputs"
                ),
                "python_version": platform.python_version(),
                "records": records,
            },
            ensure_ascii=False,
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
