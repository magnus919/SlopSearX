# Portal impact review

These adapters change native availability and emit book/media/archive fields
that the portal will display. A09 preserves accepted result schemas, existing
capability/Jev cards and unsupported filter declarations. It does not flip
catalog activation or Internet Archive's opt-in default. TMDB and Open Library
retain thumbnails and complete canonical payloads where actually produced;
MusicBrainz/Nominatim/Internet Archive do not gain invented media.

Actual native Open Library omission/identity parsing feeds accepted ranking,
the existing URL parser's six-component accessor and the HTTP result projection.
The original full JSON formatter output is captured and its identity, parsed_url,
content and media/payload fields are asserted in the native successor. Actual
TMDB parsing feeds ranking/HTTP projection with thumbnail and payload/provenance
intact. The additive accessor moves the existing parser's six strings; it does
not establish all urllib behavior or change parser semantics. The additive
thumbnail projection keeps existing push behavior and the same budget owner.

The accepted malformed provider-field boundary returns an atomic adapter Error,
which the eventual portal must report without partially exposing preceding
rows. Archive closest-snapshot diagnostic must stay visible, and empty CDX must
not be relabeled as missing history or trigger another request. T03/G01 must
exercise these exact results through native HTML/browser rendering; T02/T06/G01
must qualify actual public routes, compact result projection and snapshots.
Existing Python portal CI remains active. DTO/unit source fixtures do not prove
full portal or transport behavior. Parent delivery owns applicable portal CI.
