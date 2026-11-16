import asyncio
import importlib.util
import json
from pathlib import Path

p = Path(__file__).with_name("export.py")
s = importlib.util.spec_from_file_location("oracle", p)
o = importlib.util.module_from_spec(s)
s.loader.exec_module(o)
import httpx  # noqa: E402 - pinned oracle import behavior
import lxml.html  # noqa: E402 - pinned oracle import behavior

cases = [
    ("isolated_high", "\ud800"),
    ("isolated_low", "\udc00"),
    ("isolated_pair", "\ud800\udc00"),
    ("title_high", '<div class="g"><a href="https://x"><h3>A\ud800B</h3></a><span class="st">C</span></div>'),
    ("title_low", '<div class="g"><a href="https://x"><h3>A\udc00B</h3></a><span class="st">C</span></div>'),
    ("title_pair", '<div class="g"><a href="https://x"><h3>A\ud800\udc00B</h3></a></div>'),
    ("title_high_bmp", '<div class="g"><a href="https://x"><h3>éA\ud800B</h3></a></div>'),
    ("title_high_astral", '<div class="g"><a href="https://x"><h3>😀A\ud800B</h3></a></div>'),
    ("url_high", '<div class="g"><a href="https://x/\ud800"><h3>X</h3></a></div>'),
    ("image_alt_high", '<a class="tile" href="https://x"><img src="https://i" alt="A\ud800B"></a>'),
    ("image_alt_high_astral", '<a class="tile" href="https://x"><img src="https://i" alt="😀A\ud800B"></a>'),
]


async def main():
    rows = []
    for codec in ["utf-7", "raw_unicode_escape", "unicode_escape", "punycode"]:
        for label, text in cases:
            body = text.encode(codec)
            engine = "duckduckgo" if label.startswith("image") else "google"
            mode = "images" if engine == "duckduckgo" else "text"
            reply = {"status": 200, "headers": {"content-type": f"text/html; charset={codec}"}, "body_hex": body.hex()}
            ex = [] if engine == "google" else [o.ex("<html/>")]
            ex.append(reply)
            c = dict(id=f"surrogate_{codec}_{label}", engine=engine, query="q*~", max_results=10, exchanges=ex)
            if mode == "images":
                c["categories"] = ["images"]
            await o.capture(c)
            decoded = httpx.Response(200, headers=reply["headers"], content=body).text
            c["decoded_codepoints"] = [ord(ch) for ch in decoded]
            c["result_string_codepoints"] = [
                {k: [ord(ch) for ch in value] for k, value in result.items() if isinstance(value, str)}
                for result in c["expected"]["results"]
            ]
            try:
                doc = lxml.html.fromstring(decoded)
                c["direct_lxml"] = {
                    "root": doc.tag,
                    "text": doc.text_content(),
                    "text_codepoints": [ord(ch) for ch in doc.text_content()],
                }
            except Exception as e:
                c["direct_lxml"] = {"exception_type": type(e).__name__, "message": str(e)}
            rows.append(c)
    Path(OUTPUT).write_text(
        json.dumps(
            {
                "source": "actual pinned whole adapters plus HTTPX decode and lxml wrapper",
                "source_commit": o.BASELINE if hasattr(o, "BASELINE") else "01876639fb8d87123f12293c02821981cff3f8b9",
                "source_files": json.loads(
                    (Path(__file__).resolve().parents[4] / "docs/rust-rewrite/a05/corpus.json").read_text()
                )["source_files"],
                "cases": rows,
            },
            indent=2,
            ensure_ascii=True,
        )
        + "\n"
    )
    for c in rows:
        print(
            json.dumps(
                {
                    "id": c["id"],
                    "decoded_max": max(c["decoded_codepoints"], default=0),
                    "direct_lxml": c["direct_lxml"],
                    "outcome": c["expected"],
                },
                ensure_ascii=True,
            )
        )


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--output", required=True)
    OUTPUT = ap.parse_args().output
    asyncio.run(main())
