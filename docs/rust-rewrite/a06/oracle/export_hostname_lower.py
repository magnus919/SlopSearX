"""Temporary offline Python Unicode16 hostname-lower oracle; retire at G08.

No network or runtime fallback. Captured JSON alone feeds native controls.
CaseIgnorable is observed through CPython Final_Sigma, including characters
that are both Cased and CaseIgnorable; category approximations are not used.
"""

import hashlib
import inspect
import json
import sys
import unicodedata
import urllib.parse
from pathlib import Path


def require(value, message):
    if not value:
        raise RuntimeError(message)


def ranges(points):
    result = []
    for point in points:
        if result and point == result[-1][1] + 1:
            result[-1][1] = point
        else:
            result.append([point, point])
    return result


require(sys.version_info[:3] == (3, 14, 7), "pinned Python 3.14.7 required")
require(unicodedata.unidata_version == "16.0.0", "pinned Unicode16 required")
lower = []
cased = []
ignorable = []
for point in range(0x110000):
    char = chr(point)
    mapped = char.lower()
    if mapped != char:
        lower.append([point, [ord(c) for c in mapped]])
    is_cased = char.islower() or char.isupper() or unicodedata.category(char) == "Lt"
    if is_cased:
        cased.append(point)
        is_ignorable = ("-" + char + "Σ").lower().endswith("σ")
    else:
        is_ignorable = ("a" + char + "Σ").lower().endswith("ς")
    if is_ignorable:
        ignorable.append(point)

# Entire mapping and both property boundaries participate in context controls.
points = set(cased + ignorable + [0x41, 0x3A3, 0x105C0, 0x1C89, 0x10D50])
records = []
for point in sorted(points):
    if 0xD800 <= point <= 0xDFFF:
        continue
    char = chr(point)
    for text in [char, "A" + char + "Σ", "AΣ" + char, "AΣ" + char + "B"]:
        records.append([text, text.lower()])
for text in ["AΣ", "AΣ.B", "AΣ'B", "AΣ\u0301", "İΣ", "AΣ%ZONEΣ", "É.TEST%ZONE", "A%ΣZONE%NEXT"]:
    records.append([text, urllib.parse.urlsplit("https://" + text).hostname])
paths = [
    Path(__file__),
    Path(inspect.getsourcefile(urllib.parse)),
    Path(unicodedata.__file__),
    Path(sys.executable).resolve(),
]
print(
    json.dumps(
        {
            "schema_version": 1,
            "purpose": "temporary offline Unicode16 source oracle; retire exporter G08; native consumes JSON only",
            "python": sys.version,
            "unicode": unicodedata.unidata_version,
            "source_sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},
            "lower": lower,
            "cased": ranges(cased),
            "case_ignorable": ranges(ignorable),
            "records": records,
        },
        ensure_ascii=True,
        indent=2,
    )
)
