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

Protocol ceilings appear in configuration provenance. They are never substituted
for measurements. Missing control identity, selector timing and usage, complete
stage timing, concurrency, retry, pacing and internal capture-fanout evidence
remain unknown. These gaps must be resolved by qualified execution receipts
before the release resource gate can pass.

This collector is an offline preparation component. Integration into the stage
coordinator, full capture-stack qualification, registration, development and
untouched confirmation remain required under issue #516. Passing mock tests or
merging this component does not confer scientific or production credit.
