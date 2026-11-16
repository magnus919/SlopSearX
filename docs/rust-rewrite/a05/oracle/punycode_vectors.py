import random  # noqa: I001 - pinned oracle import behavior
import json
from pathlib import Path

rng = random.Random(925)
samples = [b"", b"-", b"abc-", b"z" * 256 + b"a", b"9" * 256 + b"a", b"\xff-", b"abc!"]
samples += [bytes(rng.randrange(256) for _ in range(rng.randrange(65))) for _ in range(1024)]
for _ in range(1024):
    text = "".join(chr(rng.randrange(0x110000)) for _ in range(rng.randrange(17)))
    samples.append(text.encode("punycode"))
rows = [{"input_hex": b.hex(), "expected_codepoints": list(map(ord, b.decode("punycode", "replace")))} for b in samples]
Path(__import__("sys").argv[1]).write_text(json.dumps(rows, indent=2) + "\n")
print(len(rows))
