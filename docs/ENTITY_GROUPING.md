# Entity groups from search snapshots

Call `slopsearx_read_entities(cursor, page=1, max_results=10)` after a search
to see relationships between source-reported identifiers. This optional MCP
read does not search again, fetch pages, alter rankings, or change the flat
results returned to SearXNG clients. No new configuration or state is required.

## Version 1 contract

The response identifies `contract: slopsearx.entity_groups`, `version: 1`,
the original `cursor` and `query`, and the requested `page`. `entities` contains:

| Field | Meaning |
| --- | --- |
| `entity_id` | Stable `entity-v1-` SHA256 identifier, or null for an unresolved singleton. |
| `namespace` | `cve`, `npm`, `pypi`, or null. |
| `identifier` | Normalized CVE ID or package name and version; null when unresolved. |
| `reason` | `explicit_adapter_identifier` or a reason identity could not be established. |
| `result_ids` | Original snapshot member IDs; expand with `slopsearx_read_result`. |
| `conflicting_fields` | Sorted shared payload field names with differing reported values. No winner is selected. |

Groups are computed across the entire captured snapshot, ordered by the first
member's original position, then paginated. **`max_results` counts entities**,
subject to the same operator bounds as other read tools. It does not limit
members within an entity; members are compact references, and a group is never
split across pages. Each captured result appears exactly once. Missing identity
produces a singleton rather than hiding the result.

`meta.total_entities`, `meta.total_results`, and `meta.unresolved_entities`
describe the complete snapshot view; `has_more` describes entity pagination.
`meta.query_id` links to the originating execution. An out-of-range page is
empty. Expired, unknown, foreign-tenant, and unavailable snapshots follow the
same errors as `slopsearx_read_results`. Group reads do not extend retention.

## Supported identifiers

Only version-one `security/vulnerability` and `packages/package` payloads are
eligible. Identity fields must be declared in `provenance.adapter_fields`,
absent from `inferred_fields`, and attributed to a contributing engine.

- CVEs: explicit `cve_id` matching `CVE-` plus four year digits and four or more
  sequence digits, compared in uppercase.
- npm releases: payload provenance must name `npm`; retain scoped names and
  their exact case. Names contain ASCII letters, digits, dots, underscores,
  and hyphens, with an optional `@scope/` prefix.
- PyPI releases: payload provenance must name `pypi`; lowercase names and
  replace runs of `.`, `_`, and `-` with `-`.

Package coordinates include the exact source-reported version. A missing
version is null and remains distinct from an explicit version; version aliases
are not inferred. npm and PyPI packages never merge across ecosystems.
An explicit `ecosystem` conflicting with the source yields an unresolved
singleton. Malformed, whitespace-bearing, over-512-character, unreported, or
inferred identity fields remain unresolved. An explicit null/invalid version
is unresolved; absent version is the supported unknown-version coordinate.

IDs hash the UTF-8 encoding of compact, sorted-key JSON
`[1, namespace, identifier]` (ASCII escaped). They are independent of cursor,
ranking, URL, and input order; member result IDs remain snapshot-specific.

Differences in shared non-identity payload fields are exposed as
`conflicting_fields`. Missing one-sided facts are not conflicts. A differing
description or reference list is an observation of metadata variation, not a
finding that sources contradict each other. Follow the member IDs to inspect
the original records and engine provenance.

## Boundaries and example

For results at indices 0 and 2 reporting `CVE-2024-12345`, the first entity's
`result_ids` are `["<cursor>:0", "<cursor>:2"]`. A result at index 1 reporting
another CVE becomes the second entity, even when requesting one entity per page.

This view describes the canonical merged results captured by SlopSearX, not
every raw upstream hit or all possible results on the web. Existing URL
deduplication may already have combined contributing engines. Entity identity
does **not** establish source independence, corroboration, affected-version
applicability, or verification. Snapshots from before this feature work without
migration; unsupported payloads remain singleton records.

DOI, job identity, semantic matching, and page-content comparisons are outside
version 1. No URL canonicalization is added: the view reuses original result
IDs, leaving canonical URL identity work in [#307](https://github.com/magnus919/SlopSearX/issues/307).

## Version 2 contract

Pass `version=2` to opt into scholarly, repository, and advisory identities.
Version 1 remains the default and retains its original fields, IDs, grouping,
and one-result-to-one-group conservation rule. Version 2 uses `entity-v2-`
IDs and adds a top-level `relationships` list. One result can contribute to
more than one v2 entity when its source explicitly reports aliases.

Version 2 retains CVE, npm, and PyPI rules and adds these authorities:

| Namespace | Accepted explicit value | Normalization |
| --- | --- | --- |
| `doi` | `doi`, `doi:` value, or direct `https://doi.org/` URL | Percent-decode the resolver path, normalize Unicode to NFC, case-fold, and require a `10.` registrant prefix plus suffix. |
| `pmid` | `pmid`, or PubMed's explicit `publication_id` | Preserve one to nine non-zero decimal digits. |
| `pmcid` | `pmcid` | Uppercase `PMC` plus one to nine non-zero decimal digits. |
| `openalex` | `openalex_id` or direct HTTPS OpenAlex work URL | Uppercase `W` plus 4–15 digits; queries and fragments are rejected. |
| `github_repository` | `repository` or `repository_url` | Accept `owner/repository` or a direct HTTPS GitHub repository URL, remove `.git`, and case-fold. Extra paths, redirects, queries, and fragments are not followed or inferred. |
| `ghsa` | `ghsa_id` or `advisory_id` | Uppercase the canonical `GHSA-xxxx-xxxx-xxxx` alphabet. |
| `osv_advisory` | `advisory_id` | Uppercase supported `PYSEC`, `RUSTSEC`, and `GO` identifiers. |

Every field must appear in `adapter_fields`, must be absent from
`inferred_fields`, and must be attributed to a contributing engine. Titles,
authors, hostnames, result URLs, and similar text never establish identity.
Malformed, oversized, inferred, or conflicting same-namespace values remain
unresolved with a stable reason. Equivalent spelling produces the same ID
regardless of cursor, result order, rank, or score.

Relationships never merge namespaces. Each record contains
`from_entity_id`, `relation`, `to_entity_id`, and the source `result_ids` that
reported the relationship:

- `source_reported_alias` connects distinct scholarly identifier namespaces;
- `candidate_repository` connects a package release to an explicitly reported
  GitHub repository candidate without asserting ownership;
- `source_reported_advisory` connects a CVE to an explicitly reported GHSA or
  supported OSV advisory without asserting affected-version applicability.

The projector performs no search, page fetch, redirect resolution, DNS lookup,
or semantic matching. Flat snapshot results and every SearXNG HTTP response
remain unchanged.
