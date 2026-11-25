# Native gateway IDNA profile

The optional `source-idna2008` feature implements Python idna 3.18
with IDNA 17 classes and CPython 3.14.7 Unicode 16 properties. It
carries the strict hostname encoding and display decoding from the
historical gateway candidate. It uses the existing `ssx-json`
Unicode 16 lowercase implementation. Both its implementation and
data match the reused part of the historical source text codec.
Port parsing already has a Unicode 16 decimal value helper in
`ssx-json`, so the generated codec table omits the duplicate
decimal table.

The inputs in `input-pins.json` bind the retained IDNA source and
license, the CPython name index and license, and the reused Unicode
16 inputs. The compressed name index expands to the exact historical
bytes. The generator parses pinned literals and data for offline
reproduction. The native A05 gate checks the pinned input, archive and assembled-table hashes. The generator runs only during manual reproduction.

To reproduce the generated Rust table, run
`python3 crates/ssx-source-codecs/provenance/idna2008/generate.py.txt --output <scratch-file>`
and then `rustfmt --edition 2024 <scratch-file>`. Compare the result
with the assembled table hash in the manifest. The checked-in table
is split before the `CCC` declaration. Concatenating
`idna2008_data.rs` without its final `include!` line and
`idna2008_normalization_data.rs` reproduces the formatted result. The formatted result matches the historical table after
removing the unused decimal table. The retained `.py.txt` files are
provenance evidence for T07. The generator is a manual maintenance
tool.

The lockfile adds two edges from `ssx-source-codecs` to existing
`idna` and `ssx-json` packages. Before historical T08A, W07 and
shared source verification, the T08A reader checks the physical
lock and reconstructs the exact previous bytes. The default builds
continue to use the existing IDNA 2003 implementation. A05 runs
the six feature-enabled gateway codec tests separately from its
default suite.

[Historical gateway source](https://github.com/magnus919/SlopSearX/tree/884d50f222792cbce82d282fef01afec33d93191/crates/ssx-source-codecs)
