# Paired answer and grading preparation

Status: implementation preparation, not registration or scientific admission.
Related to [#516](https://github.com/magnus919/SlopSearX/issues/516).

The study needs to compare the answers people can actually use, alongside
ranking measurements. This preparation builds that comparison without changing
the deployed product or running searches or model evaluations.

The answerer uses the fixed `free` alias and a serialized schedule of two
readiness probes plus sixteen paired answers. Both arms receive the same
question, captured contexts, opening budget and citation catalog. Requests and
responses are archived and verified before independent answer grading. A valid
JSON answer is not evidence that the answer is correct or useful.

Independent graders receive complete shuffled card inventories, source chunks
of at most twenty, and anonymous paired answers. Ranking judgments use the two
caller facets; source and answer judgments use their four critical checks.
These are separate contracts. Each grading phase must close every assigned
submission exactly once before identities are restored. Missing or unknown
judgments remain explicit and cannot be averaged away.

Grading closure verifies externally retained manifest and private-assignment
pins, derives schemas from the frozen reference and assignment, and binds claims
to their original citation IDs. A grader may assess a claim, but cannot rewrite
its source attribution. The coordinator also joins answer packets to actual
request, response-archive and restored-answer receipts rather than accepting
self-consistent replacement answers.

Acquisition and selector preparation are separate phases. The initial plan pins
the protocol, cohort, source closure and acquisition schedule. It cannot predict
the digest of fresh search responses. Actual pool snapshots are sealed once,
reverified from disk and compiled into the input manifest; selector preflight
then verifies that manifest before subsequent execution. Preflight creates no
admission or authority to invoke a provider.

The eight-task numerical gates apply to the mixed workload, separately for each
independent reference and assessor. Result-size and intent breakdowns are
diagnostic. A pass does not establish an improvement independently within every
size band or intent. Numeric thresholds and resource ceilings remain unchanged.

Remaining prerequisites are exact scientific source/dependency qualification,
registration and new admissions, the independently enforced capture boundary,
and fresh development and untouched confirmation results. See the
[capture boundary review](capture-fetch-boundary-review.md). A structural or
synthetic test pass grants no quality credit or production readiness.
