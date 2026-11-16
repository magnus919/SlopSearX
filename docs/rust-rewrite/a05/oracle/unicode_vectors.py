import argparse  # noqa: I001 - pinned oracle import behavior
import json
import zlib
import random
from httpx._decoders import TextDecoder
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
manifest = []
for codec in ["utf-7", "utf-8", "utf-8-sig", "utf-16", "utf-16-le", "utf-16-be", "utf-32", "utf-32-le", "utf-32-be"]:
    samples = [b"", bytes(range(256))] + [bytes([b]) for b in range(256)]
    if codec in ["utf-7", "utf-8", "utf-8-sig", "utf-16-le", "utf-16-be"]:
        samples += [n.to_bytes(2, "big") for n in range(65536)]
    if codec == "utf-7":
        samples += [chr(n).encode("utf7") for n in range(65536)]
    if codec.startswith("utf-16"):
        order = "big" if codec.endswith("be") else "little"
        bom = b"\xff\xfe" if codec == "utf-16" else b""
        for high in range(0xD800, 0xDC00):
            for tail in [b"", b"\x00", b"\x7f", b"\xff"]:
                samples.append(bom + high.to_bytes(2, order) + tail)
    if codec.startswith("utf-32"):
        order = "big" if codec.endswith("be") else "little"
        bom = b"\xff\xfe\0\0" if codec == "utf-32" else b""
        for n in [
            0,
            1,
            0x7F,
            0x80,
            0xFF,
            0x100,
            0xD7FF,
            0xD800,
            0xDBFF,
            0xDC00,
            0xDFFF,
            0xE000,
            0xFFFF,
            0x10000,
            0x10FFFF,
            0x110000,
            0xFFFFFFFF,
        ]:
            for size in range(5):
                samples.append(bom + n.to_bytes(4, order)[:size])
    rng = random.Random(925)
    samples += [bytes(rng.randrange(256) for _ in range(rng.randrange(129))) for _ in range(1024)]
    rows = []
    for raw in samples:
        try:
            decoder = TextDecoder(codec)
            text = decoder.decode(raw) + decoder.flush()
            result = {"points": list(map(ord, text))}
        except Exception as error:
            result = {"error": {"type": type(error).__name__, "message": str(error)}}
        rows.append({"body_hex": raw.hex(), **result})
    file = codec + ".json.zlib"
    (args.output / file).write_bytes(zlib.compress(json.dumps(rows, separators=(",", ":")).encode(), 9))
    manifest.append({"codec": codec, "file": file, "cases": len(rows)})
    print(codec, len(rows), flush=True)
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
