"""Explicit CJK strict error keys from validity bits and approved native record keys.
No Unicode value is interpreted or used as an error marker.
"""

import argparse
import hashlib
import json
import struct
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--validity", type=Path, required=True)
p.add_argument("--tables", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
a = p.parse_args()
validity_pins = {
    "big5.valid.bin": "201cfa80cfa171862275fb0fb563a6c00365ece9dcd33083f43ee658c9d845de",
    "big5hkscs.valid.bin": "3f9768f72d44d19d81fa457c7c665d32216a244b7abb48b2a6cf590dc70be427",
    "cp932.valid.bin": "a9a3ef55573d7b0b766dc7417909a545d88a93f7a1743c8e8895de579050b831",
    "cp949.valid.bin": "b06c8bc05fd5669196112a47d35cd94f66cfbb01d9fb0fb02ce7da677cdb2d78",
    "cp950.valid.bin": "c20ecf9596783f209e77fd46cf85eb487e123e82b3bb2e0028a98244611bba20",
    "euc_jis_2004.valid.bin": "db97d929e170a4ec6586af6a6fe861e302708281d78b1b883e2eaaa2ec4653ff",
    "euc_jisx0213.valid.bin": "016282e27fd801c079699617d157ccc4b42af2f94aa317f0ce0beeffa248bbee",
    "euc_jp.valid.bin": "c2dbaa67564b660ebf7f6354b27514ffae809d88aaf06914a3b1f7957576311d",
    "euc_kr.valid.bin": "fe13607d63c43b780ba123719992190b86d7a563128543c0799cd192a284de30",
    "gb18030.valid.bin": "386d2f7dc0a4bb1ed0986fb8fc1db5f24171271ea0c0458bd627fea6076d0de6",
    "gb2312.valid.bin": "e6b42fc00fdf3a4e0c84c12b55a8b6604fef8fae5d61ec59c3bf0a9efec0412d",
    "gbk.valid.bin": "97394e6cf8290be58e44ab558b9502abdee448115f0d76656f17817f5503a4c2",
    "johab.valid.bin": "130522f4a7d1c0f1c73b51f82ae51021329d025510b9e9e4e8627e735b555ceb",
    "manifest.json": "c57d28fdd62595c4ae5c6b63562c16ade6ab0b16384678993f3952ed61683453",
    "shift_jis.valid.bin": "02c2dde4fe2ec12884b2c4662db56eae154c1ee8bab28c81b8ec1a20f6c48789",
    "shift_jis_2004.valid.bin": "48baf3ca251a7568f4f5c8633233cef3de66174d723409a274489cce80b06722",
    "shift_jisx0213.valid.bin": "ab13f21e2b3451626f206ca4761677c95f283ed0a34997039c03834526f71a50",
}
table_pins = {
    "big5.pairs.bin": "603594bb1e79752f25b50d72ff5ce0f3ae88a942e8c81a36f82e3daa2b4ca3dc",
    "big5hkscs.pairs.bin": "fc9461564284bac32fca61da4fb07b5b8a144b6943092425f0ac51876d1b344a",
    "cp932.pairs.bin": "b51e5e105b1e9b777b5fc0ddc080860a8444881ac40533e2b1669d1b414007ac",
    "cp949.pairs.bin": "53d7b966f810d955efb6cb92f3280126ae412e0688a4e46b444ee662626a95d4",
    "cp950.pairs.bin": "89af3c2176761d1bb76b8c61fed4877ab467f3b552bcd9c3bc99a28f6139da89",
    "euc_jis_2004.pairs.bin": "68d20bf66d1aa2f509e9e2b1dc1401a3dd9e1ef7219674a4076cf8ec45f004fc",
    "euc_jisx0213.pairs.bin": "1cc39aebc4c4bf90d9b7e431bdf0a306104437d98a7e3b159c6b28765535c7a8",
    "euc_jp.pairs.bin": "ff25787a4fd0f23e987023188444cbdeb494ec6985eacaace939afbdc71c12f5",
    "euc_kr.pairs.bin": "4c113fb80dd7ff04e62417887f0d61241083ad47becae4bfab78dc22736157df",
    "gb18030.pairs.bin": "ccc6e1e022d66c88cc16ac18de3d94595095b496d866d30136c2097b100cb566",
    "gb2312.pairs.bin": "0c62f0718d021cc5f7d0f9aebe12c62813db620673b17477d00b295ee5c55339",
    "gbk.pairs.bin": "5ef8bf4c9643a792ddfc6dc0e6f70fc99453afd8d846a31009dd5f89988572fb",
    "johab.pairs.bin": "864edf098cefd7e2f6c3eb6ee4e936f315e7c1314c9049cd809dec5d2b73ecc9",
    "shift_jis.pairs.bin": "76cc259c1fd8eb8e39d668d72422a9180e6c533a81ed2967c6202d5c0205c4c0",
    "shift_jis_2004.pairs.bin": "559304f05cf5d6ff468c6169f2222c67d2653f669a6282494d2c3ca2c4f1b927",
    "shift_jisx0213.pairs.bin": "59d22cc9f01036288d8640d3ed643df25aea1611ac86b29367500cc428cbfb2b",
}

for name, pin in validity_pins.items():
    if hashlib.sha256((a.validity / name).read_bytes()).hexdigest() != pin:
        raise SystemExit("strict validity input drift " + name)
for name, pin in table_pins.items():
    if hashlib.sha256((a.tables / name).read_bytes()).hexdigest() != pin:
        raise SystemExit("native key input drift " + name)
out = [
    (
        "// Explicit strict error records; actual validity-only source ma"
        "sks retained.\n// No U+FFFD marker inference occurs in the native"
        " decoder.\npub fn pair_error(name:&str,pair:u16)->bool {let value"
        "s:&[u16]=match name {\n"
    )
]
for row in json.loads((a.validity / "manifest.json").read_text())["cases"]:
    name = row["canonical"]
    bits = (a.validity / (name + ".valid.bin")).read_bytes()
    keys = []
    for key, _, _ in struct.iter_unpack(">HII", (a.tables / (name + ".pairs.bin")).read_bytes()):
        if not bits[32 + key // 8] & (1 << (key % 8)):
            keys.append(key)
    if keys:
        out.append(json.dumps(name) + "=> &[" + ",".join(map(str, keys)) + "],\n")
out.append("_=>return false,\n};values.binary_search(&pair).is_ok()}\n")
a.output.write_text("".join(out))
