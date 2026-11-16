# Failed initial loopback setup and correction

An initial native factory test intended for private loopback supplied only
ENGINE_* environment overrides. F06 deliberately ignores those overrides when
an engine is absent from its default-entry set. Open Library had no explicit
YAML entry, so its adapter-local public endpoint was used. This violated the
no-live-provider scope. Its outputs are excluded from qualification.

The retained `evidence/a09-factories.json` is the failed initial run. It observed
three completed logical Open Library GET outcomes: one search of `a & b` and two
searches of `test`. From the executed code and adapter request contract, the
endpoint was `openlibrary.org/search.json`, with `q=a+%26+b&limit=10` once and
`q=test&limit=10` twice. Open Library is public; those requests carried no API key
or Authorization credential. No packet capture was taken: exact socket,
redirect/proxy or upstream request counts beyond the three logical calls are
unknown. Precise start/end timestamps were not captured. The failed harness
reported 2.13 seconds total test runtime; filesystem/tool records are timing
bounds, not an invented request trace. No reliability or source evidence uses
those responses.

The first corrective rerun added explicit YAML entries for all five engines and
asserted exact resolved loopback endpoints before dispatch; it completed before
the parent's stronger guard-review message arrived. Subsequent runs use parsed
HTTP/IPv4-loopback/explicit-port/no-URL-credential guards before registration,
validate both archive fallback overrides, reject proxy environment, and audit
Darwin system proxy enable flags fail-closed. All five Darwin enable flags were
zero in the retained guarded run. Negative controls execute the same guard and
prove registration/dispatch counters remain zero for public base, missing YAML,
external archive fallback and proxy environment cases. Shared runtime proxy
semantics were not changed for testing. The parent reviewed the correction.

The first replay's 157-difference failure summary remains in
`evidence/a09-replay5.json`; its detailed temporary mismatch file was overwritten
by a later diagnostic run before immutable naming was added. That missing
intermediate artifact is a limitation. Later per-process full native outcomes
and mismatches are immutable; every final source/native/request comparison is
independently rerun by the pinned native gate. Original failure logs are retained.
