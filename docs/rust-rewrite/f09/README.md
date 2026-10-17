# F09 policy, filters and scope

The source anchor is accepted main `1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`.
`source-pins.json` binds the twelve current implementation/test sources. The native
F09 gate independently pins source observations and static profile tables, verifies
source blobs, requires named regressions and runs native policy/composition tests.
Mandatory checks never run Python or call providers.

The temporary developer oracle runs with the pinned Python 3.14.7 environment:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f09/oracle/export.py
```

Its 79 observations cover explicit/category/topic/media precedence, exclusion
stages, routing bounds, sensitive atomicity, truthful layers, publication bounds
and relative dates. Five additional cases capture actual config loading into
first-match routing with named topics, duplicate keys, aliases, merges and lists.
G08 retires executable reference Python before backend removal; retained JSON
remains migration evidence. The first temporary date capture used an incorrect
source function signature; this was retained as red evidence and corrected to
`published_date_within(value, "week", fixed_today)` before qualification. Reviewed
expected answers were not changed to accommodate a native mismatch.

`PolicyService` is the one shared gate. `ScopeProposal` is untrusted input;
`ScopePreview` is explanation only. Only `authorize` constructs `ValidatedScope`,
which cannot be deserialized or defaulted. Preview cannot obtain one. Delayed work
calls `with_revalidated` against fresh policy/catalog/routing inputs. It retains
the original authority and compares the complete current scope and enforcement;
a changed scope requires new admission/budgeting. Raw Worker proposals are denied.
The callback receives a private read-only validated command including resolved
request, original origin and specialist parameters. The callback is the caller's
execution/commit boundary: later concurrent revocation requires cooperative
cancellation. It is not an atomic distributed policy snapshot or a W02 runner.

HTTP authority preserves source explicit-engine unknown dropping and explicit
sensitive access; MCP authority validates unknown/inactive engines and applies
the independent sensitive grant atomically. Specialist tool grants never imply a
sensitive grant. Automatic scopes exclude sensitive engines and apply auth,
circuit and cost/count routing; explicit scopes bypass automatic eligibility as
the source does. Adapter execution remains responsible for its own auth/circuit
checks. Accepted F06 config and registered factory availability are projected in
`ssx-server`'s pure library, callable by worker composition without server I/O.
Enabled configuration, actual active membership and executable availability are
separate signals. All unported factories fail dispatch authorization, even when
preview metadata describes them. Snapshots retain only credential presence.

Only audited `enforced_filters` declarations count. Local enforcement is limited
to the implemented relative publication-date postfilter and recognized range;
SafeSearch requires every selected source to declare upstream enforcement.
The native shared envelope additionally reports the typed request page through
the audited pagination resolver. Python public tools currently omit that core
entry; the source helper supports it. This additive native report never treats
snapshot slicing or cursor creation as upstream pagination enforcement. No
snapshot cursor or pagination execution is invented here. Jobs location/employment remain
unsupported and are carried as validated parameters. Publication bounds are
strict canonical inclusive calendar dates. No date is inferred from a URL or
other metadata. Optional Jev/provider routing is outside this pure primitive;
future advice must be submitted as a fresh proposal through the same gate.

## Deliberate native boundaries

Tier-1 fallback is sorted deterministically; Python iterates a set whose order is
process dependent. Oracle comparison normalizes only that selected fallback
order. Other source list orders and duplicates are preserved. A missing router
is `topics=None` (all active); a present disabled router is `Some(empty)`
(tier-1 fallback). Malformed topic containers/list entries fail typed native
configuration instead of leaking Python exceptions. Programmatic named maps must
supply original key order; ordered list topics need no sidecar.

F09 corrects an accepted F06 ordering gap: the private bounded YAML composer now
retains insertion order before converting ordinary config to the unchanged
public JSON shape. Named-topic order is extracted after alias/merge resolution;
duplicate keys replace values without moving first position. Original scalar,
credential, alias-depth and copy-budget behavior remains. The topic-name copy is
charged before allocation. The change uses already-pinned pure indexmap locally;
no global serde_json preserve_order feature changes canonical record codecs.

Portal impact: this adds backend primitives without changing live HTTP routes,
portal requests or browser-visible behavior. T02 and later HTTP/portal wiring
must consume this gate and retain the existing deterministic portal gates; this
packet does not claim a deployed search service or provider adapter execution.

Recovered research policy is captured in `authority.json` using actual public
MCP denials and `_research_dispatch_error(check_evidence=True)`. New and recovered
native research queries preserve the source distinction under the reviewed W03
amendment: internally built templates and retained `requires_intent_grant=false`
records need research authority without a specialist grant; caller initial plans
always set the flag true and reject caller-supplied true/false flags. The previous
conservative false-flag denial is superseded by source parity, not by a second
policy gate. Research, sensitive-engine and current eligibility checks still apply
at admission, recovery and dispatch. Missing legacy flags continue to default true.
Captured evidence scope and intent are rechecked under the current policy. F09
validates planning metadata but does not establish tenant ownership or prove
that references were admitted: the authoritative research store/worker remains
responsible for those checks and budget/lease/cancellation commit authority.

`inactive.json` preserves the observed Python hazard: an enabled catalog member
missing from active adapters passes MCP validation and silently drops from an
explicit mixed scope. Native MCP validation now rejects that scope atomically,
including recovered research evidence and fresh revalidation. Alternatives include
only active enabled members. This is intentional hardening, not source parity.
HTTP retains dropping behavior; active unported factories remain previewable and
fail only dispatch authorization through the existing availability error.

Recovered research intent is validated against the known non-media profile set
before specialist grant selection. Unknown, whitespace/case variants, `auto`,
and media intents fail closed even with explicit engines. This closes a native
validation gap and strengthens recovery to the public planner's intent boundary;
legacy recovered records with invalid intents require correction or replacement,
not normalization that could alter authority. Valid specialist records still
require their current grant at admission and revalidation.

`ScopeProposal.surface` is trusted transport provenance selected by composition,
not untrusted request data. HTTP intentionally permits an explicit sensitive
scope under its separate source policy. Endpoint code must never deserialize or
copy a surface from wire or stored query fields. Recovered queries use
`from_research_query`; its captured research context cannot be retagged as HTTP.
F09 does not isolate malicious Rust callers inside the process. T02/T06/W02 must
bind each public endpoint and worker path to its intended origin when composing
this primitive. The independent review's public-field concern is therefore a
composition obligation, not a claim of a direct wire exploit.
