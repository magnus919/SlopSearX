# O03: active experiment tooling and native evidence contracts

O03 freezes the current command and evidence boundary for the tooling ports. It
does not run an experiment, acquire a source, call a provider, issue a runtime
receipt, approve an outcome, or rewrite historical evidence. The result is an
inventory and a prospective contract for O07 (acquisition/replay) and O08
(authority/receipts/accounting/qualification/verification).

## Inventory and handoff graph

[`inventory/o03-active-tooling.json`](inventory/o03-active-tooling.json) has one
row for each current `scripts/*.py` entry. The row pins the source Git blob and
records whether the file has a real `__main__` CLI or is only a programmatic
module, its observed CLI flags and entrypoint functions, statically visible
internal import edges and reverse consumers, source summary, input/output
classes, owner, future native command boundary, disposition, and the handoff
validation that must pass before the Python entrypoint can retire. O02-owned CI, health, upstream,
and image-promotion tools are included as cross-lane ownership rows; O03 does
not implement or gate those tools. O07 and O08 rows are contracts, not claims
that a native replacement already exists.

The producer/consumer flow is:

```text
registered experiment + immutable fixture/source inputs
        │
        ├── O07: admission → authority-bound dispatch → acquisition/replay
        │                       │
        │                       └── raw observations + execution accounting
        │
        └── O08: source/runtime receipt → receipt verification
                                │
                                ├── O08: stage/material/authority checks
                                ├── O08: qualification and outcome classification
                                └── O08: final verification / release evidence
```

`scripts/coverage_live_acquire.py`, `coverage_study_acquire.py`,
`coverage_engine_replay.py`, `coverage_pipeline_inputs.py`,
`coverage_frozen_pool_collect.py`, `coverage_selector_input_map.py`,
`coverage_jev_execution.py`, and the operator dispatch/runner scripts belong to
O07 even where they currently import local backend objects transitively.
Receipt issuers, runtime/environment binders, authorities, accounting,
qualification, stage finalizers, graders, and verifiers belong to O08. Source
import text is not the ownership test: process spawning, a Python helper that
loads application modules, and evidence that relies on Python runtime hashes
all remain backend-coupled. Only a separately packaged client using a documented
native wire interface and importing no backend implementation can later be
classified as client-only. No current O03 row makes that exception.

The machine inventory exact-set checks `scripts/*.py` against the F01 Python
disposition and each current Git blob. Its separate `supporting_paths` pins the
two non-CLI regression modules (`tests/test_coverage_saved_method_materials.py`
and `tests/test_intent_ranking_replay.py`) and the one dynamically loaded,
immutable `.py.txt` legacy replay source. The executable source itself remains
outside the active script exact set; it is only read/compiled by its historical
offline replay consumer. The inventory records the corresponding tests as
`pytest -q tests/test_coverage_saved_method_materials.py` and
`pytest -q tests/test_intent_ranking_replay.py`. These commands are inventory
metadata; O03 does not run Python. F01's full interpreter/configuration census,
the inert documented evidence examples, and the mandatory F01/F02/F03/O06
gates are explicitly accounted for in `execution_surfaces`.

New scripts, missing paths, changed source, absent owners, unsupported schema
versions, missing regression anchors, and incomplete schema bindings make
`cargo xtask check O03` fail. The check is offline and does not execute a
listed Python script.

## Native command boundary

O03 proposes the stable CLI namespaces `ssx experiment <operation>` for O07 and
`ssx evidence <operation>` for O08. Each inventory row gives its proposed
operation and common `--plan`/`--input`/`--output` or evidence input/output
arguments. These commands do not exist yet; O07 and O08 must accept the names,
refine their flags, and implement the corresponding operation registry. A
proposal is not evidence of a completed port. Before retiring a legacy
entrypoint, its owner must replace the proposal with the accepted native
command or wire contract and show an offline deterministic fixture that
preserves inputs, dispatch order/count, budgets, outputs, and denied/error
behavior. Optional analysis clients may use the native wire protocol, but
cannot be required by native release checks.

O07 owns registration/admission, frozen source and selector input formation,
bounded dispatch, source acquisition, cancellation/deadlines, engine replay,
and execution transcript production. O08 owns runtime/source receipt issuance,
receipt verification, authority and reservation validation, request/cost
accounting, grading and qualification, and final evidence closure. O07 emits
observations and accounting facts; O08 verifies their conservation and binds
them to the exact native artifact and configuration. O08 must not infer
successful execution from a missing receipt or from a count of present files.

## Prospective evidence contracts

Three additive JSON Schema 2020-12 contracts live in `contracts/o03/`:

* `source-receipt.schema.json` binds a clean Git commit and exact source tree to
  a binary digest/size/path, Rust/Cargo/target identity, lockfile digest, and
  redacted effective configuration digest.
* `runtime-receipt.schema.json` adds the plan/run identity, issuer and
  reservation authority, explicit clock source and deadline, request/cost
  reservations and realized accounting, and a terminal outcome.
* `qualification-result.schema.json` binds every named check and its evidence
  digest to the runtime receipt and source, and records the decision timestamp,
  reason codes, and explicit outcome.

These schemas define prospective version `1.0.0`; they do not reinterpret or
rewrite historical Python evidence. Historical records remain byte-identical
and retain their original schema semantics. Readers may add version-aware
support, but migration evidence must preserve original bytes and record any
derived native receipt separately.

The verifier has to enforce these semantic invariants in addition to JSON
Schema validation:

* **Artifact identity:** hash the exact executable bytes, length, source commit,
  clean status, complete source file/blob set, lockfile, target, compiler and
  effective configuration. Redact secret values while binding the redacted
  configuration and its declared key set. Reject stale or partial source maps.
* **Authority:** an issuer signature/identity and bounded authorization must
  cover the exact plan, run, permitted operations, source set, provider class,
  maximum requests/cost, and deadline. Missing, expired, mismatched, or denied
  authority prevents dispatch and produces a denied/blocked record.
* **Clock:** wall-clock timestamps are nonnegative and ordered; a dispatched
  run must start no later than its bound reservation deadline. A denied, blocked
  receipt with zero dispatched requests may record time spent detecting expiry
  and writing terminal evidence after the deadline. This does not waive ordered
  wall timestamps, bound deadline identity, reservation caps or conservation;
  all actual dispatches retain the start and monotonic deadline-budget checks.
  A qualification
  decision timestamp cannot predate runtime completion. Elapsed time
  comes from a monotonic clock. A fixed clock is explicit for deterministic
  fixtures. A backward wall-clock jump, missing deadline, or expired reservation
  cannot be repaired by substituting a fresh timestamp.
* **Reservation and conservation:** dispatches cannot exceed reserved requests;
  reserved requests equal dispatched plus released; dispatched equal settled
  plus unresolved; and reserved cost cannot exceed its authorization cap.
  Known actual cost cannot exceed reserved cost. A known zero is represented as
  `{"status":"known","microusd":0}`. Unknown actual cost is instead
  `{"status":"unknown","reason_code":"..."}`; it is never encoded as
  zero and cannot yield an accepted runtime outcome. Checked addition rejects
  integer overflow. The fixtures exercise reservation caps, both request
  conservation equations, cost caps, deadline binding, denied authority, and
  accepted-with-unknown-cost rejection.
* **Failure versus unknown:** known application/provider errors are `failure`
  with `failure.reason_code` and a content-bound `failure.terminal_evidence`
  artifact (SHA-256, byte size, and path). Missing either field invalidates the
  receipt, and failure details are invalid on other outcomes. If a dispatch may
  have occurred but the acknowledgment/receipt is absent or transport outcome
  is ambiguous, record `outcome_unknown`; do not call it a failure, retry, or
  success without a new bounded authorization and idempotency/reconciliation
  evidence. `accepted`, `rejected`, `inconclusive`, and `blocked` retain
  distinct decision meanings.
* **Qualification:** each declared check is present once. `not_run`,
  `unknown`, or a failed check cannot yield `accepted`. All rejection and
  inconclusive reasons remain in the append-only record. Missing data is not a
  passing check.

The native O03 check validates schema structure (including a schema for every
required property and explicit object unknown-field policies), then applies the
committed schema keywords to every prospective fixture: closed objects,
required fields, types, constants, enums, supported patterns, numeric/string/
array bounds, unique items, references, `oneOf`, `allOf`, and conditional
`if`/`then`. Unsupported schema keywords fail closed. It then decodes the
examples into closed typed records and checks their binding and conservation
semantics. Five valid examples include accepted-known-cost,
outcome-unknown-with-unknown-cost, and a known failure with terminal evidence;
eighteen negative examples cover schema and cross-field failures, including
missing failure reason/evidence, a failure with unresolved work, a dispatch
after deadline, duplicate authority scopes, invalid plan IDs, duplicate reason
codes, and a qualification bound to the wrong runtime-receipt digest. Accepted
qualification is checked against exact runtime receipt bytes, plan ID,
source-tree digest, and runtime completion time. This executes only
deterministic receipt fixture validation. It does not issue receipts, prove a
signature, dispatch, or implement O08's production verifier; those remain O08
work.

Receipt ingress rejects duplicate decoded JSON object keys at every depth,
including equivalent escaped key spellings, before schema or typed validation.
Numbers retain the exact unsigned-integer normalization contract; runtime receipt
digests still bind the original bytes, never the normalized representation.
Source-file references use a portable repository-relative path contract: nonblank, nonempty
forward-slash-separated components, with no empty, dot or parent components,
backslashes, colons, NUL, or components ending in a dot or ASCII space. These
rules apply independently of the validating host's filesystem conventions and
preserve canonical paths in the active source inventory. They describe source
receipt references, not a general library for arbitrary Unix filenames.

## Verification and retirement sequence

Run `cargo xtask check O03` for deterministic contract and inventory validation.
The CLI reads only the checked-in inventory/schemas/fixtures and Git blob IDs;
it emits a native JSON status record and never starts a Python process, provider,
or experiment. To run the two pinned legacy regression tests after native
replacement work, use the exact pytest commands recorded in `supporting_paths`;
they are not prerequisites added to O03's CI. Its report
has `provider_calls: 0`; success certifies only the inventory and prospective
contract shape. It is not an experiment result, O07/O08 implementation
acceptance, or permission to retire any Python gate. The legacy checks remain
enabled until O07 and O08 replacements have passed their exact source-bound
fixtures and O09 accepts the cutover. `tests/test_coverage_saved_method_materials.py`
and `tests/test_intent_ranking_replay.py` remain explicit preserved regression
anchors for O07/O08; their contracts must be represented in native fixtures
before those Python tests are retired.
