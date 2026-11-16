# W05 publication evidence and integration handoff

The reviewed W06 union and integration metadata correction are qualified. The
shared metrics bridge subsequently passed formatting, strict workspace Clippy,
its actual private-Valkey recorder and the W05/F01/O06 gates; all three causal
mutations failed their semantic assertions and restored exact source bytes.
See [integration-qualified-v1.json](evidence/integration-qualified-v1.json) and
[shared-metrics-v1.json](evidence/shared-metrics-v1.json) for immutable bindings.

## Candidate and evidence identities

W05 checkpoint d82c01916d07820ce73d9a8071e2f2fbd48f58b7 contains the focused
HTTP wire correction and 99 native cases. Normal merge
93896d2c6286c304c2e480aae3043c370e4184a5 integrates accepted W06 main
0a0743904264b0054e14cb19cdd78af7d31eb60c without rewriting either history.
The reviewed two-file metadata-only correction
d83a2dffa17e4739598aec771ff561296c9491ed updates W06 integration source
bindings and the exact coverage digest; its cases, commands and runtime are
unchanged.

On the composed runtime, locked Rust1.99.0 metadata/build and strict workspace
all-target Clippy passed. Actual Cargo resolution includes normal MCP→infra and
dev infra→MCP, and the actual composed tests compile and run that graph. W05
completed all 99 required cases exactly once. W04/W11/W02/A09/F01/O06 and
T01/T04/T05 passed on the same runtime. After the metadata correction W06
completed 303 exact case/test identities (248 source-derived plus55 native);
all71 executed W06 commands, F01 and O06 returned zero. The four historical W06
reference controls remain explicitly source-only, not executed case claims.
Publication-head CI must still bind the eventual published source.

The evidence manifest binds actual gate reports and preserved failures by
SHA256. Runtime-source and documentation-only heads remain distinct; successful
old reports are never relabelled as executions of later bytes.

## Corrections and preserved failures

The original numeric HTTP characterization preserves exact bigint text but
shows the SDK structuredContent rounding adjacent integers above u64. The shared
trusted-handler restoration now preserves raw object text and structured JSON
for this actual W05 HTTP mount. The actual maximum batches preserve every public
field, adjacent 18446744073709551617/618, the literal reserved number-tag object,
98 nested arrays and all100 independently retained exact64KiB events in each
of three variants. HTTP byte sizes are13,116,620 /19,625,420 /16,371,020.

The service uses a direct typed array: reparsing the entire valid batch through
the default4MiB parser caused the retained panic/pending request and owned-child
cleanup. The shared32MiB profile applies only to trusted output; inputs stay4MiB,
32pending and the saved-handler64KiB argument bound remains. Other HTTP output
stays4MiB, legacy output16MiB. No larger global parser limit is used.

All three wire negative controls caused their named failures: early large-array
reparse panic plus actual request timeout; output4MiB actual HTTP502; old lossy
structured serialization actual adjacent-integer mismatch. Each control restored
exact frozen bytes. The earlier seven semantic negative controls, original
legacy acknowledgement collision, original failed oracle import, A09 stale pin,
O06 formatting failure and W06 integration stale source pin remain immutable.
These failures are distinct from their subsequent corrected passing runs.

## Shared metrics bridge and O01 mapping

SharedEventObserver increments only fifteen literal static names
through ssx_application::Metrics and forwards the original typed observation to
its downstream. The actual-store recorder observes committed definition,
report, baseline and event at the publication callback, and the authoritative
stored cursor at successful acknowledgement callbacks. Duplicate publication,
backend error and capacity rejection do not report publication success.
The qualified manifest preserves all99 cases and adds six actual assertion-completion
identities. The final W05 execution records all105 exactly once with no ignored
required tests; its three causal mutations fail and restore exact source bytes.

The following mapping is the explicit O01 handoff, not a claim of an installed
Prometheus exporter or rendered label parity:

| Static shared counter | Original metric and closed semantic dimension |
| --- | --- |
| saved_event_published_report_created | saved_event_publications, event_type=report_created |
| saved_event_published_run_incomparable | saved_event_publications, event_type=run_incomparable |
| saved_event_published_run_failed | saved_event_publications, event_type=run_failed |
| saved_event_published_definition_paused | saved_event_publications, event_type=definition_paused |
| saved_event_published_definition_expired | saved_event_publications, event_type=definition_expired |
| saved_event_read_delivered | saved_event_reads, outcome=delivered |
| saved_event_read_empty | saved_event_reads, outcome=empty |
| saved_event_read_gap | saved_event_reads, outcome=gap; also projects to saved_event_retention_gaps |
| saved_event_read_redacted | saved_event_reads, outcome=redacted |
| saved_event_read_rejected | saved_event_reads, outcome=rejected |
| saved_event_ack_advanced | saved_event_acknowledgements, outcome=advanced |
| saved_event_ack_idempotent | saved_event_acknowledgements, outcome=idempotent |
| saved_event_ack_rejected | saved_event_acknowledgements, outcome=rejected |
| saved_event_capacity_stream | saved_event_capacity_rejections, resource=stream |
| saved_event_capacity_consumer | saved_event_capacity_rejections, resource=consumer |

The original names above are Python metric variables in slopsearx/metrics.py;
exported names carry the slopsearx_saved_search_event_ prefix. A gap produces one
primary read outcome and one corresponding source retention-gap projection;
capacity is separate. Counter names never include tenant, subject, consumer,
saved/run/event IDs, query or callback data. O01 must retain these semantics when
adapting the static names, including the additional gap projection.

The shared Metrics API only supports increment and cannot represent the original
backlog-age gauge. The downstream receives unchanged Option<BacklogAge>, whose
private-field constructor enforces finite, nonnegative seconds. O01 must bind
this typed value to the concrete gauge, set zero for absent age on empty/gap,
and preserve existing behavior for other absent-age outcomes. NoopEventObserver
explicitly means absent downstream composition. A shared counter recorder does
not qualify exported rendering, scraping or deployment; Sentry and other egress
remain outside this task.

## W09, G04 and release obligations

The actual HTTP mount admits only the three captured saved-tool descriptors,
uses explicit NetworkAuth and managed cleanup, and rejects missing/wrong bearer
credentials before definition mutation or engine dispatch. The fixture uses a
trusted fixed principal. Complete production identity resolution, live grants,
all native registry workflows, W05 stdio/legacy/gateway mounts and portal/browser
composition remain W09/W10 and their auth/release gates.

G04 must drain legacy acknowledgement writers before v2 migration. Ambiguous
v1 authority is quarantined; explicit replay and acknowledgement establish only
the current tenant/consumer v2 authority without attributing or mutating legacy
state. Native v2 wins thereafter. Mixed old/new writers are not a qualified
rollout. No deployment, production/public acquisition or exactly-once remote
execution claim follows from these private tests. G01/G02/G04/G08 remain full
release obligations; optional original capture helpers retain their G08
retirement inventory and mandatory Rust gates never invoke them.
