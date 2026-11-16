import argparse  # noqa: I001 - pinned oracle import behavior
import json
import struct
import zlib
import random
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
configs = {
    "iso2022_kr": [b"", b"\x0e", b"\x1b$)C\x0e", b"\x1b$(C"],
    "iso2022_jp": [b"", b"\x1b$B", b"\x1b$@", b"\x1b(J"],
    "iso2022_jp_1": [b"", b"\x1b$B", b"\x1b$@", b"\x1b(J", b"\x1b$(D"],
    "iso2022_jp_2": [
        b"",
        b"\x1b$B",
        b"\x1b$@",
        b"\x1b(J",
        b"\x1b$(D",
        b"\x1b$(C",
        b"\x1b$A",
        b"\x1b(A",
        b"\x1b(F",
        b"\x1b.A",
        b"\x1b.F",
        b"\x1b.J",
    ],
    "iso2022_jp_2004": [b"", b"\x1b$B", b"\x1b$(Q", b"\x1b$(P"],
    "iso2022_jp_3": [b"", b"\x1b$B", b"\x1b$(O", b"\x1b$(P"],
    "iso2022_jp_ext": [b"", b"\x1b$B", b"\x1b$@", b"\x1b(J", b"\x1b(I", b"\x1b$(D"],
}
manifest = []
targeted = []


def observe(raw, codec):
    try:
        return {"points": list(map(ord, raw.decode(codec, "replace")))}
    except Exception as error:
        return {"error": {"type": type(error).__name__, "message": str(error)}}


for codec, prefixes in configs.items():
    for state, prefix in enumerate(prefixes):
        name = f"{codec}-{state}"
        compress = zlib.compressobj(9)
        parts = []
        errors = {}
        for pair in range(65536):
            value = observe(prefix + pair.to_bytes(2, "big") + b"\x1b(B", codec)
            if "error" in value:
                errors[str(pair)] = value["error"]
                row = struct.pack(">BIIIII", 255, *([0xFFFFFFFF] * 5))
            else:
                points = value["points"]
                assert len(points) <= 5
                row = struct.pack(">BIIIII", len(points), *(points + [0xFFFFFFFF] * 5)[:5])
            part = compress.compress(row)
            if part:
                parts.append(part)
        parts.append(compress.flush())
        (args.output / (name + ".zlib")).write_bytes(b"".join(parts))
        (args.output / (name + ".errors.json")).write_text(json.dumps(errors, separators=(",", ":")) + "\n")
        manifest.append(
            {
                "codec": codec,
                "prefix_hex": prefix.hex(),
                "case_count": 65536,
                "file": name + ".zlib",
                "errors": name + ".errors.json",
            }
        )
        for third in range(256):
            raw = prefix + b"\x1bN" + bytes([third])
            targeted.append({"codec": codec, "body_hex": raw.hex(), **observe(raw, codec)})
    rng = random.Random(925)
    rawcases = [
        b"\x1b" + bytes([head, mark]) + tail
        for head in b"()$.&"
        for mark in range(256)
        for tail in [b"", b"A", b"\x1b(B"]
    ]
    rawcases += [b"\x1b&@\x1b$B" + b"A" * n for n in range(5)]
    rawcases += [b"\x1b" + b"!" * n + tail for n in range(20) for tail in [b"", b"@", b"Z", b"\xff"]]
    rawcases += [bytes(rng.randrange(256) for _ in range(rng.randrange(129))) for _ in range(1024)]
    for raw in rawcases:
        targeted.append({"codec": codec, "body_hex": raw.hex(), **observe(raw, codec)})
    print(codec, len(prefixes), flush=True)
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
(args.output / "targeted.json").write_text(json.dumps(targeted, separators=(",", ":")) + "\n")
print("states", len(manifest), "targeted", len(targeted), flush=True)
