# EXP-043 result: operational pass, selection not supported

All 65 registered Jev requests returned valid responses. The split complete-pool requests avoided the preceding stress-request rejection, but the candidate failed its unchanged quality gates. This result does not support production adoption or fresh confirmation.

| Check | Result |
| --- | --- |
| Primary E minus W0 nDCG@10, reference A | Mean +0.215735; bootstrap lower95 +0.131130; passed |
| Primary E minus W0 nDCG@10, reference B | Mean +0.067716; bootstrap lower95 +0.004150; passed |
| E minus same-response D ordinal control | Zero on all eight primary cases; noninferiority passed |
| Additional facet coverage over D | Zero under both references; strict improvement failed |
| Preserve useful comparator topics | Failed: research lost B's `evidence_grounding` topic versus W0 |
| Exact navigation | All five frozen targets ranked first under W0 and D; F bypassed |
| Repeated/rotated top-ten overlap | Cardiac and research 0.9; evaluation and constructed stress 1.0; passed |
| Operations and exact membership | 65 valid responses; all 13 full pools and applicable arms preserved; passed |

Macro coverage under A was W0 0.954545, D 1.0, E 1.0. Under B it was W0 0.977273, D 0.977273, E 0.977273. The denominators comprise 11 evaluable guard pools for each reference; q9 remains separate, and the constructed 79-representative stress pool is excluded from relevance. Neither reference is independent human gold.

## What prevented selection

The frozen selector made **zero insertions in every main pool**, so E retained D's top ten. Its engineering rule required both source Score >=5 and facet Noul >=0.80. The descriptive selector trace shows the missing research grounding candidates had source Scores 6.02 (`c6`) and 5.98 (`c10`), with grounding probabilities 0.72 and 0.67. The strongest selected grounding probability was 0.61. These numbers are probability outputs, not calibrated proof of correctness. They suggest the absolute cutoff suppressed potentially useful comparative signal; they do not establish that changing that cutoff would pass the gates.

Research's B-labeled useful grounding representatives `c6` and `c10` were present in W0's top ten but absent from D/E. E still had ten B-labeled useful sources and higher research nDCG than W0, illustrating why average usefulness cannot substitute for preserving requested topics. The primary usefulness gain is real under these saved reference rows, while the topic-preservation failure remains decisive.

No threshold, label, candidate, prompt or analysis rule was changed after measurement. No retrospective variant is presented as a registered success. A prospective probability-aware selection policy is the next design question; it must be separately registered, tested against the same quality gates, and then confirmed on untouched queries if supported.

## Cost and limits

Reported usage totaled 1,236,249 input tokens and 108,840 output tokens. Maximum reported per-call input was 46,145 tokens; state-token usage remains unavailable. The maximum remote HTTP time was 626.494 ms, and the maximum D/F remote-time sum was 1,014.925 ms. Both registered limits passed. These times exclude proxy and platform overhead and do not establish end-to-end production latency. Acceptance of these frozen bodies does not guarantee token fit for every allowed 80-card request.

All raw responses, structured receipts and single-attempt history are retained with hashes. Qualification artifacts remain byte-identical to the published premeasurement packet. There were no retries, new searches, Brave requests, page fetches, deployments or Hermes changes. Brave budget remains **0/10**. Issue #516 and the production integration goal remain open.
