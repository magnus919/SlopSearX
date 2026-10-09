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
for measurements. Missing control identity, selector timing and usage, complete
stage timing, concurrency, retry, pacing and internal capture-fanout evidence
remain unknown. These gaps must be resolved by qualified execution receipts
before the release resource gate can pass.

The stage coordinator invokes this collector only after the acquisition,
capture, answer and independent grade artifacts have passed their pinned
bindings. It writes and fsyncs the resource receipt before gate calculation;
the calculator consumes those observations directly. A missing measurement
stays unknown and therefore cannot become a passing resource gate.

The coordinator and collector remain preparation tooling. Full capture-stack
qualification, registration, development and untouched confirmation remain
required under issue #516. Passing mock tests or merging these components does
not confer scientific or production credit.
