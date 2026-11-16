"""Source strict-mode classifications; valid text remains in original full corpus."""

import argparse
import hashlib
import json
import sys
import zlib
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--iso", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
if sys.version_info[:3] != (3, 14, 7):
    raise SystemExit("requires3.14.7")
for name, pin in [
    ("manifest.json", "57fe4849a880e6a4c38c74b40e4902c2354766fb2f4c3f93b6fe21de6e0c9f22"),
    ("targeted.json", "a826a2e510b754b8ca03ce82024c34c01653d4ff97a8cab7fca1ea9a457fe199"),
]:
    if hashlib.sha256((a.iso / name).read_bytes()).hexdigest() != pin:
        raise SystemExit("original state corpus drift")


def outcome(body, name):
    try:
        value = str(body, name, "strict")
        if value != str(body, name, "replace"):
            raise SystemExit("strict valid text differs replacement")
        return 1
    except UnicodeError:
        return 0
    except RuntimeError as e:
        if str(e) != "internal codec error":
            raise
        return 2


rows = []
a.output.mkdir(exist_ok=True)
for i, row in enumerate(json.loads((a.iso / "manifest.json").read_text())):
    prefix = bytes.fromhex(row["prefix_hex"])
    modes = bytes(outcome(prefix + n.to_bytes(2, "big") + b"\x1b(B", row["codec"]) for n in range(65536))
    file = "iso-" + str(i) + ".modes.zlib"
    data = zlib.compress(modes, 9)
    (a.output / file).write_bytes(data)
    rows.append(
        {
            "codec": row["codec"],
            "prefix_hex": row["prefix_hex"],
            "source_file": row["file"],
            "file": file,
            "counts_unicode_valid_runtime": [modes.count(n) for n in range(3)],
            "sha256": hashlib.sha256(data).hexdigest(),
        }
    )
for i in range(2):
    prefix = b"~{" if i else b""
    modes = bytes(outcome(prefix + n.to_bytes(2, "big") + b"~}", "hz") for n in range(65536))
    file = "hz-" + str(i) + ".modes.zlib"
    data = zlib.compress(modes, 9)
    (a.output / file).write_bytes(data)
    rows.append(
        {
            "codec": "hz",
            "prefix_hex": prefix.hex(),
            "file": file,
            "counts_unicode_valid_runtime": [modes.count(n) for n in range(3)],
            "sha256": hashlib.sha256(data).hexdigest(),
        }
    )
target = json.loads((a.iso / "targeted.json").read_text())
modes = bytes(outcome(bytes.fromhex(r["body_hex"]), r["codec"]) for r in target)
(a.output / "iso-targeted.modes.zlib").write_bytes(zlib.compress(modes, 9))
(a.output / "manifest.json").write_text(
    json.dumps(
        {
            "python": sys.version,
            "states": rows,
            "targeted_count": len(target),
            "targeted_sha256": hashlib.sha256((a.output / "iso-targeted.modes.zlib").read_bytes()).hexdigest(),
            "classification": {
                "0": "caught UnicodeError",
                "1": "success, exact full text in original source corpus",
                "2": "propagated RuntimeError: internal codec error",
            },
        },
        indent=2,
    )
    + "\n"
)
print(json.dumps({"states": len(rows), "pairs": len(rows) * 65536, "targeted": len(target)}))
