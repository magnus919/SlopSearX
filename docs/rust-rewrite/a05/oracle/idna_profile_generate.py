"""Offline candidate generator, no codec/runtime backend. Not adopted."""

"""Candidate independent Unicode input derivation; not runtime integration."""
import argparse  # noqa: E402, I001 - pinned oracle import behavior
import ast  # noqa: E402 - pinned oracle import behavior
import functools  # noqa: E402 - pinned oracle import behavior
import json  # noqa: E402 - pinned oracle import behavior
import hashlib  # noqa: E402 - pinned oracle import behavior
from pathlib import Path  # noqa: E402 - pinned oracle import behavior

parser = argparse.ArgumentParser()
parser.add_argument("--inputs", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
root = args.inputs
pins = {
    "UnicodeData-3.2.0.txt": "5e444028b6e76d96f9dc509609c5e3222bf609056f35e5fcde7e6fb8a58cd446",
    "CompositionExclusions-3.2.0.txt": "1d3a450d0f39902710df4972ac4a60ec31fbcb54ffd4d53cd812fc1200c732cb",
    "UnicodeData-16.0.0.txt": "ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f",
    "SpecialCasing-16.0.0.txt": "8d5de354eef79f2395a54c9c7dcebbaf3d30fc962d0f85611ea97aa973a0c451",
    "stringprep.py": "60b6c83581093029312efb6670b11c540090b3f78bcf72264467b494f02f21a5",
}
for name, pin in pins.items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != pin:
        raise SystemExit("IDNA profile input drift: " + name)


def ucd(path):
    out = {}
    start = None
    for line in path.read_text().splitlines():
        f = line.split(";")
        c = int(f[0], 16)
        if f[1].endswith(", First>"):
            start = (c, f)
            continue
        if f[1].endswith(", Last>"):
            a, x = start
            for n in range(a, c + 1):
                out[n] = x
            start = None
        else:
            out[c] = f
    return out


old = ucd(root / "UnicodeData-3.2.0.txt")
new = ucd(root / "UnicodeData-16.0.0.txt")
excluded = {
    int(l.split("#")[0].strip(), 16)
    for l in (root / "CompositionExclusions-3.2.0.txt").read_text().splitlines()  # noqa: E741 - preserve source variable spelling
    if l.split("#")[0].strip()
}
ccc = {c: int(f[3]) for c, f in old.items() if int(f[3])}
decomp = {c: [int(x, 16) for x in f[5].split() if not x.startswith("<")] for c, f in old.items() if f[5]}
compose = {
    tuple(decomp[c]): c
    for c, f in old.items()
    if c in decomp
    and not f[5].startswith("<")
    and len(decomp[c]) == 2
    and c not in excluded
    and ccc.get(decomp[c][0], 0) == 0
}
lower = {c: [int(f[13], 16)] for c, f in new.items() if f[13]}
for line in (root / "SpecialCasing-16.0.0.txt").read_text().splitlines():
    line = line.split("#")[0].strip()
    if not line:
        continue
    f = [x.strip() for x in line.split(";")]
    if not f[4]:
        lower[int(f[0], 16)] = [int(x, 16) for x in f[1].split()]
module = ast.parse((root / "stringprep.py").read_text())
exceptions = next(
    ast.literal_eval(n.value)
    for n in module.body
    if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name) and n.targets[0].id == "b3_exceptions"
)


@functools.lru_cache(None)
def expand(c):
    if 0xAC00 <= c < 0xD7A4:
        i = c - 0xAC00
        return (0x1100 + i // 588, 0x1161 + (i % 588) // 28) + ((0x11A7 + i % 28,) if i % 28 else ())
    return tuple(x for n in decomp.get(c, [c]) for x in (expand(n) if n != c else (n,)))


def pair(a, b):
    if 0x1100 <= a < 0x1113 and 0x1161 <= b < 0x1176:
        return 0xAC00 + ((a - 0x1100) * 21 + b - 0x1161) * 28
    if 0xAC00 <= a < 0xD7A4 and (a - 0xAC00) % 28 == 0 and 0x11A8 <= b < 0x11C3:
        return a + b - 0x11A7
    return compose.get((a, b))


def nfkc(seq):
    out = []
    for c in seq:
        for p in expand(c):
            out.append(p)
            i = len(out) - 1
            k = ccc.get(p, 0)
            while k and i and ccc.get(out[i - 1], 0) > k:
                out[i], out[i - 1] = out[i - 1], out[i]
                i -= 1
    if not out:
        return []
    result = [out[0]]
    starter = 0
    last = ccc.get(out[0], 0)
    for c in out[1:]:
        k = ccc.get(c, 0)
        joined = pair(result[starter], c)
        if joined is not None and (last < k or last == 0):
            result[starter] = joined
        else:
            if k == 0:
                starter = len(result)
            result.append(c)
            last = k
    return result


def b3(c):
    return list(map(ord, exceptions[c])) if c in exceptions else lower.get(c, [c])


def b2(c):
    a = b3(c)
    b = nfkc(a)
    d = nfkc([y for x in b for y in b3(x)])
    return d if b != d else a


def literal(node):
    if isinstance(node, (ast.List, ast.Tuple, ast.Constant)):
        return ast.literal_eval(node)
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
        return literal(node.left) + literal(node.right)
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
        values = [literal(v) for v in node.args]
        if node.func.id == "range":
            return range(*values)
        if node.func.id == "list":
            return list(*values)
        if node.func.id == "set":
            return set(*values)
    raise SystemExit("unexpected pinned set expression")


sets = {
    n.targets[0].id: literal(n.value)
    for n in module.body
    if isinstance(n, ast.Assign)
    and isinstance(n.targets[0], ast.Name)
    and n.targets[0].id in ["b1_set", "c22_specials", "c6_set", "c7_set", "c8_set", "c9_set"]
}
b2maps = []
ranges = []
lastflag = None
start = 0
for c in range(0x110000):
    mapped = b2(c)
    if mapped != [c]:
        b2maps.append([c, mapped])
    f = old.get(c)
    category = f[2] if f else "Cn"
    bidi = f[4] if f else ""
    prohibited = (
        (category == "Zs" and c != 32)
        or (c >= 128 and (category == "Cc" or c in sets["c22_specials"]))
        or category in ["Co", "Cs"]
        or 0xFDD0 <= c < 0xFDF0
        or (c & 0xFFFF) in [0xFFFE, 0xFFFF]
        or any(c in sets[k] for k in ["c6_set", "c7_set", "c8_set", "c9_set"])
    )
    flag = int(c in sets["b1_set"]) | (int(prohibited) << 1) | (int(bidi in ["R", "AL"]) << 2) | (int(bidi == "L") << 3)
    if flag != lastflag:
        if lastflag:
            ranges.append([start, c, lastflag])
        start = c
        lastflag = flag
if lastflag:
    ranges.append([start, 0x110000, lastflag])
out = {
    "inputs": pins,
    "b2": b2maps,
    "compatibility_decomposition": sorted(decomp.items()),
    "combining_classes": sorted(ccc.items()),
    "canonical_composition": [[*k, v] for k, v in sorted(compose.items())],
    "profile_ranges": ranges,
    "hangul": "algorithmic Unicode3.2 decomposition/composition; no table",
}
data = (json.dumps(out, separators=(",", ":"), ensure_ascii=True) + "\n").encode()
if hashlib.sha256(data).hexdigest() != "5d6d1c88e4ccf80e10da18bb60d01d7d12616f9eb8c96100b255f2360c10bd8d":
    raise SystemExit("IDNA profile output drift")
args.output.write_bytes(data)
print(
    json.dumps(
        {
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "b2_records": len(b2maps),
            "profile_ranges": len(ranges),
        }
    )
)
