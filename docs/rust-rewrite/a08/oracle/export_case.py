"""Temporary FDA Unicode16 producer, retired G08. No network/native runtime role."""

import hashlib
import json
import pathlib
import sys
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[4]
assert sys.version_info[:3] == (3, 14, 7)
assert unicodedata.unidata_version == "16.0.0"
# Reuse the accepted A06 operational property capture, independent of hostname policy.
REF = pathlib.Path("/private/tmp/a06-hostname-lower-freeze/after/docs/rust-rewrite/a06/source-hostname-lower.json")
assert (
    hashlib.sha256(REF.read_bytes()).hexdigest() == "db70a80f98f2758da47030752c826a42ee97aac12e482a7145a11f5efe15d1df"
)
ref = json.loads(REF.read_text())
mapping = {name: [] for name in ["upper", "title", "lower"]}
points = set()
for point in range(0x110000):
    if 0xD800 <= point <= 0xDFFF:
        continue
    c = chr(point)
    for name in mapping:
        mapped = getattr(c, name)()
        if mapped != c:
            mapping[name].append([point, mapped])
            points.add(point)
for key in ["cased", "case_ignorable"]:
    for a, b in ref[key]:
        points.update([a, b])
queries = [
    "Σ",
    "AΣ",
    "AΣ\u0301",
    "AΣ\u0301B",
    "AΣ.B",
    "AΣ'B",
    "AΣ\u0345",
    "AΣ\u0345B",
    "they're BILL'S",
    "ßtraße",
    "ﬃ",
    "ǳELTA",
    "İΣ",
    "𐵐 test",
]
for p in sorted(points):
    queries.extend([chr(p), "AΣ" + chr(p), "AΣ" + chr(p) + "B"])
records = []
for text in queries:
    n = text.strip()
    values = list(dict.fromkeys([n, n.casefold(), n.upper(), n.title(), n.capitalize()])) if n else []
    records.append([text, values])
out = {
    "schema_version": 1,
    "purpose": "FDA exact-query variants; native data only; producer retires G08",
    "python": sys.version,
    "unicode": unicodedata.unidata_version,
    "interpreter_sha256": hashlib.sha256(pathlib.Path(sys.executable).resolve().read_bytes()).hexdigest(),
    "property_capture_sha256": hashlib.sha256(REF.read_bytes()).hexdigest(),
    "producer_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
    **mapping,
    "cased": ref["cased"],
    "case_ignorable": ref["case_ignorable"],
    "records": records,
}
data = {k: v for k, v in out.items() if k != "records"}
for key in ["upper", "title", "lower"]:
    data[key] = {str(p): v for p, v in data[key]}
d = ROOT / "docs/rust-rewrite/a08/case16-data.json"
d.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n")
controls = {k: v for k, v in out.items() if k not in ["upper", "title", "lower", "cased", "case_ignorable"]}
controls["data_sha256"] = hashlib.sha256(d.read_bytes()).hexdigest()
p = ROOT / "docs/rust-rewrite/a08/case16.json"
p.write_text(json.dumps(controls, ensure_ascii=False, separators=(",", ":")) + "\n")
print(len(records), hashlib.sha256(p.read_bytes()).hexdigest(), controls["data_sha256"])
