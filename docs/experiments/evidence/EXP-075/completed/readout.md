# EXP-075 completed comparison: development rejected

The qualified producer completed all 44 physical calls and 21 research
operations with no retries or new searches. Usage is fully known: 992,269 input
and 35,535 output tokens. Total paired wall time was 18.204 seconds. The
80-card capacity candidate phase took 739.801 ms; research candidate phases
ranged from 267.839 to 634.454 ms. These are bounded study observations, not
a production latency guarantee.

The original qualified analyzer failed before metrics because it did not store
the validated W0 order in its lookup. That failure and every original source
and receipt are retained unchanged. A separately committed, source-bound
post-run utility inserts only that already-validated order. Root ran it without
mocks: original source closure and all raw receipts passed verification under
the explicitly corrected evaluator. See [post-run verification](../postrun/verification-postrun.json)
and [analysis](../postrun/analysis-postrun.json). The original evaluator is not
retroactively described as successful.

| Reference | Mean nDCG change | Fixed bootstrap 95% interval | Mean gate |
| --- | ---: | --- | --- |
| A | +0.031187 | [0.013226, 0.051986] | fails required +0.05 |
| B | +0.035683 | [0.011551, 0.058828] | fails required +0.05 |

No primary-case useful-count/facet/loss safeguard failed. However, q4 rotation
retained only one of the original top-ten candidates (overlap 0.1 versus
required 0.8). Missing acquisition targets q10/q11 remain unmet. The final
result is `development_rejected`; no candidate qualifies for implementation
or untouched confirmation. Positive averages do not erase stability or
acquisition failures. One natural long pool cannot establish general tail
benefit.

The next step is a bounded offline diagnosis of q4 component scores, ties and
presentation dependence using these saved results. Any revised ranking rule
requires its own declared development evaluation and then genuinely untouched
tasks and pools before the full production acceptance work. Do not tune or
relabel this frozen comparison into a pass, and do not rerun its provider calls.

Publication scans cover the original JSON and decoded request/response fields,
with zero credential/private-host/address/personal-path hits. Byte-identical
original-file hashes and separate post-run hashes are retained. Offline
post-run regression tests exercise the complete non-synthetic metric path with
local fixtures and original source-closure checks; test metadata is not study
quality evidence. Experimental fork/mainline/Hermes/deployment defaults remain
unchanged.
