# Source behavior and ownership boundaries

Open Library keeps work → first valid edition → first ASCII-shaped ISBN
precedence and omits unidentified records before assigning contiguous positions.
Work/edition digits use Unicode16 decimal semantics; ISBN accepts ASCII digits,
terminal X/x and shape only, without claiming checksum validation. Its thumbnail
is preserved; no invented media object or coordinate payload is added.

MusicBrainz searches the artist endpoint once despite the broader source module
description. It preserves explicit User-Agent/Accept, query/limit/fmt, fixed score,
falsey lifespan handling, optional content joins and Python nested stringification.
It does not invent release or recording fallback.

TMDB uses Authorization: Bearer only, fixed page 1 and configured local slice.
Movie/TV/other branches, Unicode16 capitalize, display-prefix truncation, full
media-item overview and ordered field provenance survive. Missing and null vote
remain distinct from zero/false/empty conversions. The composition root snapshots
F06 secrets; adapters never reread ambient credentials. Direct raw config key
fallback observations and YAML/env precedence evidence remain separate. Invalid
credentials, HTTP bodies and library exceptions never expose secret values in
native diagnostics. The test credential strings are synthetic.

Nominatim retains exact q/format/limit/addressdetails and User-Agent/Accept.
Truthy osm_type AND osm_id selects an OSM item; otherwise details.php uses place_id.
Numeric zero importance becomes .5 while string "0" becomes 0. Title truncation
uses 100 codepoints and an ellipsis only when longer. Coordinates are not added
to a payload absent in the source.

Internet Archive routes the actual Python Unicode16 word/suffix domain predicate,
keeps successful empty/header-only CDX authoritative without fallback, and makes
at most one Availability request after CDX failure. CDX gets half the configured
aggregate budget; fallback gets remaining time intersected with SearchContext.
No retry or extra quota admission is invented. Availability can return one
validated closest snapshot and explicitly disclaims CDX history. It verifies
trusted outer host, scheme, userinfo/port/query/fragment restrictions, timestamp
and original hostname using source semantics, including case-sensitive embedded
scheme matching and empty final-param/query/fragment normalization. CDX rows
retain the source's weaker timestamp/nonempty-original checks; that is not an
Availability URL-validation guarantee. Catalog timeout and HTTP429 remain Error
as in the source, while Wayback has the explicit blocked/timeout/rate taxonomy.

Per-search source JSON has the accepted 4 MiB/depth/text/integer limits. A09's
32 MiB owned-operation ledger covers produced result capacity, scratch string
construction, source envelopes, thumbnails and projection overlap until the
named public response handoff. HTTP/request/source DOM and cold immutable
Unicode tables have their accepted separate owners. Public response retention,
clones, cache/flight lifetime, global process RSS and arbitrary downstream
serializers are not established by these tests. Cancellation drops per-search
operations; shared HTTP shutdown remains the composition owner's responsibility.

F06 intentionally ignores ENGINE_* variables for engines absent from its default
entry set until YAML creates their entries. Tests create explicit entries before
environment overrides and guard the fully resolved URL before registration.
The raw adapter defaults and F06 default-entry behavior are not conflated.

G01 owns aggregate production registration and active/credential/health selection;
T02/T06/G01 own public transport and snapshot/read-result contracts. T03/G01 own
portal HTML/browser media, payload, omission and archive-diagnostic rendering.
G02 owns retained response/clone accounting; G08 owns producer retirement.
