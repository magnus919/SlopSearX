# W11 staged admission and recovery store

Implementation baseline: `1e9dd82d62456fe6f511cc5ade88bae619fac027`.
Issue #911 consumes accepted F07 (#886) and F09 (#888) storage/policy
contracts, F05 canonical identities and the accepted native W00/W01 patterns.
The authoritative handoffs are `../f07/README.md` and `../f09/README.md`
and their accepted implementation at this baseline.

`ssx_domain::staged::Record` reads full public v1 records, retains opaque extras
and original IDs, and computes the source identity from version, query, requested
and effective objectives, scopes, expansion and filters. F05's bounded lossless
ASCII compact canonical JSON encoder supplies the exact digest bytes. Neither
presentation options nor `fallback_when` are silently added to identity.
The codec normalizes only schema-declared empty cjson maps, never arbitrary
objects. Both stage scopes must match the immutable plan, scopes are disjoint,
IDs unique, costs sum to the durable reserved budget, and observed calls stay
bounded. Interrupted/unknown calls remain charged; aggregate unknown never
becomes known merely because a retry completes.

`ssx_infra::valkey::staged::StagedStore` implements admission/idempotency, reads,
recent pages, claim/renew/fenced completion, retry-key replay, bounded ready
selection, orphan interruption and cursor-based ready/running reconciliation.
It reuses all six exact accepted F07 staged Lua bodies and key encodings.
Admission revalidates supplied F09 scopes against current policy, binds resolved
query/filters/MCP origin and selected engines/enforcement, then atomically
applies source idempotency/quota/index behavior. Legacy full-key recent-index
members are accepted only for the exact tenant prefix; `idem:` slots are ignored.

`prepare_claim` captures and validates exact raw bytes before the claim.
`claim_prepared` atomically compares that snapshot before the unchanged F07
claim body, so a complete intervening claim/save/retry cycle cannot reuse an
attempt ID or reserve from stale validation. A changed observation returns no
claim and makes no charge. The convenience `claim` performs these two steps.
`OwnedStage` retains the exact raw script reply and a typed owner token.
`save_owned` permits current-attempt completion/next-stage progression or narrow
unused-fallback skipping while preserving prior attempts, plan, budget charges,
identity and retention. Its wrapper compares raw bytes before the source token
fence. A source Lua error can follow earlier writes; an ambiguous write timeout
can hide a committed completion. Both errors remain explicit F07 outcomes.
Callers reconcile authoritative state before considering another logical action.
There is no automatic retry, local fallback or provider execution.

## W06 handoff

W06 owns constructing caller plans and policy-approved requested scopes, the
staged tool grant and operator deadline/call ceilings, source-derived saved
report admission, fresh authorization/renewal immediately before each provider
request, cancellation, clean-empty fallback decisions, snapshots/results and
public rendering. Its stored records are evidence, never execution authority.
The available F09 `PolicyService` does not model the staged tool grant or these
operator ceilings; W06 must bind them at its endpoint/worker boundary.

1. Resolve/authorize each stage through the shared policy service. Build a full
   source-compatible `Record` with the F05 identity digest. `admit` accepts its
   current `PolicyService` and the corresponding `ValidatedScope` references.
2. Claim with fresh unpredictable owner tokens and globally unique attempt IDs.
   A prepared snapshot can be revalidated before `claim_prepared`; a lost
   comparison requires a fresh observation. Never dispatch an unclaimed record.
3. Recheck policy and renew the owned lease before executing; preserve the
   current owned attempt ID and reservation. Renewals use the fixed60-second
   source lease; logical deadline remains independent.
4. Derive a completed/failed/next-stage record and call `save_owned`. A stale
   lease, changed raw state, malformed history or altered identity cannot commit.
5. Periodically run bounded `recover_orphans` and every cursor page of
   `reconcile_indexes`. Ready/running indices are hints; scripts recheck authority.
   No recovery automatically retries a potentially charged remote request.

The state APIs establish one valid owner and duplicate-commit prevention under
fencing. They do not promise exactly-once remote execution or online safety for
incompatible old/new writers. Drain or use a reviewed compatible migration before
sharing an authoritative namespace with old owners.

## Verification and provenance

Run `cargo xtask check W11`. The native gate checks source Git blobs/current
hashes, immutable source corpus/exporter/guard hashes, the accepted F07 script
and private Valkey qualification, and named native tests. Each named required
case must appear and actually report success; ignored or absent cases fail.
No mandatory command executes Python. It uses only private disposable Valkey
processes, never an application URL, global flush or provider.

`crates/xtask/fixtures/w11/corpus.json` records actual source AST `plan_digest`
and `_normalize_record` outputs over a source-compatible full-record fixture.
Its original `expiry_equality` expressions were authored source-inspection
controls, not method execution. They are retained unchanged and superseded by
`source-controls.json`: actual source `StagedSearchStore.read/request_retry`
executed with a fixed clock and real private Valkey. It observes read success at
expiry equality, read expiry just afterward, and source Valkey retry expiry at
equality. The original exporter is retained as inert `evidence/export-v1.source.txt`.
The supplemental exporter uses stdlib imports and no backend imports.

Temporary developer-only recapture:

```sh
SLOPSEARX_TEST_VALKEY_SERVER=/path/to/pinned/valkey-server /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/w11/oracle/export.py
```

G08 removes executable Python before backend removal and retains immutable JSON.
The inventory/F01 registration proposal is `inventory-proposal.json`; shared
integration is parent-owned. Source-bound mapping and differences are in
`parity.json`; actual commands/hashes/results are in `qualification.json`.

Portal impact: native state APIs only. Active Python HTTP/MCP and portal routes
are unchanged. W06/W09/W10 surface integration must keep the shared policy/service
contracts and run the existing portal gate. This packet proves no deployed
workflow, live provider behavior, relevance improvement or production migration.

## Review correction: observed call conservation

Completion derives the observed-call aggregate from all immutable attempts and the completed current attempt. It rejects erasing a derivable total, including after retries with a known zero count, while preserving unknown totals when an earlier attempt has unknown observations. The real-Valkey `retry_completion_preserves_derivable_observed_total` regression verifies refusal leaves durable state unchanged, valid completion succeeds, and prior history is retained. Legacy record decoding remains permissive; the stronger check applies at the owned write boundary.
