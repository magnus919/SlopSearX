# A11 source contract and explicit differences

Each replay compares complete normalized public fields, classified status,
error message, answers, infoboxes, corrections and synthetic flag, plus the full
ordered method/URL/custom-header/body request tuple and unused exchanges. It does
not compare result counts alone. Source latency is the only capture field removed.

Routing follows the source: OTX prefers IPv4, then hash, then CVE, then pulses;
URLhaus prefers URL, then MD5, then an IPv4 prefix, then the first hostname token.
Python Unicode16 word membership, Unicode decimal digits and original regex
boundaries are preserved, including the optional URL `s` matching U+017F long-s
under Python ignore-case. Slice-before-parse and engine-specific missing-credential,
optional GreyNoise community access and admission ordering remain observable.

`native-differences-v1.json` contains 370 exact identity-bound differences. All
method/full-URL/body/custom-header tuples remain exact. Differences in native
versioned User-Agent and transport-added host, encoding, connection and content
length headers use a closed five-name vocabulary; no arbitrary header is ignored.
Thirteen named real private-loopback rows verify complete wire requests and full
public outcomes. Their private endpoint/base-prefix substitution is explicit;
these are not calls to the original provider hosts or wire proof for all injected
source records.

Sixty diagnostic-only differences bind the complete expected and native messages
and every other output field. Eleven nested source helper exceptions bind the
actual associated search parent identity and exact full parent outcome. They do
not expose a new standalone native helper exception API or substitute null.
Eight nonpositive source timeouts bind strict classified native Timeout, zero
requests and the exact unused source exchange. Original MockTransport behavior
is not evidence of real Python network deadline enforcement. There is no tiny
positive deadline or scheduling exemption in A11.

Four original malformed-result cases and ten additional representation cases are
explicit whole-response F05 refusals. They preserve complete original reference
output and native Error with zero partial results: OTX pulse names of None/int,
URLhaus numeric URL/list date, six nonfinite score strings and four lone-surrogate
content values. No malformed field is silently stringified or sanitized to claim
parity. Tagged nonfinite scores and escaped lone surrogates in the original
reference describe the original internal API; they are not valid public numeric
scores or proof of source public wire serialization.

`config-entry-differences-v1.json` is a separate entry-layer contract. Four valid
original synthetic YAML values (None, list, dict and a 100-digit integer) actually
survive load → asdict → normalizer → OTX object construction. Accepted native F06
rejects the first three as invalid engine settings and the integer as invalid
configuration YAML, before A11 settings/factory construction. This preserves the
approved typed F06 contract; A11 adds no raw settings API. That oracle performed
no search, HTTP request or slicing call and makes no adapter/wire parity claim.

Owned operation tests observe actual IOC/projection work before cancellation or
expiry, typed allocation refusal and return of all scoped charges. Private socket
tests observe requests before cancellation/shutdown/deadline and join owned work.
These establish per-operation scope, not global RSS, throughput or measured speed.

Four additional credential controls preserve the original rejection of non-ASCII
string header values before transport. The supported F06 Unicode string reaches
all four native factories without changing config validation. Each original
returns full Error after a real HTTPX ASCII UnicodeEncodeError, zero transport
calls and joined cleanup. Native uses the existing fixed credential diagnostic
without echoing the key and otherwise preserves the exact full Error response.
The four explicit bindings live in `credential-refusal-bindings-v1.json`; these
are separate from the unchanged 370-entry replay ledger and F05/F06 refusals.
