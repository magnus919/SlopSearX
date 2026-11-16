# F05 wire and durable codec evidence

F05 records pinned Python 3.14.7 observations from source revision
`16f03be656f34d90edd8285f9c81dfb1c4626b0a`. The corpus contains storage
result/response reads and writes, HTTP JSON and YAML-facing result mappings,
MCP result cards, media URL sanitization, canonical JSON, stable identities,
and Unicode 16 case folding. Inputs and identities are synthetic and contain
no credentials, personal records, or provider responses. The repository's MIT
license applies to source; the committed corpus is a compact, source-derived
contract artifact.

The Python exporter is developer-only. It imports the pinned source checkout,
requires a clean tracked and untracked tree with no ignored Python bytecode,
uses an allowlisted environment and isolated temporary home, and makes no
network or shared-store calls. Its output records source Git blob IDs and the
exporter SHA-256. Re-capture with the repository's pinned Python 3.14.7
Unicode 16 environment:

```sh
git worktree add --detach /private/tmp/slopsearx-f05-oracle-src 16f03be656f34d90edd8285f9c81dfb1c4626b0a
/private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f05/oracle/export.py \
  --source-root /private/tmp/slopsearx-f05-oracle-src \
  --baseline 16f03be656f34d90edd8285f9c81dfb1c4626b0a \
  --output crates/xtask/fixtures/f05/corpus.json
```

The native acceptance command is `cargo xtask check F05`. It validates source
blob pins and the Unicode table digest, then compares Rust codec/projection
outputs with the committed observations and runs the focused crate tests. The
native gate never invokes Python, providers, or a live store. These fixtures
verify serialization and projection boundaries only; they do not establish
backend parity, live HTTP/MCP routing behavior, provider quality, or external
retrieval safety. Typed Rust JSON cannot represent Python object cycles or
lone surrogates, and those source-runtime cases are outside this corpus.

HTTP JSON cases carry `pubdate` and `parsed_url` metadata captured from the
pinned formatter. F05 checks DTO mapping only; it does not claim native date or
URL parser parity. T02 owns complete formatter behavior and actual HTTP
integration coverage. MCP retrieval classification is likewise caller-supplied
typed metadata here; F05 does not claim classifier parity, which belongs to
F11's complete URL classification work.

Refresh the exporter and corpus together when changing cases. Review the
resulting bytes and provenance changes before updating any inventory or pin.

Publication metadata is validated against the pinned source: DOI prefixes use
Python Unicode case-insensitive matching, decimal registrants and full Unicode 16
casefolding. DOI/list limits count characters; scalar Paper strings count UTF-8
bytes; volume accepts strings and integers only. Source fixtures cover all 76
Unicode decimal digit ranges and the distinct length/type boundaries.

The lossless codec currently parses raw JSON twice to enforce bounds and preserve
number tokens, and inline payload checks serialize the bounded object before
measuring its size. These are correctness-first implementation choices; their
performance has not been measured and no throughput claim is made.
