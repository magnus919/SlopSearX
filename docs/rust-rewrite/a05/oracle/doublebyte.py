import argparse  # noqa: I001 - pinned oracle import behavior
import struct
import json
import random
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
codecs = [
    "cp932",
    "gbk",
    "cp949",
    "cp950",
    "big5",
    "big5hkscs",
    "gb2312",
    "johab",
    "shift_jis",
    "shift_jis_2004",
    "shift_jisx0213",
    "euc_jp",
    "euc_jis_2004",
    "euc_jisx0213",
    "euc_kr",
]
manifest = []
for codec in codecs:
    single = []
    for byte in range(256):
        try:
            text = bytes([byte]).decode(codec, "strict")
            assert len(text) == 1
            single.append(ord(text))
        except UnicodeDecodeError:
            single.append(0xFFFFFFFF)
    mapping = bytearray()
    expected = bytearray()
    for pair in range(65536):
        raw = pair.to_bytes(2, "big")
        text = raw.decode(codec, "replace")
        assert 1 <= len(text) <= 2
        points = list(map(ord, text))
        expected += struct.pack(">BII", len(points), points[0], points[1] if len(points) == 2 else 0xFFFFFFFF)
        # HKSCS source output is oracle evidence only; native tables are derived
        # separately from WHATWG/ICU plus the validity-only mask.
        if codec == "big5hkscs" or single[raw[0]] != 0xFFFFFFFF:
            continue
        try:
            text = raw.decode(codec, "strict")
        except UnicodeDecodeError as error:
            if error.start == 0 and error.end == 2:
                text = "\ufffd"
            else:
                continue
        assert 1 <= len(text) <= 2
        mapping += struct.pack(">HII", pair, ord(text[0]), ord(text[1]) if len(text) == 2 else 0xFFFFFFFF)
    if codec in ["euc_jp", "euc_jis_2004", "euc_jisx0213"]:
        triples = bytearray()
        expected3 = bytearray()
        for pair in range(65536):
            raw = b"\x8f" + pair.to_bytes(2, "big")
            text = raw.decode(codec, "replace")
            points = list(map(ord, text))
            assert 1 <= len(points) <= 3
            expected3 += struct.pack(">BIII", len(points), *(points + [0xFFFFFFFF] * 3)[:3])
            try:
                text = raw.decode(codec, "strict")
            except UnicodeDecodeError:
                continue
            assert 1 <= len(text) <= 2
            triples += struct.pack(">HII", pair, ord(text[0]), ord(text[1]) if len(text) == 2 else 0xFFFFFFFF)
        (args.output / (codec + ".triples.bin")).write_bytes(triples)
        (args.output / (codec + ".source-triples.bin")).write_bytes(expected3)
    if codec != "big5hkscs":
        (args.output / (codec + ".single.bin")).write_bytes(b"".join(struct.pack(">I", p) for p in single))
        (args.output / (codec + ".pairs.bin")).write_bytes(mapping)
    (args.output / (codec + ".source-pairs.bin")).write_bytes(expected)
    rng = random.Random(925)
    rows = []
    for _ in range(256):
        raw = bytes(rng.randrange(256) for _ in range(rng.randrange(129)))
        rows.append([raw.hex(), list(map(ord, raw.decode(codec, "replace")))])
    (args.output / (codec + ".source-streams.json")).write_text(json.dumps(rows, separators=(",", ":")) + "\n")
    manifest.append(
        {
            "canonical": codec,
            "pairs": None if codec == "big5hkscs" else len(mapping) // 10,
            "single": sum(p != 0xFFFFFFFF for p in single),
        }
    )
    print(codec, "pairs", len(mapping) // 10)
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
