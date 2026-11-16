# Separate research-size qualification companion

This companion adds a stricter delivery condition to the frozen eight-task
mixed-workload gates. It does not change the protocol, cohorts, budgets,
thresholds, sampling plan, or stage schedule. Each registered run must pin the
same shared stage contract, source-closure v2, and initial registration before
admission. The shared contract declares both development and confirmation
membership so the qualified source-closure digest remains identical across the
required development-to-confirmation lineage. The operator still validates
that the actual run is one of those two stage kinds and fails closed if any
binding is absent or does not match.

The prior `development-contract.json` and `confirmation-contract.json` remain
bound to the archived v3 material. The fresh frozen-input method has its own
shared contract at
`../frozen-input-method-materials/band-companion-contract.json`, pinned by
that directory's material lock. It binds the unchanged per-band gates to
`D26-R01`–`D26-R08` and `C26-R01`–`C26-R08` under the exact fresh protocol and
cohort hashes. The gate accepts only those two exact material pairs; it does
not generalize acceptance to other protocol or cohort bytes.

For each exact four-task band, the existing A/B ranking calculation and R1/R2
task-use calculation run again over the same closed grades. The same thresholds
apply independently. The original mixed-workload result remains reported as-is;
confirmation eligibility additionally requires the mixed result and both band
results to pass, together with all original shared navigation, stability,
resource, and terminal gates. Missing evidence is inconclusive. No task can be
dropped, moved, or reassigned to rescue a band.

These are four fixed cases per band, not random samples. Their results support
claims only about these frozen cases and conditions. They do not establish a
population estimate, a causal effect of research depth, or an intent-specific
effect (each band has only two cases of each intent). The companion reuses
existing observations and adds no acquisition, capture, selector, answerer, or
grader calls.
