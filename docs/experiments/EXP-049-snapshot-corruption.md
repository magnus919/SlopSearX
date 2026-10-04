# EXP-049: Fail closed on malformed persisted snapshots

## Registration

- State: registered; start 2026-10-04T13:02:02.496Z, Eastern scheduled cycle.
- Baseline: 3d5e8ba. Isolated worktree; offline, zero paid calls, 45-minute ceiling.
- Opportunity: snapshot pagination, entities and detail reads deserialize Valkey
  records without handling field failures. Non-finite expiry compares false and
  can expose malformed records. Code inspection is the initial evidence;
  fault prevalence in production is unknown. Priority: shared read-path failure
  and lifecycle integrity, reproducible offline, affects three agent surfaces.
- Last seven completed experiments reviewed: EXP-045 inconclusive; EXP-046,
  044, 043, 042 and 040 fail selection gates; EXP-041 supported and merged #529.
  EXP-047/048 are incomplete delivery/assessment work under #516, not supported
  runtime candidates. No supported unmerged candidate. Do not repeat ranking
  selection without new independent evidence or paid authorization.
- Intended before/after: malformed snapshot currently raises or returns evidence;
  candidate returns existing invalid_cursor errors, no results or dispatch.
- Evidence class: normal-path deterministic defect replay, not task success.
- Primary: fraction of 24 fault reads returning invalid_cursor with no evidence,
  no exception and no engine calls. Minimum: 100% candidate, >=50 percentage
  point gain over baseline, justified by fail-closed integrity of stored evidence.
- Fixed faults (one field changed per authentic captured record): non-object
  scope, non-object result item, invalid total, invalid created_at, invalid
  expires_at, NaN expires_at, positive infinity expires_at, NaN created_at.
  Persist through disposable real Valkey and call ordinary MCP read_results,
  read_entities and read_result functions. No provider calls or new classifier.
- Controls: healthy, expired, missing, cross-tenant, legacy absent expires_at,
  disconnected store, each across the three paths (18 cases). Preserve existing
  result/expired_handle/invalid_cursor/store_unavailable behavior, lineage and
  legacy JSON-safe engines. Invalid records must not be silently repaired or
  deleted; no logs containing query, payload, credentials or exception text.
- Candidate scope: deserialize fault handling plus finite timestamp validation
  in shared SnapshotStore.read; preserve lifecycle/error schema and policy.
- Fixed corpus exhaustive; no statistical interval or population relevance claim.
  Retain baseline, all candidate trials, worker and checksums. No threshold tuning.
- Commands: PYTHONPATH=. .venv/bin/python evidence worker with EXP049_URL pointing
  only to disposable Valkey and EXP049_OUTPUT to a fresh directory; same worker
  and fields at baseline and candidate. Required targeted snapshot/entity/MCP,
  portal contracts/browser, full regression, mypy, hooks, graphify, CI/review.
- Portal review: shared snapshot reader; no browser UI or policy change. Existing
  portal gates required; docs clarify corrupt snapshots use existing invalid
  handle errors. No replayed query, refresh dispatch, monitoring or alerts.
- Supported only if primary and all controls/guardrails pass; otherwise retain
  not-supported/inconclusive/blocked outcome and evidence before discarding code.
- Delivery: ready implementation PR if supported; exact-SHA CI and substantive
  review before authorized merge, docs-only ledger auto-merge, no deployment.
