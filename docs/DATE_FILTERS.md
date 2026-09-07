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
page does not prove that OpenAlex has no matching recent works. When both absolute
and relative constraints appear on a shared SearchRequest, both apply.

A scope containing only OpenAlex reports `enforced`; OpenAlex plus other sources
reports `partially_enforced`. Other engines' results remain present and are not
claimed to satisfy the dates. Other-only scopes report `unsupported`. Unknown
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
