# Dependency dossiers

Dependency dossiers are an opt-in MCP workflow that links package registry,
GitHub repository, and NVD advisory search records without turning search hits
into security or maintenance verdicts.

Enable all three grants:

```text
MCP_GRANT_DEPENDENCY_DOSSIER=1
MCP_GRANT_RESEARCH=1
MCP_GRANT_SECURITY=1
```

The workflow currently supports `pypi` and `npm`. Start it with an
ecosystem-qualified name. A repository is optional and, when supplied, must be
`owner/repository` or a canonical HTTPS GitHub URL.

```json
{
  "ecosystem": "pypi",
  "package": "requests",
  "version": "2.32.3",
  "repository": "psf/requests",
  "idempotency_key": "requests-2.32.3-review"
}
```

Call `slopsearx_start_dependency_dossier`, then poll
`slopsearx_get_dependency_dossier` with the returned job ID. The read tool does
not issue searches. Each section preserves its query, immutable snapshot
cursor, result IDs, source timestamps, and engine coverage.

Package identity resolves only from an attributed package payload whose
ecosystem-normalized name exactly matches the request. A popular near-match or
same-named package in another ecosystem is only a candidate. The requested
version and observed registry version remain separate.

An explicit repository is labeled `caller_supplied`; it is not proof that the
repository owns or publishes the package. Without one, a registry-provided
GitHub root URL may be reported as unverified metadata, but v1 does not search
that candidate automatically. Multiple candidates remain ambiguous.

NVD results are advisory leads. Every advisory section reports applicability
as `not_evaluated` because keyword search does not establish affected package
versions. Empty advisory results mean only that the selected source returned no
matching leads. Missing credentials, rate limits, timeouts, and partial source
coverage remain visible.

This workflow uses existing research deadlines, Valkey leases, snapshots, and
tenant isolation. Admission is atomically idempotent in Valkey. A dossier gets
at most two attempts per planned source, bounded by the operator's research
query limit, and shares the research result budget across initial execution and
retries. Current grants and the complete captured engine scope are checked
before every dispatch and read. It adds no HTTP route or search parameter, so
SearXNG clients retain the same request and response behavior.
