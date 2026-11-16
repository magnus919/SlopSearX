import json
import random
from pathlib import Path

samples = [
    b"",
    b"\\",
    bytes(range(256)),
    b"\\N{LATIN SMALL LETTER E WITH ACUTE}",
    b"\\N{SNOWMAN}",
    b"\\N{NULL}",
    b"\\N{KEYCAP DIGIT ONE}",
    b"\\N{}",
    b"\\N{",
    b"\\Na",
    b"\\N{NO SUCH NAME}",
    b"\\ud800",
    b"\\udc00",
    b"\\ud800\\udc00",
    b"\\777",
    b"\\400",
]
for marker, n in [(b"x", 2), (b"u", 4), (b"U", 8)]:
    for k in range(n + 1):
        for bad in [b"", b"X", b"\\", b"\xff"]:
            samples.append(b"A\\" + marker + b"0" * k + bad + b"Z")
rng = random.Random(925)
samples += [bytes(rng.randrange(256) for _ in range(rng.randrange(65))) for _ in range(256)]
rows = [
    {"input_hex": b.hex(), "expected_codepoints": list(map(ord, b.decode("unicode_escape", "replace")))}
    for b in samples
]
Path(__import__("sys").argv[1]).write_text(json.dumps(rows, indent=2) + "\n")
print(len(rows))
