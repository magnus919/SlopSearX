"""Offline implementation/table bound inventory; no decoder/oracle backend."""

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[4]
package = root / "crates/ssx-source-codecs"
policy = json.loads((root / "docs/rust-rewrite/a05/source-codec-policy.json").read_text())
single_source = (package / "src/singlebyte.rs").read_text()
singles = set(re.findall(r'"([^"]+)" => Some', single_source))
if len(singles) != 69:
    raise SystemExit("single-byte family drift")
cjk = {p.name.removesuffix(".pairs.bin") for p in (package / "src/cjk").glob("*.pairs.bin")}
if len(cjk) != 16:
    raise SystemExit("CJK family drift")
rejected = {"base64", "bz2", "hex", "quopri", "rot-13", "uu", "zlib", "idna", "undefined"}
rows = []
for entry in policy["canonical_codecs"]:
    name = entry["canonical"]
    if name in rejected:
        continue
    tables = []
    if name in singles:
        family = "single-byte-table"
        files = ["src/singlebyte.rs"]
        arm = re.search(r'"' + re.escape(name) + r'" => Some\(&([^)]*)\)', single_source).group(1)
        contents = re.search(r"static " + arm + r": \[u32; 256\] = \[(.*?)\];", single_source, re.S).group(1)
        points = [int(value, 16) for value in re.findall(r"0x[0-9a-f]+", contents)]
        if len(points) != 256 or any(point > 0x10FFFF for point in points):
            raise SystemExit("single-byte mapping invariant failed: " + name)
        tables.append(dict(name=arm, entries=256, consumed_bytes=1, maximum_emitted_points=1))
    elif name in cjk:
        family = "CJK"
        files = ["src/doublebyte.rs"]
        if name == "gb18030":
            files.append("src/gb18030_ranges.rs")
        for suffix, consumed in [("pairs", 2), ("triples", 3)]:
            path = package / f"src/cjk/{name}.{suffix}.bin"
            if not path.exists():
                continue
            raw = path.read_bytes()
            if len(raw) % 10:
                raise SystemExit("invalid mapping frame")
            maximum = 0
            for key, first, second in struct.iter_unpack(">HII", raw):
                points = [first] + ([] if second == 0xFFFFFFFF else [second])
                if any(point > 0x10FFFF for point in points):
                    raise SystemExit("invalid mapping point")
                maximum = max(maximum, len(points))
            files.append(str(path.relative_to(package)))
            tables.append(
                dict(
                    name=str(path.relative_to(package)),
                    entries=len(raw) // 10,
                    consumed_bytes=consumed,
                    maximum_emitted_points=maximum,
                )
            )
        files.append(f"src/cjk/{name}.single.bin")
    elif name.startswith("iso2022"):
        family = "ISO2022"
        files = ["src/iso2022.rs"]
        for path in sorted((package / "src/iso2022").glob("*.bin")):
            raw = path.read_bytes()
            if len(raw) % 10:
                raise SystemExit("invalid ISO mapping frame")
            maximum = 0
            for key, first, second in struct.iter_unpack(">HII", raw):
                points = [first] + ([] if second == 0xFFFFFFFF else [second])
                if any(point > 0x10FFFF for point in points):
                    raise SystemExit("invalid ISO point")
                maximum = max(maximum, len(points))
            files.append(str(path.relative_to(package)))
            tables.append(
                dict(
                    name=str(path.relative_to(package)),
                    entries=len(raw) // 10,
                    consumed_bytes=2,
                    maximum_emitted_points=maximum,
                )
            )
    elif name.startswith("utf-"):
        family = "Unicode"
        files = ["src/unicode.rs"] + (["src/utf7.rs"] if name == "utf-7" else [])
    elif name in {"ascii", "charmap", "iso8859-1"}:
        family = "single-byte-algorithm"
        files = ["src/lib.rs"]
    elif name in {"hz", "punycode", "raw-unicode-escape", "unicode-escape"}:
        family = name
        files = ["src/" + name.replace("-", "_") + ".rs"]
        if name == "unicode-escape":
            files += ["src/unicode_names.rs", "src/unicode_name_ranges.rs"]
    else:
        raise SystemExit("unclassified applicable codec: " + name)
    if any(table["maximum_emitted_points"] > table["consumed_bytes"] for table in tables):
        raise SystemExit("table expansion violation")
    files = sorted(set(files + ["src/lib.rs", "src/aliases.rs", "src/text.rs"]))
    rows.append(
        dict(
            canonical=name,
            labels=entry["labels"],
            family=family,
            implementation_files=[
                dict(path=file, sha256=hashlib.sha256((package / file).read_bytes()).hexdigest()) for file in files
            ],
            tables=tables,
            point_bound="completed output codepoints <= consumed body bytes; algorithm actions require review",
        )
    )
if len(rows) != 108:
    raise SystemExit("applicable codec inventory drift")
args.output.write_text(
    json.dumps(
        dict(
            status="implementation argument and actual table invariant inventory; not RSS or full parity qualification",
            canonical_count=108,
            rejected_canonical=sorted(rejected),
            codecs=rows,
        ),
        ensure_ascii=False,
        indent=2,
    )
    + "\n"
)
