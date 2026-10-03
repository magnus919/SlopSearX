# Scholarly work identity and source provenance

SearchService groups source-reported publications before presence/RRF fusion and
before Jev's 40-candidate shortlist. A shared recognized DOI, PMID, PMCID or exact
arXiv identifier establishes identity. Known Nature article URLs map to their
10.1038 DOI. Equal titles, snippets, citations and indexing dates do not establish
identity. Conflicting known identifiers decline an ordinary merge.

Explicit source payload `relations` entries (`relation_type`, `doi`) may connect
`isVersionOf`, `hasVersion`, `isPreprintOf`, `hasPreprint`, `isNewVersionOf` or
`isPreviousVersionOf`. These are internal source-reported fields; adapters never
fetch related records to populate them. Generic relations establish identity,
not chronology. Only explicit new/previous chronology or a complete arXiv version
number series selects a newer member. A query naming an available exact arXiv
version selects that member. Ambiguous chronology keeps stable source order.
Publication and indexing dates alone never select a newer revision.

Each admitted member retains its engine, original feed position, URL, title,
snippet, publication date, identifiers and source payload in the internal
`work_group` cache/snapshot record. Each engine feed is compacted to one entry per admitted scholarly work before
fusion; original member positions remain in provenance. No engine contributes
more than once to a work's presence or RRF score. Existing tier priority remains intact. Groups have
at most 64 members and 64,000 bytes of member records; serialized internal records
have a separate 128,000-byte cap. A merge that exceeds the bound is declined,
so exceptionally large groups can still occupy multiple results. Individual source payloads beyond 64,000 bytes decline grouping and retain
the existing bounded-persistence limitation in the ordinary result record.
There is no additional HTTP or model fanout.

Source-reported retraction, withdrawal and correction types remain attached to
members. Known warnings are also surfaced as attributed source-member notices
in the representative's `comments`; this does not declare every version
retracted. A separate correction/retraction DOI remains a distinct result unless
an explicit permitted version relation establishes identity.

The SearXNG JSON contract retains `url` and `engine` strings and the `engines`
array. Supported optional Paper fields (`doi`, `authors`, `journal`, `publisher`,
`editor`, `pages`, `number`, `comments`, `type`, `pdf_url`, `html_url`, `issn`,
`isbn`, `tags`, `volume`) are type checked before JSON/YAML/MCP projection.
DOI is an identifier string. Alternative URLs/DOIs remain internal; no arbitrary
alternative array is added to the public contract. Existing `publishedDate`
keeps the selected member's publication date. New metadata remains optional.

The portal's source agreement indicator now includes the same identified work
across different URLs. Cache identity includes the grouping policy version.
This grouping measures distinct work coverage, not relevance or publication
validity; model agreement is not independent ground truth.

Contract source: [SearXNG Paper](https://docs.searxng.org/dev/result_types/main/paper.html).
