# A12 source semantics and evidence limits

Censys sends Basic key/secret credentials to `/hosts/search` with `q` and
`per_page`; its original parser ignores a nonmapping `result` and does not apply
a local result limit. ASN is read inside `location`, matching the source.
Shodan strips only its key with Python whitespace semantics, sends bearer auth,
and slices matches before parsing. Hostnames slice before join; vulnerability
mapping keys preserve source insertion order. VirusTotal always uses `/search`
with `query`/`limit` and `x-apikey`; hash/IP patterns do not select a new route.
Its parser does not locally slice data. Detection totals retain arbitrary signed
integer digits and compensated mixed-float accumulation before score projection.
VulnCheck requires its key before CVE extraction. The first case-insensitive
`CVE-\d{4}-\d{4,}` match uses Unicode16 decimal digits without word boundaries.
No CVE means empty success without HTTP. Vendors use the first five source rows,
then deduplicate/sort. The result link remains the original fixed API URL.

CRT.sh constructs `/?output=json`, then HTTPX `params={q:...}` replaces that query;
the effective request has `q` alone. No corrective `output=json` parameter was
invented. Status429/403 precede body parsing. HTML MIME or the original first256
byte sniff precedes JSON; markers are searched in the first65536 decoded code
points, in the original nine-marker order. Challenge, other HTML, malformed
JSON, wrong root, wrong row and valid empty list retain distinct outcomes and
exact original CRT.sh messages. Every row is validated before applying the local
slice, including a malformed row beyond a zero limit. SAN content intentionally
uses only the first newline entry. Serial fallback evaluation remains lazy.

No adapter enforces filters or adds pagination semantics absent from its source.
Request presentation max_results/page/language/SafeSearch/date bounds do not
replace provider config or change outbound tuples. The shared policy/filter
pipeline remains authoritative and unchanged.

Generic HTTP/network diagnostics cross the previously accepted redacted F08 error
boundary. Native errors use a constant provider label/status, not reconstructed
HTTPX exception text or provider-body text. These tests prove classification,
empty failures and non-disclosure; no original raw exception output was captured.
Unicode non-ASCII bearer/header keys retain source pre-transport rejection;
Censys Basic credentials first encode UTF-8. Diagnostic spelling is explicit and
must be reviewed, with no tuple-wide normalization or count exemption.

F05 refuses nonfinite scores, nontext typed title/date fields and unpaired
surrogates in public text. Unused malformed scalar values remain legal when
source operations do not touch them. These are whole empty native Errors and
are not silently stringified/null-replaced. They are inherited typed-boundary
applications requiring review for the listed A12 cases, not a newly declared
A12 input domain. No source execution or public-source-wire success is claimed
for those Python internal dataclass values. Mixed source-operation/output-refusal
precedence remains a review focus; the component does not claim exhaustive proof
for every malformed-field cross product.

All source expectations here are static. Passing native fixtures cannot establish
observed Python parity, production injection or provider compatibility. The
unapplied gate is designed to bind source identity, this map and exact compiled
test identities; parent acceptance and current-main integration remain required.
