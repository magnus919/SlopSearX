# Artifact lineage

SlopSearX MCP responses identify durable and derived workflow artifacts with an
additive, versioned reference:

```json
{"contract":"slopsearx.artifact_ref","version":1,"kind":"snapshot","id":"snap-..."}
```

Version 1 has a closed kind vocabulary: `snapshot`, `result`, `entity_group`,
`research_job`, `research_attempt`, `staged_search`, `saved_search`,
`saved_report`, `retrieval_receipt`, `research_manifest`, and
`dependency_dossier`. Composite identities are opaque. Clients must preserve
the complete reference instead of parsing its `id`.

## Lineage graph

`slopsearx_get_artifact_lineage` reads existing records and returns
`slopsearx.artifact_lineage` version 1. It accepts `direction` (`outgoing`,
`incoming`, or `both`), `max_depth` from 0 through 5, and `max_nodes` from 1
through 100. Nodes and edges have deterministic breadth-first order. The
`truncated` field is true when either bound prevents an expansion.

Each node contains its artifact reference and one lifecycle status: `live`,
`expired`, `missing`, `unavailable`, or `policy_denied`. A legacy snapshot that
predates stored lineage remains readable and reports
`details.lineage_status: lineage_unavailable`. This makes the gap explicit
without migrating or extending the artifact.

Edges are directed and use these relations:

- `derived_from`: the source artifact is derived from the target;
- `contains`: the source aggregate includes the target;
- `selected`: the source workflow selected the target;
- `observed`: the source records an observation of the target;
- `retrieval_of`: the source receipt records retrieval of the target result.

Every expansion uses the authenticated tenant and the current grants and
sensitive-engine policy. A denied reference reports `policy_denied` without
confirming whether the underlying artifact exists. The reader performs no
engine dispatch, page retrieval, write, lease renewal, or retention extension.
An ephemeral research manifest reports `unavailable` when queried after its
export response because manifests are not persisted.

## Manifest export

`slopsearx_export_research_manifest` continues to accept `result_ids`. It also
accepts `artifacts`, plus `max_depth` and `max_nodes`. Each artifact is resolved
through the same bounded lineage reader. Live result nodes in the selected cut
are included, and the exact graph is recorded in `lineage_cuts`. Explicit and
discovered results are deduplicated in input order and remain limited to 25.
Export reads retained observations only; it does not fetch a result URL or
claim that an observation is verified.

## Compatibility and retention

Artifact references are additive fields at MCP serialization boundaries. They
do not change the SearXNG-compatible HTTP JSON response. Existing receipt and
manifest contract versions remain readable. Relationships are embedded in the
records that own them or derived from existing records; SlopSearX does not add
a durable graph database. Each source record keeps its original Valkey TTL.

For rollback, deploy a reader that ignores the additive `artifact`, `lineage`,
and `lineage_cuts` fields. Existing snapshots and workflow records remain
valid.
