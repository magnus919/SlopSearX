import hashlib
import json
import pathlib
import sys

cases = []


def tagged(v):
    if v is None:
        return {"type": "null"}
    if isinstance(v, bool):
        return {"type": "bool", "value": v}
    if isinstance(v, int):
        return {"type": "int", "value": str(v)}
    if isinstance(v, float):
        return {"type": "float", "value": repr(v)}
    if isinstance(v, str):
        return {"type": "str", "codepoints": list(map(ord, v))}
    if isinstance(v, list):
        return {"type": "list", "items": [tagged(x) for x in v]}
    if isinstance(v, dict):
        return {"type": "dict", "items": [[tagged(k), tagged(x)] for k, x in v.items()]}
    raise TypeError(v)


def add(name, value):
    cases.append({"id": name, "input": tagged(value), "repr": repr(value)})


for name, v in [
    ("null", None),
    ("true", True),
    ("false", False),
    ("bigint", -90071992547409931234567890),
    ("float1", 1.0),
    ("negativezero", -0.0),
    ("floatcutoffsmall", 1e-5),
    ("floatcutofflarge", 1e16),
    ("nan", float("nan")),
    ("infinity", float("inf")),
    ("negativeinfinity", float("-inf")),
    ("emptylist", []),
    ("emptydict", {}),
    ("singlequote", "a'b"),
    ("bothquotes", "a'\"b"),
    ("backslash", "\\"),
    ("scalar", chr(128578)),
    ("separatesurrogates", chr(55357) + chr(56898)),
    ("lonesurrogate", chr(0xD800)),
    ("nested", {"x": [None, True, 1.0, "a\n", chr(0xD800)], "q\"'": {"key": "🙂"}}),
]:
    add(name, v)
for p in [0, 9, 10, 13, 31, 32, 127, 0x85, 0xAD, 0x200B, 0x1D173, 0xE000, 0xFDD0, 0x10FFFF]:
    add("character-" + hex(p), chr(p))
v = "leaf"
for _ in range(99):
    v = [v]
add("depth99", v)
cases[-1]["input"] = {"type": "nested_list", "depth": 99, "leaf": tagged("leaf")}
w = pathlib.Path("/Users/magnus/.codex/worktrees/rust-a04-broad-api-adapters/SlopSearX")
p = pathlib.Path("/private/tmp/a07-source-repr-controls.json")
p.write_text(
    json.dumps(
        {
            "status": (
                "actual Python repr controls; isolated offline; tagged codepoints preserve scalar versus "
                "separate-surrogate identity; temporary historical evidence pending registration"
            ),
            "python": sys.version,
            "source_bindings": {
                f: hashlib.sha256((w / f).read_bytes()).hexdigest()
                for f in ["crates/ssx-infra/src/adapters/source_json.rs", "contracts/unicode/python-text-16.0.0.json"]
            },
            "cases": cases,
        },
        ensure_ascii=True,
        allow_nan=False,
        indent=2,
    )
    + "\n"
)
print(len(cases), hashlib.sha256(p.read_bytes()).hexdigest())
