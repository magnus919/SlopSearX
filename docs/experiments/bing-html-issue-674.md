# Issue #674: keyless search investigation and Bing HTML decision

Date: 2026-10-09. Outcome: Wikipedia defect fixed; Bing HTML candidate not adopted.
This is a retrospective diagnostic evidence ledger, not a preregistered experiment
or a statistical evaluation of Bing search quality.

## Request and disposition

[Issue #674](https://github.com/magnus919/SlopSearX/issues/674) reported empty
keyless general searches on Windows 11 and a residential connection. It proposed
a Wikipedia User-Agent fix and a Bing HTML adapter.

The Wikipedia fix merged in [PR #675](https://github.com/magnus919/SlopSearX/pull/675)
on October 6 at `f6a3f0e4bf9dd67b111b46ba3d1f7bdaa1ac3d9e`. Its default
User-Agent includes the project URL and supports the engine's `user_agent`
override. These address Wikimedia's contact-information requirement. A direct
opensearch request with that default header returned HTTP 200 and the titles
`Kubernetes` and `Kybernetes` during this investigation.

On October 9, the maintainer directed that the issue be closed as partially
resolved and the remaining Bing proposal, including
[PR #676](https://github.com/magnus919/SlopSearX/pull/676), be declined. Available
live evidence did not establish useful retrieval for the reported queries.
Returning enough external URLs was insufficient to meet the actual outcome.
No Bing adapter or new rate-limiter implementation was adopted by this work.

## Independent investigation boundary

The issue report, maintainer review text on #676, current repository code and
contribution guidance were inspected. **PR #676's implementation was not read,
executed, or used.** This decision concerns the independently observed provider
responses, not a new code review of that contribution.

GitHub `main` was inspected at `19dd0ff58a4dcd27bc47df1789bb0ef124a80588`.
The documentation branch subsequently advanced to
`cb6c82ac262617a3ced892ac17925fb7fda430ac` before recording these findings.
The original checkout and its unrelated work were preserved in an isolated
managed worktree. No runtime files were changed.

## Observations

Direct HTTP requests to `https://www.bing.com/search` used Chrome-style browser
headers. Local macOS probes used HTTPX and lxml; a read-only Linux probe used
Python's urllib. The HTML search input echoed the exact submitted query.
The observed HTML contained `li.b_algo` result blocks and redirect links whose
`u=a1...` payloads could be decoded into external URLs.

| Submitted query | Initial local response | Example observed titles and decoded targets |
| --- | --- | --- |
| `Kubernetes 1.34 release date` | HTTP 200, 11 organic blocks | “The Kubernetes Security and Observability Summit”: `https://events.ringcentral.com/events/the-kubernetes-security-and-observability-summit`; “v.ringcentral.com”: `https://v.ringcentral.com/conf/on/095594678`; “Getting Started With OpenTelemetry in Kubernetes - RingCentral”: `https://events.ringcentral.com/events/opentelemetry-workshop-kubernetes-072820`; “RingCentral Video”: `https://v.ringcentral.com/conf/on/373980512`; “RingCentral - Sign In”: `http://login.ringcentral.com/` |
| `Rust 1.90 release notes` | HTTP 200, 10 organic blocks | “Rust - Explore, Build and Survive”: `https://rust.facepunch.com/`; “Rust on Steam”: `https://store.steampowered.com/app/252490/Rust/`; “Rust Programming Language”: `https://rust-lang.org/`; “Install Rust - Rust Programming Language”: `https://rust-lang.org/tools/install/`; “Rust (programming language) - Wikipedia”: `https://en.m.wikipedia.org/wiki/Rust_(programming_language)` |

The inspected top-five results did not answer the requested release-date or
version-specific release-notes questions. The presence of some Rust language
pages did not establish that the version constraint was satisfied. This record
does not claim that every result on every returned page was irrelevant.

Additional local diagnostics retained the exact query:

- Kubernetes with `mkt=en-US` and `count=10`, plus shared browser Accept and
  Accept-Language headers: the ten inspected titles concerned RingCentral
  events, conferencing, sign-in, APIs, education, SDKs and presentations.
- Rust with `form=QBLH`, `qs=n`, `sp=-1`, `setlang=en-US`, `cc=US` and
  `Cache-Control: no-cache`: titles included the game, Steam, language homepage,
  installation, Wikipedia, companion app, GitHub, playground and a tutorial.
- Kubernetes with `form=QBRE`, `ensearch=1` and `filters=ex1:""`: the same
  unrelated RingCentral title list.
- Kubernetes with spaces encoded as `%20` instead of `+`: the same unrelated
  RingCentral title list.
- The Linux urllib request for Kubernetes also returned HTTP 200 and the same
  unrelated RingCentral title list.

No standard proxy environment variables were present in the local process.
Independent public egress for the two execution environments was not established.
Response bodies were not archived; the retained evidence is the bounded tool
output summarized above. No provider ranking root cause was established.

## Reproduction

Run a bounded direct HTML probe with the existing development dependencies:

```python
import httpx
from lxml import html

query = "Kubernetes 1.34 release date"
response = httpx.get(
    "https://www.bing.com/search",
    params={"q": query},
    headers={
        "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Accept-Language": "en-US,en;q=0.9",
    },
    timeout=15,
)
page = html.fromstring(response.text)
print(response.status_code, page.xpath('//input[@name="q"]/@value'))
for link in page.cssselect("li.b_algo h2 a")[:10]:
    print(link.text_content())
```

A future probe must preserve its own outcome; live provider responses can change.
Limit future requests across all test processes to two per rolling minute,
spaced at least 30 seconds apart. Do not repeatedly vary requests to obtain a
favorable result or infer relevance from HTTP status and result counts alone.

## Requirements retained for any future proposal

The maintainer's October 6 review required Tier 2 ranking, including service-level
unscoped coverage; validated original and decoded HTTP(S) destinations using
parsed hostnames; safe handling of malformed URLs; exclusion of Bing-internal
hosts even with explicit ports; complete adapter documentation; and required CI,
including the portal gate. Tier-1 promotion requires separate broader evidence
and explicit approval. General category and Jev role are separate from ranking.

The October 9 discussion added shared Valkey admission for human-facing engines,
including existing Google and DuckDuckGo adapters: at most two outbound requests
per rolling 60 seconds per provider, with at least 30 seconds spacing across
replicas, and fail-closed behavior when shared state is unavailable. Cache hits
must not consume allowance; every retained bootstrap, fallback, health probe,
retry or redirect request must be admitted at the outbound boundary. Ordinary
capacity exhaustion must not cause permanent engine deactivation. DDG's optional
bootstrap cannot consume allowance and then immediately bypass pacing for search.
These are future design requirements, not shipped behavior.

## Limits, alternatives and reopening trigger

These were provider HTTP probes, not full-service or production checks. Wikipedia's
rich-query stage and unscoped live searches were not exercised in this run.
The reporter's Windows installation was unavailable. No broader query cohort,
statistical quality conclusion or universal claim about Bing is supported.
Existing adapter tests, rate-limiter design and portal contracts were inspected;
no implementation was produced and no runtime tests were needed for this record.

Accepting five external links as proof of useful retrieval, changing ranking to
Tier 1, or shipping a parser while the original queries remained unqualified were
rejected. The maintainer chose not to pursue the Bing portion of #674.

Reconsider only through a new proposal with fresh, independently reproducible
relevant results for the reported queries, broader everyday-query/environment
coverage, and the retained safety and pacing requirements. No automatic retry,
Tier-1 promotion or revival of the declined implementation is implied.

Documentation prepared with OpenAI Codex on behalf of the maintainer.
