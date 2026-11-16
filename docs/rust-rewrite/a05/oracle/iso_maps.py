import argparse  # noqa: I001 - pinned oracle import behavior
import struct
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
configs = [
    ("jis0208", "iso2022_jp", b"\x1b$B"),
    ("jis0212", "iso2022_jp_1", b"\x1b$(D"),
    ("ksx1001", "iso2022_kr", b"\x1b$)C\x0e"),
    ("gb2312", "iso2022_jp_2", b"\x1b$A"),
    ("jis0213_2000_1", "iso2022_jp_3", b"\x1b$(O"),
    ("jis0213_2000_2", "iso2022_jp_3", b"\x1b$(P"),
    ("jis0213_2004_1", "iso2022_jp_2004", b"\x1b$(Q"),
    ("jis0213_2004_2", "iso2022_jp_2004", b"\x1b$(P"),
]
manifest = []
for name, codec, prefix in configs:
    output = bytearray()
    for first in range(0x20, 0x80):
        for second in range(256):
            raw = prefix + bytes([first, second]) + b"\x1b(B"
            try:
                text = raw.decode(codec, "strict")
            except UnicodeError:
                continue
            assert 1 <= len(text) <= 2
            output += struct.pack(
                ">HII", (first << 8) | second, ord(text[0]), ord(text[1]) if len(text) == 2 else 0xFFFFFFFF
            )
    (args.output / (name + ".bin")).write_bytes(output)
    manifest.append({"map": name, "codec": codec, "prefix_hex": prefix.hex(), "records": len(output) // 10})
    print(name, len(output) // 10)
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
