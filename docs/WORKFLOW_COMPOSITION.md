# Workflow composition

SlopSearX workflows can continue from a retained artifact by passing its
versioned artifact reference as the optional `source` argument. Composition is
an MCP-only extension. It does not change the SearXNG HTTP routes or response
schema.

```json
{
  "source": {
    "contract": "slopsearx.artifact_ref",
    "version": 1,
    "kind": "snapshot",
    "id": "snap-..."
  }
}
```

The source contributes retained evidence and an immutable `derived_from`
lineage edge. It does not contribute grants, engine permission, a deadline, a
budget, or permission to broaden scope. The destination tool revalidates its
current grants and every engine it will dispatch before it persists or queues
the new operation.

## Transition matrix

| Source kind | Destination | Caller still supplies | Source contribution | Lifecycle behavior |
| --- | --- | --- | --- | --- |
| `snapshot` | `slopsearx_start_research` | `question`, research strategy or explicit plan, budget, deadline, and optional idempotency key | Snapshot evidence summary and lineage | Live snapshots are admitted; expired, missing, denied, and unavailable snapshots are rejected. |
| `saved_report` | `slopsearx_start_research` | `question`, research strategy or explicit plan, budget, deadline, and optional idempotency key | Retained report status and lineage | Comparable reports are admitted. Incomparable or truncated observations are rejected with a source lifecycle error. |
| `staged_search` | `slopsearx_start_research` | `question`, research strategy or explicit plan, budget, deadline, and optional idempotency key | The selected result snapshot and lineage | A live operation with a selected, live result snapshot is admitted. Incomplete, expired, denied, missing, and unavailable operations are rejected. |
| `snapshot` | `slopsearx_start_dependency_dossier` | Ecosystem, package, optional version/repository, deadline, and optional idempotency key | A bounded set of source result identities and lineage | A large snapshot is admitted with an explicit `truncated` source status. Other lifecycle failures are rejected. |
| `entity_group` | `slopsearx_start_dependency_dossier` | Ecosystem, package, optional version/repository, deadline, and optional idempotency key | Group member result identities and lineage | The group and parent snapshot must still be live and permitted. |
| `result` | `slopsearx_start_dependency_dossier` | Ecosystem, package, optional version/repository, deadline, and optional idempotency key | One retained result identity and lineage | The result and parent snapshot must still be live and permitted. |
| `saved_report` | `slopsearx_search_staged` | Objectives, initial and optional fallback scopes, expansion decision, filters, and idempotency key | The saved definition query and report lineage | The caller omits `query`; supplying both is an `input_conflict`. Incomparable or truncated reports are rejected. |
| `staged_search` | `slopsearx_export_research_manifest` | No result IDs | Result IDs from the selected live snapshot and lineage | Incomplete operations are rejected. More than 25 results produces a bounded manifest marked `truncated`. |
| `research_attempt` | `slopsearx_export_research_manifest` | No result IDs | Result IDs from the attempt snapshot and lineage | Only an attempt with a live captured snapshot is admitted. |
| `research_job` | `slopsearx_export_research_manifest` | No result IDs | Result IDs from completed attempt snapshots and lineage | Terminal or partial jobs with evidence are admitted. Missing evidence is rejected; more than 25 results is marked `truncated`. |

Direct `result_ids` remain supported by manifest export. A caller supplies
either `result_ids` or `source`, never both. Direct staged-search callers still
supply `query`; a saved-report source is the one transition that derives the
query so agents do not need to copy it.

## Errors and atomicity

Composition validates the entire source before calling the destination:

- `unsupported_source_contract` covers legacy versions and malformed artifact
  references;
- `unsupported_transition` covers a valid artifact kind with no matrix entry;
- `source_not_found`, `source_expired`, `source_unavailable`,
  `source_policy_denied`, `source_partial`, `source_incomparable`, and
  `source_truncated` report deterministic source lifecycle outcomes;
- `input_conflict` rejects source-derived fields supplied directly by the
  caller;
- `idempotency_conflict` rejects replay of an idempotency key with a different
  source or destination request.

Every rejection happens before destination persistence or dispatch. An
unsupported or unavailable source never falls back to a fresh unlinked search.
Accepted source summaries are bounded and stored with the destination lineage;
reading or composing does not extend the source retention period.

## Portal impact

This contract adds no public portal action or browser-visible field. The portal
continues to use the shared search service without composition inputs. A later
authenticated workflow console can use the same MCP transition matrix and
artifact references rather than inventing a browser-only composition path.
