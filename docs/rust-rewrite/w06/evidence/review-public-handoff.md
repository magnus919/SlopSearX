# W06 independent-review source supplement

This capture supplements the independent review receipt
`86f59f6839dda415849d1a2f8526324f185336d3b0cba9311a47b072fd134ccd`.
The original85 corpus/exporter and the sealed151-receipt component packet remain
unchanged. This supplement is source evidence awaiting native binding and final
qualification; it does not by itself establish native parity.

Producer `/private/tmp/w06-review-public-source-capture.py` is frozen as
`review-public-source-producer.source.txt`, SHA256
`9a9e01b55290a741c6795b9eb1cfa09e0cf7b44c79d4156e0ececba71e887fce`.
Output `review-public-source-outcomes.json` SHA256 is
`0d43f086c5c98a9bee7bd5339dc80d1ea7801147d7fa6e4be4c71263a5c86a1e`.
`review-public-capture-receipt.json` retains exact argv, exit status, stdout and
stderr for normal, optimized, repeated optimized and wrong-source-hash optimized
runs. All three successful outputs match byte for byte. The last command fails
before backend imports. All14 original source guards and the original85 corpus
identity are checked before backend imports, with explicit checks unaffected by
`-O`. The producer is a manual W06/G08 migration oracle, never a runtime or
mandatory native-gate dependency. Provider and network calls are zero.

| Source group | Rows | Native binding requirement |
| --- | ---: | --- |
| review.plan.* |26| Feed literal input to native planning; compare full plan/error envelope |
| review.view.* |11| Feed literal include/max bound; compare full normalized view or full error |
| review.submit.* excluding admission_collision |50| Actual submit/replay/admission against private Valkey, literal setup/input and full public output |
| review.get.* |3| Actual retained reader/store-mode errors |
| review.retry.* |5| Actual retry/store-mode and key-before-store precedence |
| review.source.* |13| Actual source parser/resolver with declared saved fixture; full envelope including destination/kind/field |
| review.replay.* |12| Existing exact source record plus literal submission; full public response and record; fresh-plan output separately |
| review.submit.admission_collision |1| Both real lookups miss, first actual admission wins, second actual admission conflicts; full outputs and final winner record |
| review.runner.* |4| Reference controls only; pair explicitly with native tests, never emit source-execution receipts for a different native fixture |

The121 parity rows are tagged `qualification_role=source_case`. The four runner
rows are tagged `reference_control`. Stable exact IDs are enumerated in the
receipt; counts alone are not proof. `saved_fixture` declares real input
artifacts: saved definition engines and the complete run-a/run-b report objects.
Composition traces include the actual original resolver's stored descriptor.
`setup.existing_record` contains the actual seeded original record. Successful
wire responses include `after_record`, copied from the actual backing fixture
without adding a synthetic public read to the trace. The admission race retains
its actual winner record. A native random operation identity may be compared via
an explicit bijection; it must not manufacture other expected record fields.
Clock/UUID/query identity/lease token are explicit deterministic fixture inputs.

The original behavior includes these easily missed branches:

- Both initial/fallback malformed engine lists return the complete scope.engines
  envelope, including the same message for empty, nonlist, blank, nonstring and
  oversize lists.
- Falsey JSON fallback inputs reach Python truthiness before scope validation.
- Required argument type/shape checks, key normalization, store unavailability,
  replay conflicts and source conflicts preserve their original order/messages.
- Artifact shape/type/kind/id/contract failures retain distinct full messages;
  a valid snapshot source includes unsupported_transition destination/source_kind.
- Composite run component `bad:run` is structurally valid and absent from the
  report list: source_not_found, rather than a policy denial.
- Empty include selects the original default engine_status/results view.
- Equal-valued float/bool objective replay and ignored nonmap replay filters can
  return an existing operation while fresh strict planning rejects that input.

The runner controls establish the following source facts and native pairings:

| Control | Actual original outcome | Native pairing and scope |
| --- | --- | --- |
| deadline_after_acknowledged_renewal | Definite original renew ack, clock1000→1006, no dispatch; deadline_exceeded message is "deadline expired before dispatch"; full failed record retained | native.deadline_after_renewal_no_dispatch; exact diagnostic/accounting can be bound, but differing fixture identities are explicit |
| actual_shared_rate_denial | Actual original SearchService raises RateLimitExceededError with empty str after client-rate denial; runner saves execution_failed with empty message and unknown observation, no engine call | native.typed_acquisition_failure_diagnostic; actual closed native service is a different operational trigger, approved static message "shared search acquisition failed" is an intentional difference |
| presave_revocation.sensitive | Original snapshot persistence flips sensitive grant, runner still acknowledges completed record with selected snapshot; subsequent public read denies | native.presave_policy_recheck; approved native final pre-save authority hardening intentionally denies publication |
| presave_revocation.staged_grant | Original snapshot persistence revokes staged grant, runner still acknowledges completed record with selected snapshot; subsequent public read denies | native.presave_policy_recheck; same explicit hardening, no claim that the current native test executes both original control inputs |

The original controls use actual SearchService, StagedSearchRunner and
SnapshotStore. They do not synthesize an execution_failed response or pretend an
exception-return fixture exercised acquisition. The native authority seam is
synchronous revalidation at explicit boundaries, not a continuous revocation
notification or an atomic authority fence around a previously sent SAVE.

Saved-report authority correction has its own13-row packet and producer owned
by the implementation agent: `saved-source-authority-outcomes.json`, SHA256
`7b3f1375ee478af9fbb7aaa8e975390ee5c5427e8beab976b41955a54922f079`.
Its receipt and both temporary producers are listed in the local oracle
inventory. Parent owns global temporary-oracle registration and final F01.

Do not modify either frozen producer or output when binding native tests. Add
new evidence under unique paths if another original behavior must be captured.
The next qualification manifest must explicitly bind actual tests after their
inventory is frozen, preserve the old component receipts, and use fresh output
paths for actual mutation negatives. No native gate/runtime/global inventory was
changed as part of this supplement capture.
