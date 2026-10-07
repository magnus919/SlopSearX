# GDELT adoption decision — issue #277

Decision on 2026-10-07: **no-go for adding a standard built-in adapter in this delivery**.
This completes the current adoption evaluation, not a claim that GDELT is
permanently unsuitable. No adapter, production configuration, or default
engine set changes. Reopen adoption when a new bounded capture demonstrates
availability inside the existing search latency budget and supports the
remaining filter, attribution and relevance qualifications below.

## Live evidence

Four free public DOC requests, no account or paid service, no automatic retries.
The first three ran sequentially with five seconds after completion, requesting
10 ArtList JSON results across one week, sorted by hybrid relevance:

| Query | Outcome | Elapsed seconds |
|---|---|---:|
| climate change | HTTP 429 | 10.17 |
| semiconductor | HTTP 200, 10 articles | 15.19 |
| elections | HTTP 429 | 10.19 |

A subsequent semiconductor request for five articles across 24 hours exceeded
a ten-second socket timeout (17.565 seconds total including connection setup).
The first three used a 25-second socket timeout. These are client elapsed times;
no server timing or exact per-request UTC timestamp was captured. All calls
were made on 2026-10-07. HTTP error bodies and headers were not retained, so
Retry-After and actual throttling rules are unknown. Shared egress reputation,
request spacing, or temporary upstream conditions may explain the 429s.
The sample does not estimate general availability. It does fail to qualify
this integration within the existing typical three-second engine timeout.

[Captured evidence](gdelt-277/observations.json) records parameters, timeout,
spacing and baseline SHA. The successful GDELT response is preserved in
[the article fixture](gdelt-277/gdelt-semiconductor.json); no publisher page
bodies were fetched. Data citation: [The GDELT Project](https://www.gdeltproject.org/).
Underlying article rights remain with publishers.

## Incremental coverage and limitations

Compare the same semiconductor query to the existing free Hacker News Algolia
story source, using its native numeric filter for the preceding seven days:
HTTP 200 in 0.350 seconds, two results. The
[baseline fixture](gdelt-277/hackernews-semiconductor-week.json) retains only
public URLs, titles and story submission times. None of its two URLs equals
any of the ten GDELT URLs. GDELT contributes ten distinct URLs across seven
hostnames, including Vietnamese and Korean outlets. One India semiconductor
industry article has an English title; the remaining original-language titles
require translation or a fluent reader for relevance assessment. Therefore ten
additional URLs is coverage evidence, not ten verified useful results or a
production relevance improvement. Hacker News submission time and GDELT
observation time differ, and HN is a narrow technology community; comparison
against broad web/news sources remains unqualified. No credentials were used.

Distinct article URLs are not distinct events. Two pairs visibly concern
similar semiconductor developments; URL deduplication does not remove syndicated
or translated reporting of the same event. The captured `seendate` is an
observation timestamp, not verified article publication time. It must not map
to `published_date` without independent publisher evidence.

## Current official contract and adoption requirements

[DOC documentation](https://blog.gdeltproject.org/gdelt-doc-2-0-api-debuts/)
was rechecked on 2026-10-07. It describes English-keyword searching across
65 machine-translated languages, a rolling three-month date window,
STARTDATETIME/ENDDATETIME bounds within that window, and ArtList caps of 75
by default and 250 maximum. No offset or cursor pagination is documented.
Live returned titles retain their original Vietnamese/Korean language, correcting
the prior issue comment's claim that non-English titles return translated.
The sampled request did not exercise maximum windows, language filters or
explicit dates; documentation alone does not qualify their live enforcement.
The API does not provide an audited publication-date field in this capture.
Machine tone is a derived signal, not a factual judgment, and was not requested.

[Project data access](https://gdeltproject.org/data.html) describes free open
data; [terms](https://www.gdeltproject.org/about.html) permit commercial use and
redistribution with mandatory project citation and website link. Dataset rights
do not transfer publisher article rights. No DOC quota, guaranteed service level
or precise rate-limit schedule was found in those official references. Public
access requires no key; BigQuery billing is a separate path not exercised here.

A future accepted adapter must remain Tier 2 under `news`, with specialist Jev
routing for global multilingual news discovery. It needs pooled HTTP lifecycle,
classified timeouts/429/non-JSON errors, safe public result URLs, sanitized
captured-response replay and bounded upstream contract coverage. Declare only
audited filters: do not silently clamp unsupported long date windows, call
observation dates publication dates, advertise unsupported pagination, or infer
language/SafeSearch enforcement. Citation plus link must survive HTTP, MCP,
portal, cached and deduplicated representations; operator docs alone cannot
establish citation for every redistributed result. Evaluate this with shared
policy/cache/serialization and portal contracts before enablement.

## Portal and validation

This PR contains documentation and inert JSON evidence only. Browser-visible
behavior, capability catalog, policy, cache and result formats are unchanged;
portal implementation coverage and graphify AST update do not apply. JSON
fixtures were parsed locally and the captured URL overlap/counts checked.
No synthetic replay is represented as a live adapter qualification. This
decision records the bounded failure and concrete conditions for a future go.
