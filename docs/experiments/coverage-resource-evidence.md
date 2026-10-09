# Coverage study resource observations

`scripts/coverage_resource_evidence.py` reconstructs resource observations from
independently pinned acquisition, capture, answer and assessor artifacts. It does
not execute a provider, authorize a study, or establish a quality result.

The collector checks artifact hashes and stage/source bindings before counting
physical acquisition requests, per-engine requests, capture calls and contexts,
answer request settings and outputs, and assessor submissions. Response bytes
received by capture are recorded separately from bytes retained in its archive:
truncation or suppression can make these counts differ. Older receipts without
an observed-byte counter leave the corresponding measurement unknown.

The coordinator seals the acquisition receipt inventory at acquisition
completion and includes that seal in the durable phase record. Collection uses
this earlier pin; replacing receipts after acquisition stops the run. Deadline
checks stop collection between bounded reads. An OS read already in progress
cannot be interrupted here, and the final post-fsync guard still invalidates an
overrun.

The 2 MB grader-response limit applies to each submitted response. A closed
grade view combines those responses, restored identities and repeated source
views, so its separate aggregate bound is derived from the existing 112-packet
and per-packet ceilings. This does not enlarge the provider-response limit.

Protocol ceilings appear in configuration provenance. They are never substituted
for measurements. Missing control identity, selector timing and usage,
concurrency, retry, pacing and internal capture-fanout evidence remain unknown.
The resource receipt collected during the stage leaves complete stage timing
unknown because collection precedes final inventory fsync. After that fsync,
the coordinator writes `stage-closeout.json`, which binds elapsed time through
the final inventory file and directory fsync to the final inventory digest,
source revision, protocol, cohort and stage. Its own fsync is explicitly
outside the measured interval. `verify_stage_closeout` reopens and checks both
durable artifacts against the externally retained `StageResult` pins; a later
consumer may then use the returned elapsed observation. A missing or mismatched
closeout remains unknown and cannot pass the resource gate.

The stage coordinator invokes this collector only after the acquisition,
capture, answer and independent grade artifacts have passed their pinned
bindings. It writes and fsyncs the resource receipt before gate calculation;
the calculator consumes those observations directly. A missing measurement
stays unknown and therefore cannot become a passing resource gate.

The coordinator and collector remain preparation tooling. The closeout timing
proof is a separate post-inventory handoff and does not rewrite the resource
receipt or gate calculation from that completed stage. An independent
subsequent consumer must verify and join it before treating
`stage_elapsed_seconds` as observed. Full capture-stack qualification,
registration, development and untouched confirmation remain required under
issue #516. Passing mock tests or merging these components does not confer
scientific or production credit.
