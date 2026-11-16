# Audited publication-date filtering

OpenAlex supports `date_from` and `date_to` on the science MCP tool and shared
SearchRequest. Both are inclusive ISO calendar dates (`YYYY-MM-DD`), independently
optional. Invalid dates, empty strings, timestamps and reversed ranges reject
before dispatch. These constrain the publication date, not the date OpenAlex
indexed or updated the work.

The adapter sends `from_publication_date` and `to_publication_date` in OpenAlex's
comma-separated `filter` parameter. Its audited declaration reports
`upstream:openalex`. As a defensive check, records missing a valid publication
date or outside the requested bounds are discarded before returning results.
Dates are never inferred from publication years, titles or URLs.

For generic/targeted/research `time_range`, OpenAlex declares `local:openalex`:
the service filters the returned page by `published_date`, using its existing
inclusive windows (day=1, week=7, month=30, year=365 days before the current server
calendar date, through today). Missing or malformed dates are excluded. This
local page filter does not search additional upstream pages, so an empty filtered
page does not prove that OpenAlex has no matching recent works. Absolute and relative constraints cannot be combined on a shared SearchRequest.

Absolute dates require an explicit OpenAlex-only scope. Mixed and unsupported
scopes reject before dispatch rather than retaining unbounded results. Relative
filter reports continue to describe partial or unsupported engine coverage. Unknown
relative values are rejected before dispatch when the scope advertises relative
date enforcement. Capability discovery includes `date_from` and `date_to` alongside
the existing filter keys. Language and SafeSearch behavior are unchanged.

Absolute bounds and the resolved relative calendar window are part of cache
identity. Reports are resolved against the actual selected scope on fresh and
cached responses. Snapshots preserve the captured results.

## Source audit

Reviewed September 6, 2026:

- [OpenAlex filtering](https://help.openalex.org/api/filtering/) documents the
  two publication-date filters and AND semantics for comma-separated filters.
- [Official DateField implementation](https://github.com/ourresearch/openalex-elastic-api/blob/master/core/fields.py)
  implements the bounds as `gte` and `lte` range predicates.
- [Work attributes](https://help.openalex.org/data/works/attributes/) defines
  `publication_date` as the publication day of the primary-location version,
  usually the earliest electronic publication date known for that version.

Regression tests use the real adapter with mocked HTTP responses to check
inclusive boundaries, malformed/missing dates, date serialization, mixed scopes,
cache separation and fail-closed validation. They do not measure the completeness
or correctness of OpenAlex's underlying publication metadata.

## HTTP absolute-date contract

`GET /search`, `GET /`, and form-encoded POST on either route accept inclusive
`date_from` and `date_to`. Supply `engines=openalex` explicitly. Absolute bounds
cannot be combined with `time_range`; mixed, implicit, unavailable, or unaudited
engine scopes reject before acquisition. Unknown `date*` controls reject rather
than silently disappearing. Existing non-date SearXNG extension parameters remain
accepted. The science MCP tool follows the same explicit scope requirement;
existing specialist permission grants are unchanged.

Successful bounded responses expose `meta.date_filter`: `requested` contains
nullable `date_from`, `date_to`, and `time_range` (null); `applied` contains the
same two nullable absolute bounds. `status` is `enforced`, `enforced_by` is
`["upstream:openalex"]`, `engines` is `["openalex"]`, and `warnings` is empty.
The proof derives from the admitted engine scope and audited adapter declaration,
not merely echoed controls. It survives cache serialization; older cache entries
without matching proof are ignored. No absolute dates means no new proof field.
HTTP acquisition failure still returns 503 and per-engine status; MCP returns
`all_engines_failed`. An enforcement report alone does not prove acquisition
success. Clients must validate both transport/outcome and matching proof.

Portal impact: no date picker is added. Date parameters arriving via portal URLs
use this same validation and shared service; JSON/YAML expose the proof while
HTML retains bounds in forms, pagination and machine-view links and shows audited
publication-date enforcement badges. Bounded CSV/RSS reject before dispatch because
those formats do not carry the proof; their no-date behavior is unchanged.
