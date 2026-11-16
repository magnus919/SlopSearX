# W04 source mapping and intentional differences

Source identity: source.json pins SHA256 for saved_models.py, saved_runner.py,
saved_store.py and all three starting source test files at baseline
1bad48bbe4e25e2f60d5b86668e92bbce922fdf0. Native gate verifies current files
against both SHA256 and the baseline Git blobs. Golden case IDs are exact gate
inputs; each report checks source-compatible canonical JSON bytes via SHA256.

| Source behavior/regression | Native evidence |
| --- | --- |
| test_saved_models: added/changed/not-observed and source values | changes, published_date_only, published_date_empty golden cases |
| ranking ignored | ranking golden case |
| incomparable outage/cache/truncation/provenance preserves baseline | outage/cached/truncated/provenance/provenance_set_changed cases and retention Valkey test |
| baseline expiration/window resets | expired/window golden cases |
| source URL identity, ambiguity, UTF8 byte bounds | ambiguous/unicode/utf8_oversize/empty_url cases; native oversize-report test |
| source sorted multiengine provenance | provenance_order and provenance_set_changed cases |
| exact report size limit and empty records/events on excess | report_size_uses_ascii_spaced_bytes_and_retains_prior_baseline_without_panicking |
| test_scheduled_run_bypasses_canonical_cache | runner_revocation_shutdown_outer_drop_and_uncached_execution injects a populated canonical cache; zero reads and actual engine call |
| midflight saved grant revocation | same native runner test mutates actual injected authority before upstream completion; no report/baseline |
| current/retained old scope reads | publication_is_atomic_fenced_and_tenant_bound and retained_old_scope_is_revalidated_after_definition_changes |
| failed run retains baseline | retained_baseline_and_report_eviction_keep_independent_horizons; pure incomparable golden cases |
| missed slots, immediate creation slot, full results before bounding | scheduled_due_batch_preserves_missed_slots_and_unsliced_observation; W00 exact schedule corpus; runner success contract |
| duplicate/stale/cross-tenant commits, update/pause/delete fencing | publication_is_atomic_fenced_and_tenant_bound; update_pause_delete_reclaim_and_malformed_key_preflight_prevent_partial_writes |
| completion-time report order, bounded reports/physical deletion and independent retention | retained_baseline_and_report_eviction_keep_independent_horizons; completion timestamp is ZADD score |
| persisted claim identity/restart | killed_process_recovery_preserves_slot_and_publishes_once kills actual native owner, expires private lease, reclaims identical run ID and proves one report |
| ownership renewal, lease loss, outer drop/shutdown and held socket cleanup | held_loopback_renews_authority_and_lease_loss_joins_only_owned_socket; runner_revocation_shutdown_outer_drop_and_uncached_execution |
| definition quotas/revisions/repair/tenant scans/tombstones | accepted W00 gate, unchanged ownership interfaces |

## Reviewed intentional hardening

Source _COMMIT overwrites the tenant-wide due-index TTL and report-index TTL
with the current record horizon. A short-lived definition/report could shorten
an index containing longer-lived members. Native publication extends these
index TTLs and never shortens them, consistent with W00 authoritative repair.
The two-definition retained-horizon test proves this change. Individual
records retain their independent TTLs and logical expiry checks.

The runner keeps renewal ownership through publication (the Python
runner stops renewal immediately after acquisition). Comparison itself is bounded
synchronous CPU work and does not concurrently tick renewal; the store token and
raw CAS remain the final fencing boundary. Lease loss cancels and
joins owned acquisition. This is the W04 lifecycle requirement and follows the
accepted W02 ownership pattern. Native operational dispatch errors are labeled
SearchError (timeout is TimeoutError); they do not claim Python exception-class
identity across languages. Both produce dispatch_failed incomparable reports.

## Mandatory downstream obligations — not covered or accepted as complete here

- `test_expired_lease_recovery_normalizes_running_gauge` module-global metric
  assertions are **NOT YET COVERED**. State recovery and duplicate-publication
  fencing are exercised here. O01/W09 own concrete workflow metric composition,
  including running→interrupted→running normalization and recovery counters.
- Production dynamic grant/config hot reload is not implemented by static
  native SearchService/SavedAuthority composition. W09 must select the actual
  authority lifecycle and wire the fresh SavedAuthority port; the mutable
  authority fixture proves the runner's boundary behavior only.
- W05/W09 must compose report/baseline/outbox publication atomically. This
  script intentionally has no event stream or event-delivery claim.
- W09/W10 own wire errors, artifact/lineage envelopes, report-list pagination,
  authenticated saved tools and portal behavior. No route is exposed here.
- G04/G08 own mixed-version namespace/data rehearsal and removal of the isolated
  temporary source exporter. Unknown accepted provider/write outcomes remain
  unknown; recovery never promises exactly-once external dispatch.

## Cancellation negative control

Restoring a direct `store.now().await` only at the post-acquisition observation
TIME read causes the real CLIENT PAUSE regression to return
`Store(Store(Unavailable))` instead of `Shutdown`. Pinned redis1.7.1 has a 500ms
inner default response timeout beneath the configured 2s Valkey outer budget;
cleanup is separately bounded at 250ms. This is causal cancellation-classification
evidence. The mutant did not violate the 800ms post-signal join bound, so this
experiment does **not** establish a join-latency improvement. Exact source is
restored after the isolated negative control. Corpus/source drift and ignored/
missing required-contract mutations also return nonzero before acceptance.
