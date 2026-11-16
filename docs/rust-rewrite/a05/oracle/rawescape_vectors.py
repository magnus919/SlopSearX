import json
from pathlib import Path

samples = [
    b"",
    b"\\",
    b"\\u",
    b"\\u0",
    b"\\u00x1",
    b"\\U00110000",
    b"\\ud800",
    b"\\udc00",
    b"\\ud800\\udc00",
    b"\\U00010000",
    b"\\x41",
    b"\\n",
    b"\\\\u0041",
    bytes(range(256)),
]
for marker, n in [(b"u", 4), (b"U", 8)]:
    for k in range(n + 1):
        for bad in [b"", b"X", b"\\", b"\xff"]:
            samples.append(b"A\\" + marker + b"0" * k + bad + b"Z")
rows = [
    {"input_hex": b.hex(), "expected_codepoints": list(map(ord, b.decode("raw_unicode_escape", "replace")))}
    for b in samples
]
Path(__import__("sys").argv[1]).write_text(json.dumps(rows, indent=2) + "\n")
print(len(rows))
