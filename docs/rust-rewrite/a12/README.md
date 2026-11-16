# A12 infrastructure security adapter candidate

A12 implements Censys, Shodan, VirusTotal, VulnCheck and CRT.sh through the
accepted SearchEngine, Registry, SearchAdmission and caller-owned AdapterHttp
ports. The six adapter source files and one component test target are local to
A12. Shared module exports and the native gate are included in this review branch.
No application/portal composition or production availability is claimed.

The implementation baseline is `3571e499b9d5da8f5ddface32b6ebc3cfcb9a06c`.
A01/#917 was already accepted and closed; its merged interface contract is the
prerequisite, with the current absolute SearchContext extension retained.
Fresh main `80ecfcf5bcd7393bbe1457fa5f406d439525e4f8` is the continuation
integration baseline. The normal current-main integration retains the existing
A14 jobs registration; production composition remains separate.

The accepted historical private candidate has 27 native tests: the original 23
plus four source-selection regressions. The two runtime corrections select date
prefixes and the first SAN line before scalar projection, and include vertical
tab in CRT.sh byte-regex whitespace. Three causal reverts failed their specific
whole-response assertions. This is historical private evidence at the baseline
above. The subsequent current-main component qualification passed 14 commands
on d4c4ad1e, including the closed 27-case A12 gate. The current gate adds a 28th
case for the intentional CRT.sh JSON-query correction described in PARITY.md;
prior-head evidence is not relabeled as qualification of that change.

`source-test-map-v1.json` pins five original engines, all 33 named original test
functions, five F06 fragments and shared contract sources. Every original test
maps to a specific compiled native test and its concrete assertions. The five
`static-fixtures-v1.json` payloads are derived from inline named original tests.
They are **static derivations**, not executed Python observations. No A12 Python
backend, oracle, new original capture, live/paid provider, telemetry or production
call was made. Existing F04/F06 artifacts establish the prior contract/catalog;
they do not contain executed A12 search observations to relabel as this evidence.

The component target path-includes the exact six A12 source files, independently
of the additive shared module exports. It exercises the real built-in
registry descriptors and the real five-engine factory pack. Success comparisons
cover the entire normalized AdapterResponse and every SearchResult field; latency
is separately required finite/nonnegative and normalized to zero for comparison.
Injected tests assert the exact method, complete URL/query, custom-header map and
empty body. A bounded child additionally observes five actual private-loopback
HTTP requests, with proxies/CA environment removed. Its raw stdout/stderr is
losslessly base64 embedded in the native parent log; it does not call providers.

Deadline/admission, actual in-flight cancellation/shutdown, caller HTTP survival,
wrong-shape atomic failure, original CRT.sh classifications, numeric/container/
Unicode operations, unchanged filter and paging behavior, inherited F05 refusal,
and scoped projection charge release have native controls. This is component
qualification, not a Python replay, global memory proof or final issue acceptance.
See [PARITY.md](PARITY.md), [BOUNDS_REVIEW.md](BOUNDS_REVIEW.md) and
[PORTAL_IMPACT.md](PORTAL_IMPACT.md).
