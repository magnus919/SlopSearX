# EXP-075 implementation review

Independent GPT-6 Luna review is in progress. This record does not qualify the
harness or the ranking candidate. No live EXP-075 request has been admitted.

Material findings from the first review pass:

- The programmatic live runner accepted caller-supplied qualification/proof
  bytes without requiring the gate and could substitute the offline credential
  marker. Require a bound, one-use admission capability and a real nonempty key.
- Environment-selected lease directories could evade the durable exclusive
  study identity. Use a fixed live identity store; admission tests isolate home
  and forbid network/credential access.
- Aggregate usage checks did not reserve the permitted per-call input/output
  allowance before dispatch. Refuse dispatch when remaining budget cannot
  cover those bounds; retain unknown-usage terminal stopping.
- Source closure omitted the actual service reranking seam. Add service.py to
  the exact qualification closure without claiming the harness ships that path.

Root review additionally corrected baseline parse timing, durable failure
endpoints, inconsistent fallback status, and successful response receipt writes.
The earlier synthetic builder failure is retained separately.

All material findings require fixes, affected offline tests and a bounded
follow-up before qualification. The full production acceptance checklist and
untouched task/pool confirmation remain open.

## Bounded runner follow-up

At `751d162dbe86a617fd8e61939c1f473ee2b3851a`, independent review
confirmed the original admission, fixed lease location, usage-reservation and
service source-closure fixes. Root independently ran all 13 runner tests under
CPython 3.14.7; all passed.

At `f68e2f045648d91817b0a831748c34b9c321655c`, follow-up confirmed frozen
input reconstruction and rejection of injected live builders/clients. The
private admission constructor still self-assigned its seal; an issued-object
registry and constructor-forgery regression are required before qualification.
This is a same-process misuse guard, not a security boundary against hostile
Python code with process/module access.

Analyzer and raw-receipt verifier review/tests remain pending. No provider
acceptance, quality improvement or production readiness is established.

At `d0f1ee293122afc3ae9bc4b34f1bf66baf0497c`, independent review confirmed
that only gate-issued object identities enter the admission registry and that
run consumes the entry. The direct-constructor regression rejects before
transport. Root independently reran all 13 runner tests successfully. No
unresolved material runner finding remains in this bounded follow-up; full
analyzer/verifier qualification is still pending.

## Analyzer and raw-receipt review

The first raw-verifier review found that neutral interruptions could be marked
verified without the required first call or durable failure/fallback sidecar.
The one-call branch skipped durable failure checking and the two-call failure
branch treated the final receipt as optional. Require the first saved call and
the matching durable failure artifacts on every terminal neutral path. Add
deletion/tampering regressions while preserving verification of real bounded
failures and unknown usage. An incomplete analysis label alone is insufficient
to establish packet integrity. Fixes and affected tests are pending.

At `e3adc7b`, bounded follow-up confirmed mandatory neutral W0 and
phase-appropriate failure sidecars match runner output. Root independently ran
all 14 verifier tests at `fc5afbb`; all passed, including deleting the neutral
call and required terminal receipts. A complete explicit local fixture run
also verified after the source fix, without quality credit.

One analyzer consistency correction remains: a baseline failure never enters
the candidate phase and therefore has only its durable baseline-failure
receipt. Analyzer receipt requirements must follow the executed phase instead
of treating absent candidate/final receipts as corruption. This does not
change incomplete/no-quality policy.

Verifier tests additionally exposed a runner diagnostic issue: a measured
build overrun before dispatch carried a physical elapsed time. The committed
fix keeps physical elapsed null, retains build/serialization diagnostics and
reports the timeout without fabricating an HTTP attempt. Root reran all 14
runner tests successfully after that correction.
