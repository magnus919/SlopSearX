# Portal acceptance and release gates

This matrix is the release contract for milestone #9. It separates browser
overhead from upstream engine latency so a slow source is not misdiagnosed as a
slow interface.

## Browser matrix

The supported review matrix is the current stable release of Chromium, Firefox,
and Safari on a 1440×900 desktop viewport and a 390×844 narrow viewport. The
layout must remain usable from 320 CSS px through 200% zoom. Screen-reader
checks cover VoiceOver on macOS/iOS and NVDA on Windows when those environments
are available; unavailable environments are recorded as a validation gap.

## Accessibility gates

- WCAG 2.2 AA: keyboard-only search, scope selection, pagination, external
  links, and theme toggle.
- Every form control has a visible label or an equivalent accessible name.
- Result/state changes are announced through a restrained live region; focus is
  moved only when the user needs to recover from invalid input.
- Text contrast is at least 4.5:1 for normal text; focus/non-text indicators
  are at least 3:1. Meaning never depends on color alone.
- 200% zoom and reflow preserve query editing, source attribution, result
  activation, and pagination.
- `prefers-reduced-motion: reduce` removes decorative transitions and loading
  movement.

Reference: [WCAG 2.2](https://www.w3.org/TR/WCAG22/).

## Performance budgets

Measure with a local deterministic fake-engine response and record the browser,
viewport, network profile, and commit. Upstream search time is reported
separately in `meta.response_time_ms`.

| Budget | Target |
| --- | --- |
| Initial HTML shell | ≤ 50 KiB uncompressed and ≤ 15 KiB gzip |
| Portal CSS + JavaScript | ≤ 30 KiB uncompressed; no third-party runtime |
| First contentful paint | ≤ 1.5 s on a mid-tier laptop, simulated Fast 3G |
| Layout shift | CLS ≤ 0.10 through result render |
| Input response | ≤ 100 ms p95 for theme, focus, and disclosure interactions |
| Search handoff | Browser overhead ≤ 150 ms p95 after the response is available |

These are budgets for the portal layer, not a promise about third-party engine
latency. A budget exception requires a measured reason and a follow-up issue.

## Deterministic contract fixtures

The browser suite must cover:

1. normal results from multiple engines with one long title;
2. missing date, thumbnail, and optional metadata;
3. empty results with suggestions;
4. partial results naming an unavailable source;
5. all-source failure and cached error;
6. HTTP 400 invalid filter, 429 rate limit, 403 disabled format, and 503
   unavailable responses;
7. unsupported and rejected filters, including strict SafeSearch when no
   selected source enforces it;
8. malicious title, content, URL, and engine name strings;
9. page two and back/forward URL restoration;
10. Dark/Darker persistence with storage enabled and storage blocked.

The existing MCP `state_factory` harness and SearXNG compatibility suite remain
authoritative for machine behavior. Portal tests assert the HTML projection
without changing those contracts.

## Portal dependency map and gate ownership

The portal contract depends on the shared search request/response models and
`SearchService`/`ScopeResolver`; the live capability catalog and filter
enforcement resolver; MCP policy inputs for sensitive access; cache and
snapshot view derivation; HTTP route and format negotiation; engine result
schemas; layered configuration; and the Python package/container that serves
the formatter. These are shared backend dependencies, so the portal contract
job runs on every pull request instead of relying on frontend-only path
filters.

`tests/test_server.py` exercises the real FastAPI boundary with deterministic
fake engines and isolated test state. `tests/test_formatter.py` exercises the
HTML projection and escaping rules. The dedicated browser job loads those
same formatter outputs in Chromium and covers landing/search, theme toggle,
source attribution, safe result links, and pagination. A change that removes a
required result or scope field makes the contract assertions fail; a compatible
backend change continues through the same required checks. The normal test
matrix remains authoritative for the API/MCP suites, while these portal jobs
own browser-visible regressions and should be updated alongside any baseline
change.

## Privacy and operations

- No query analytics, third-party tracking, or remote assets by default.
- Request/engine metrics may retain bounded operational counters, but logs and
  traces must not include raw query text or credentials.
- Proxy authentication, TLS, and trusted-forwarded-header behavior are
  deployment concerns and must be documented for each public instance.
- Smoke test `/`, one deterministic HTML search, `/healthz`, and one machine
  format after deployment. Roll back on broken landing/search routes, policy
  bypass, XSS evidence, or a budget regression beyond the documented threshold.

Security review follows the [OWASP XSS Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html).

## Evidence record

Each release PR links the exact test commands, browser screenshots or run
record, measured budget output, and any environment gaps. A passing static test
is not a substitute for the manual visual and assistive-technology review.
