# W00 saved definitions and schedule ownership

W00 implements saved-definition records, pure validation and policy fingerprints,
tenant-bound persistence, optimistic edits, pause/resume/delete controls, quotas,
recent/due indexes, bounded reconciliation and schedule leases. It makes no
provider calls and does not execute saved searches. W04 owns execution and guarded
result/schedule publication and comparison/report construction; W05 owns
event/outbox behavior. W09/W10 own public tool/portal composition.

`SavedPolicy` consumes the accepted F09 pure `PolicyService`, a separate saved
search grant and a complete, secret-free `auth_configured` map for selected
engines. Public engines can be eligible while `auth_configured=false`; readiness
is not configuration evidence. Definition validation uses F09 preview, which is
not dispatch authority and can describe unported factories. W04 must freshly
validate live policy, credentials, executable availability and current fingerprint
before provider dispatch. Pause/resume remains possible under revoked engine
eligibility when the saved grant remains enabled. Resume only changes state.

`SavedStore` owns the accepted F07 shared Valkey connection. It exposes no generic
mutation callback. `PreparedDefinition` construction and claim ownership fields
are private. Claim records have read-only access; tokens/raw CAS snapshots never
appear in Debug. Native opaque IDs and fresh claims use 18 random AWS-LC bytes
(144 bits), base64url without padding, matching source handle construction.
Production callers use `claim_fresh`; deterministic token injection is a testing
seam and requires fresh, unpredictable tokens in other compositions.

The native `cargo xtask check W00` gate replays reviewed immutable observations
and runs mandatory real disposable Valkey tests. Python is only a temporary
inventoried developer oracle, retired by G08; it is not a runtime or native-gate
dependency. The oracle requires CPython 3.14.7, the pinned disposable Valkey binary,
no configured providers, and writes only this fixture. No scientific experiment,
production deployment or performance claim is made.

The portal-visible definition shape, recent ordering and source observation
window are preserved. There are no portal routes/assets in this slice. Existing
legacy portal gates stay enabled; future W10 must compose current policy and
read projections, rather than treating stored definitions as browser authority.
See [parity and hardening](PARITY.md) and [test mapping](coverage.json).

Developer-only recapture (outside native acceptance):

```sh
PYTHONPATH=. SLOPSEARX_TEST_VALKEY_SERVER=/path/to/pinned/valkey-server /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/w00/capture_source.py
```
