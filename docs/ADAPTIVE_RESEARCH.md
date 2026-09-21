# Caller-directed adaptive research

Research tools require `MCP_GRANT_RESEARCH=1` and connected Valkey. They extend
MCP research jobs; ordinary SearXNG HTTP requests keep their existing behavior.
SlopSearX executes bounded searches and records evidence. The caller decides
which follow-ups are useful and whether the evidence answers a question.

## A model-independent workflow

Call `slopsearx_start_research` with these arguments (engine names must be active
and permitted on the server):

```json
{
  "question": "Compare maintenance and alternatives",
  "max_queries": 3,
  "max_attempts": 3,
  "max_engine_attempts": 3,
  "max_results": 30,
  "subquestions": [
    {"id": "maintenance", "question": "Is the project maintained?"},
    {"id": "alternatives", "question": "What alternatives exist?"}
  ],
  "initial_plan": [
    {
      "query": "project maintenance release history",
      "engines": ["wikipedia"],
      "subquestion_id": "maintenance",
      "rationale": "Establish a baseline before comparing alternatives"
    }
  ]
}
```

Poll `slopsearx_get_job(job_id)` until its state leaves `queued`/`running`.
Read a query attempt's `cursor` with `slopsearx_read_results`. A `plan_executed`
stop reason means the submitted plan ran; it does not mean the investigation
is complete or the evidence is sufficient.

Call `slopsearx_extend_research` with the same `job_id`, a new `query`, explicit
`engines` or an `intent`, and optional `subquestion_id`, `rationale`,
`parent_attempt_id` and `continuation_key`. The parent must be a terminal
attempt in this job. The continuation key provides request idempotency: replay
with the same normalized query, scope and metadata returns current status;
reuse with different contents returns `idempotency_conflict`. Read-only replay
remains available after completion or deadline expiry while the record is
retained and current caller/scope policy permits access. A busy worker
response means the request was not appended; retry after that worker finishes.

Record your own assessment with `slopsearx_update_research`:

```json
{
  "job_id": "<returned job id>",
  "subquestion_states": {"maintenance": "resolved"},
  "complete": true,
  "rationale": "Enough evidence for this task; alternatives remain open"
}
```

This preserves the unresolved `alternatives` subquestion, sets
`caller_completed` and, for an otherwise active job, retains the historical
`stop_reason: caller_completed` behavior. If bounded execution already stopped
with `query_budget_exhausted`, `result_budget_exhausted`,
`attempt_budget_exhausted`, or `engine_budget_exhausted`, completion is
metadata-only: the execution `state` and `stop_reason` remain unchanged, while
the caller flag and rationale are
recorded separately. This metadata-only completion remains allowed after the
dispatch deadline under the same fenced lease; it never reopens retry or
follow-up dispatch. Identical completion requests replay the persisted summary;
different rationale or requested subquestion state maps (including an omitted
map) return `idempotency_conflict`. Older completed records without a stored
request map accept a matching rationale and nonempty subquestion state assertion
that matches the retained state. Empty maps conflict because equivalence cannot
be proven. Omit `complete` to update progress and keep the job open. Progress
is a caller declaration, not SlopSearX certification.

## Budgets and evidence

Positive integer limits are clamped to operator ceilings and persisted. A
later policy change can lower those ceilings but cannot enlarge the original
limits or reset usage. `max_queries` bounds distinct planned queries;
`max_attempts` counts executions including retries; `max_engine_attempts`
counts reserved engine slots across executions; `max_engines_per_query`
bounds each query's scope. Defaults come from operator research limits.

Attempts and engine slots are reserved durably before dispatch. If a worker
dies after reservation, recovery marks its outcome `interrupted` and retains
the charge, even if the upstream request might not have happened. A replacement
attempt consumes a new reservation. An expired lease cannot authorize a late
worker to overwrite a newer owner's history.

`max_results` bounds admitted provenance records, not upstream result counts.
Every admitted record consumes a slot, including repeated URLs. Attempts expose
`result_count` for total discovered results, `admitted_result_ids` for admitted
snapshot records and `new_lead_ids` for newly seen normalized URL identities.
URL novelty is not semantic novelty or verification. Full canonical snapshots
remain readable until their normal expiry; admission never truncates them or
the shared search cache. Terminal attempt records retain their original cursors,
errors, coverage and enforcement reports across retries.

Job summaries expose `budgets.limits`, `used`, `remaining`, subquestions and
operational `stop_reason`. Budget exhaustion is distinguished from caller
completion, cancellation, execution failure and deadline expiry. Deadline
expiry stops further dispatch; already returned evidence remains readable.
Omitting discovered records because of the admission cap reports
`result_budget_exhausted`; exactly filling the cap without omissions can still
report `plan_executed`. When all planned queries succeed and their count reaches
the job's query cap, the retained job instead reports
`query_budget_exhausted`, unless a result, attempt, or engine budget reason
already applies. A follow-up beyond `max_queries` still receives the existing
pre-admission `job_budget_exceeded` response. In every budget case, no new
dispatch occurs and `caller_completed` stays false until the caller explicitly
completes the job. Storage TTL expiry is separate: once
stored records expire, their handles are no longer available.

Every dispatch, including retry and recovered work, rechecks current research
and engine policy through the shared gate. Explicit custom-plan and follow-up
intents also require their specialist grants. Existing strategy templates retain
their research-grant behavior. Older stored queries without explicit grant
provenance require the specialist grant when resumed; this conservatively
protects legacy continuations after grant revocation. Neither path bypasses
sensitive-engine policy.

## Rollout and rollback

Drain existing research workers before enabling this version. Older workers do
not understand cumulative reservations or caller completion and must not consume
adaptive jobs alongside new workers. Rollback requires draining adaptive jobs
before restoring older research workers. Existing HTTP search traffic does not
activate these jobs. Research records and snapshot handles retain their normal
retention limits; this feature adds no permanent database.
