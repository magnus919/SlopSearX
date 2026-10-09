# Coverage-first execution preparation

Status: **offline-tested implementation, not registered or admitted**.
Related to [#516](https://github.com/magnus919/SlopSearX/issues/516).

The new seams implement the proposed budgets without invoking them during
preparation. They are study tooling; the supported product feature has not
been implemented or qualified.

The combined local suite passed 176 tests and nine subtests on Python 3.12
(`tests/test_coverage*.py` and `tests/test_intent_ranking*.py`).
Bounded independent coding reviews were completed and their findings addressed.
[execution-preparation-validation.json](execution-preparation-validation.json)
records the limited scope of that evidence. Full repository CI remains a merge
requirement; these checks are not scientific qualification or an admission.
Repository-wide pre-commit checks, including import contracts and mypy, passed
using a new isolated environment populated from the current declared development
dependencies. The existing repository `uv.lock` omitted those current dependencies;
it is not accepted as the future scientific runtime lock. Exact scientific
dependency closure remains a registration prerequisite.

| Module | Implemented behavior |
| --- | --- |
| `coverage_live_acquire.py` | Explicitly admitted keyless public-engine transport, no retries, per-request and stage limits, 7-second serial operation pacing, 3-second arXiv physical-request pacing, private durable complete canonical-pool snapshots |
| `coverage_pipeline_inputs.py` | Reverify snapshots from disk; retain full metadata/native order; compile the existing bounded ranking projection and stable URL/source bindings |
| `coverage_source_capture.py` | Offline MockTransport seam only; binds protocol/cohort/endpoint/runtime identity and preserves bounded synthetic responses and terminal inventory. Live HTTP capture is blocked pending independent qualification of the delegated fetch redirect and DNS-rebinding boundary. |
| `coverage_jev_execution.py` | Bind one-shot request bytes before dispatch; original V1 generator and replay for the control; unchanged coverage prototype for the candidate; private response archival before parsing; token ledger and one-second phase deadline |
| `coverage_consumer_inputs.py` | Join duplicate URLs to the same newly captured context; identical first-ten/five-success/8,000-character budgets; anonymous paired inputs and exact passage-citation identity restoration |
| `coverage_guarded_driver.py` | No-call status and guard contract tests; full scientific orchestration remains outstanding |

Runtime identity was checked through read-only container metadata on 2026-10-09.
The experimental agent reports revision
`b5c1bf473bec78f3ebbfb630213b08612b3264fb` and model alias `free`.
The public application source at that revision confirms the `/health` runtime
projection. [candidate-runtime-identity.json](candidate-runtime-identity.json)
contains only this projection and is an expected-input artifact, not a health
receipt or admission. The capture runner must observe matching healthy runtime
metadata during the admitted stage before scraping.

The experimental search service still matches all four previously observed
control module digests in
[control-source-parity.json](control-source-parity.json). Its original V1 parser
and generator remain the control; preparation source on current main is not
substituted for that deployed implementation.

Source capture currently accepts only an exact `httpx.MockTransport`. It
rejects real HTTP transports before the one-shot lease or any request because
the scraper performs the delegated fetch and this runner has not qualified its
redirect, DNS resolution, or rebinding protections. Synthetic transport tests
verify receipt and terminal-inventory behavior only; they do not establish
live-fetch SSRF safety. Live capture remains a registration prerequisite.

Every operation's canonical pool is persisted before the next acquisition
operation starts. Downstream preparation consumes externally hash-pinned
snapshots rather than relying on an in-memory object that would be lost after
a process failure. The once-only stage cannot be reacquired after such a
failure. Contexts for URL duplicates are reused only within the current sealed
stage. A complete capture inventory can include unsupported URLs and ordinary
source failures; it does not mean every page was captured.

Acquisition retains the 2 MB limit **per response**. Its derived aggregate
transfer bound is 53 requests times the response bound plus the existing 64 MB
material bound per request; it does not impose an additional 2 MB stage ceiling.
Pool snapshots use the existing 64 MB material bound. Actual engine requests
remain constrained by the frozen query and adapter plans. Every arXiv HTTP
dispatch, including the adapter's same-host redirect follow-up, is separated
by at least three seconds; this wait is charged against that request's existing
10-second deadline. The seven-second inter-operation pacing remains in force.
This follows the [arXiv API manual's request pacing guidance](https://info.arxiv.org/help/api/user-manual.html).
Redirects remain capped at two physical requests per arXiv operation, and any
failure is terminal with no retry.

Jev raw response archival occurs before parsing. A per-call result receipt is
explicitly provisional: phase acceptance is determined only after its fsync
finishes and the measured deadline is checked. A late fsync records terminal
failure and a durable correction; it does not confer successful timing credit.
Unknown usage remains terminal and is not invented as zero.

Before live execution, finish the answer and scientific-assessor orchestration,
source-reference closure, prompt/assignment and dependency locks, decision-scope
freeze, exact registrations, independent source qualification, and new admission
receipts. Confirmation remains untouched. An injected permit-verifier interface,
passing synthetic tests, or this document grants no execution authority or
scientific quality credit. The proposed protocol's numerical acceptance and
resource ceilings remain unchanged.

No Brave search, source capture, Jev evaluation, answerer or scientific-assessor
call was made in this preparation. Read-only source/configuration checks changed
no deployment, credentials or Hermes configuration. Private endpoints,
hostnames, addresses, keys, personal paths and raw operational artifacts are
excluded from the publication.
