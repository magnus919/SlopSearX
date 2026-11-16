import json
import unicodedata
from pathlib import Path

names = [
    "LATIN SMALL LETTER E WITH ACUTE",
    "latin small letter e with acute",
    "SNOWMAN",
    "NULL",
    "NUL",
    "BYTE ORDER MARK",
    "KEYCAP DIGIT ONE",
    "LATIN SMALL LETTER A WITH MACRON AND GRAVE",
    "HANGUL SYLLABLE GA",
    "hangul syllable hih",
    "HANGUL SYLLABLE GAX",
    "CJK UNIFIED IDEOGRAPH-4E00",
    "cjk unified ideograph-4e00",
    "CJK UNIFIED IDEOGRAPH-04E00",
    "CJK UNIFIED IDEOGRAPH-4e00",
    "CJK UNIFIED IDEOGRAPH-3400",
    "CJK UNIFIED IDEOGRAPH-31350",
    "CJK UNIFIED IDEOGRAPH-323AF",
    "CJK UNIFIED IDEOGRAPH-323B0",
    "TANGUT IDEOGRAPH-17000",
    "CJK COMPATIBILITY IDEOGRAPH-F900",
    "",
    "SNOWMAN\0",
    " SNOWMAN",
    "SNOWMAN ",
    "NO SUCH NAME",
]
rows = []
for name in names:
    body = b"\\N{" + name.encode("utf8") + b"}"
    rows.append(
        {
            "name": name,
            "body_hex": body.hex(),
            "unicode_escape_codepoints": list(map(ord, body.decode("unicode_escape", "replace"))),
        }
    )
Path(__import__("sys").argv[1]).write_text(
    json.dumps({"unidata_version": unicodedata.unidata_version, "cases": rows}, indent=2) + "\n"
)
