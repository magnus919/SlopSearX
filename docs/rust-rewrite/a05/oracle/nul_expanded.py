import asyncio  # noqa: I001 - pinned oracle import behavior
import importlib.util
import json
import subprocess
import struct
import hashlib  # noqa: F401 - pinned oracle import behavior
import argparse
from pathlib import Path

args = argparse.ArgumentParser()
args.add_argument("--native214", required=True)
args.add_argument("--native215", required=True)
args.add_argument("--output", type=Path, required=True)
args = args.parse_args()
p = Path(__file__).with_name("export.py")
s = importlib.util.spec_from_file_location("oracle", p)
o = importlib.util.module_from_spec(s)
s.loader.exec_module(o)
import httpx  # noqa: E402, F401 - pinned oracle import behavior
import lxml.html  # noqa: E402 - pinned oracle import behavior


async def main():
    rows = []
    for engine, mode in [("google", "text"), ("duckduckgo", "text"), ("duckduckgo", "images")]:
        if engine == "google":
            base = '<div class="g"><a href="https://x"><h3>AXB</h3></a><span class="st">S</span></div>'
        elif mode == "images":
            base = '<a class="tile" href="https://x"><img src="https://i" alt="AXB"></a>'
        else:
            base = (
                '<div class="result"><a class="result__a" href="https://x">AXB</a'
                '><div class="result__snippet">S</div></div>'
            )
        cases = [
            (tag, f"<{tag}>A\0B</{tag}>" + base) for tag in ["script", "style", "title", "textarea", "xmp", "plaintext"]
        ] + [
            ("tag_name", "<di\0v>" + base + "</di\0v>"),
            ("attribute_name", base.replace("href=", "hr\0ef=")),
            ("doctype", "<!DOCTYPE ht\0ml>" + base),
            ("pi", "<?pi A\0B?>" + base),
            ("unclosed_comment", "<!-- A\0B" + base),
            ("bogus_comment", "<! A\0B>" + base),
            ("cdata", "<![CDATA[A\0B]]>" + base),
            ("unquoted_attribute", base.replace('href="https://x"', "href=https://x/A\0B")),
        ]

        for width, prefix in [("ascii", ""), ("latin1", "é"), ("bmp", "Ā"), ("astral", "😀")]:
            for label, text in cases:
                text = prefix + text
                body = text.encode("utf8")
                reply = {"status": 200, "headers": {"content-type": "text/html;charset=utf-8"}, "body_hex": body.hex()}
                ex = [] if engine == "google" else [o.ex("<html/>")]
                ex.extend([reply] * (2 if engine == "duckduckgo" and mode == "text" else 1))
                c = dict(
                    id=f"nul_expanded_{engine}_{mode}_{width}_{label}",
                    engine=engine,
                    query="q*~",
                    max_results=10,
                    exchanges=ex,
                )
                if mode == "images":
                    c["categories"] = ["images"]
                await o.capture(c)
                points = list(map(ord, text))
                c["decoded_codepoints"] = points
                try:
                    doc = lxml.html.fromstring(text)
                    c["direct_lxml"] = {"root": doc.tag, "text_codepoints": list(map(ord, doc.text_content()))}
                except Exception as e:
                    c["direct_lxml"] = {"type": type(e).__name__, "message": str(e)}
                frame = struct.pack(">Q", len(points)) + b"".join(struct.pack(">I", n) for n in points)
                for version in ["214", "215"]:
                    run = subprocess.run(
                        [getattr(args, "native" + version)], input=frame, capture_output=True, check=True
                    )
                    raw = run.stdout
                    c["native_" + version] = {"tree_utf8_hex": raw.hex(), "stderr_hex": run.stderr.hex()}
                rows.append(c)
    out = {
        "source_commit": "01876639fb8d87123f12293c02821981cff3f8b9",
        "source_files": json.loads((Path(__file__).parent.parent / "corpus.json").read_text())["source_files"],
        "cases": rows,
    }
    args.output.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n")
    print("rows", len(rows), "Ctree differences", sum(c["native_214"] != c["native_215"] for c in rows))
    for c in rows:
        if c["native_214"]["tree_utf8_hex"] != c["native_215"]["tree_utf8_hex"]:
            print(
                c["id"],
                bytes.fromhex(c["native_214"]["tree_utf8_hex"]),
                bytes.fromhex(c["native_215"]["tree_utf8_hex"]),
            )


asyncio.run(main())
