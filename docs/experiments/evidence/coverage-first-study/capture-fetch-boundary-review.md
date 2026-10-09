# Capture fetch-boundary review

Status: source inspection only; live capture remains disabled.

The candidate identity retained in this preparation is revision
`b5c1bf473bec78f3ebbfb630213b08612b3264fb`. A read-only review of that exact
public source found an initial hostname/IP guard, but did not establish
destination restrictions at every actual outbound connection.

The normal scrape path checks a destination before ordinary tier fetching.
Adapter dispatch precedes that check. HTTP clients subsequently resolve hosts
and follow redirects independently, and browser/FlareSolverr fetching delegates
additional requests. The initial check therefore does not establish that all
redirects, DNS changes, or browser subresources remain on permitted destinations.

Relevant public source at the inspected revision:

- [Main scrape dispatch and initial guard](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/fetch.py#L420)
- [Shared hostname/IP guard](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/common/url.py#L221)
- [HTTP and browser fetching](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/fetch_tiers.py)
- [Adapter shared page fetch](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/adapters/_helpers.py#L14)

This review made no service calls and inspected no private deployment settings.
Network-level protections, if any, were not observed. It does not establish that
an internal destination was actually contacted or that a deployment was exploited.

Before enabling real study capture, qualification must establish restrictions
at the actual connection boundary, covering adapters, redirect hops, DNS changes,
browser requests and delegated fetching. A caller-side DNS precheck or a receipt
that merely asserts safety is insufficient. Any required implementation or
deployment change must be reviewed and its exact runtime identity resealed before
study admission. The fixed quality gates and the no-retry policy remain unchanged.
