# Temporary A06 source and qualification tooling

These commands are migration tooling only. They invoke actual source adapter
methods with private HTTPX fixtures; no provider is contacted. Retain captured
JSON after G08 retires the seven exporters. The eight invocations are distinct:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_source.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_source.py --regressions
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_shared.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml_grammar.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml_stress.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_wire.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_json_matrix.py
```

The first four captures preserve139 historical records. Grammar adds165 actual
whole-source records. Stress adds five complete outputs and three explicit
large-field summaries, with deterministic recipes/input hashes; it does not
claim eight complete outputs. Parser replay consumes only applicable records.

`docs/rust-rewrite/a06/oracle/native_qualification.py` is a temporary manual
qualification helper. It compiles an offline native probe and checks inherited
file/socket descriptor closure, stdio, resource limits, and GNU absolute address
space behavior. The required CI driver is now the native `cargo xtask check A06`
gate. The helper was moved from `scripts/rust-a06-native-qualification.py` after
that replacement; historical invocation records retain the original path. G08
must retire this remaining executable Python helper. No backend runtime calls it.

Wire supplement captures63 complete request controls, four complete UTF7/ET error
controls, and every Unicode codepoint of the pinned Python word predicate. Native
query helpers replay169 first-request URLs and nine exact OpenAlex preflight
errors across historical/new observations; adapter lifecycle remains pending.

JSON-operation supplement adds198 complete actual OpenAlex/SemanticScholar
outputs, including143 source successes and55 classified source errors. These
are source controls, not native implementation or fixture-count acceptance.

Shared charset source control command (temporary A06/A05/G08 ownership):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_charset.py
```

See [selector design and strict-decoder obligations](HTTP_CHARSET.md).

OpenAlex ordering and Python float lexical supplement commands (temporary
A06/G08-owned developer tools):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_openalex_order.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_float_lexical.py
```

Ordering adds172 complete actual OpenAlex adapter outputs:34initial controls,
eight appended NaN-identity/quoted controls and46appended numeric-boundary
controls plus84appended run-boundary/late-error stress controls. Each earlier
complete-record prefix remains unchanged. They include
mixed numeric positions, nested values and NaN runs around sort boundaries. Lexical capture
replays35 fixed text inputs against actual CPython float and records bits or
ValueError text; these are operation controls, not35 complete adapter outputs.
The quoted1,990,012byte citation-count regression uses a compact deterministic
recipe and full input SHA256 rather than a duplicate multi-megabyte fixture.

Full abstract/order operation supplement (same172fixed source inputs, no500char
slice; separate evidence from complete adapter outputs):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_openalex_order.py --full-order
```

See [source sort provenance and boundaries](OPENALEX_SORT.md).

Configuration scalar observations (temporary A06/G08 developer oracle):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_config.py
```

The153 complete source observations preserve max_results and timeout_ms values
without text parsing, Boolean conversion or float truncation. Null/container
cases remain source-only because F06 rejects them; reachable environment floats
include nonfinite values. MockTransport evidence does not qualify real socket
timeouts. Native configuration/request helpers do not establish factory or
complete adapter activation.
