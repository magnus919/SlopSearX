# SlopSearX portal UX specification

This is the accepted first-release interaction contract. It deliberately
keeps the portal useful before exposing every engine knob.

## Audiences and jobs

| Audience | Job to be done | First-release success |
| --- | --- | --- |
| Curious visitor | Ask one question and understand where the answer came from | Search submission is obvious; the first useful result is identifiable; source and freshness are visible |
| Researcher | Narrow a broad search without losing the query | Scope/filter changes are visible in the URL and survive reload/share |
| Operator or engineer | Explain why coverage is partial or unavailable | Source health, unsupported filters, and degraded states are truthful and non-secret |

Accounts, saved collections, autocomplete providers, research workspaces, and
AI summaries remain later work. They are not implied by this specification.

## Information architecture

```text
Portal
├── Home (/)
│   ├── query field
│   ├── optional scope chooser
│   └── theme preference
└── Search (/search)
    ├── persistent query editing
    ├── scope/filter summary
    ├── result list with source attribution
    ├── truthful state banner (partial, empty, unavailable, rate-limited)
    └── numbered previous/next pagination
```

The header keeps the brand, query field, and theme control together. Results
use one reading column with a compact scope rail on wide screens; the rail
stacks above results below the small-screen breakpoint. A user can always
return to the same query by copying the URL.

## Primary journeys

1. **First search:** open `/`, focus the query field, enter text, press Enter,
   and arrive at `/search?q=...`. Focus returns to the query field after a
   failed submission; it does not jump to a decorative element.
2. **Refine scope:** open the scope control, select a capability-backed
   category, apply, and see the selected scope in both the control and URL.
   Unsupported controls are omitted or disabled with a short reason.
3. **Inspect provenance:** each result shows title, safe destination, source
   engine(s), category/type, and publication metadata when present. Missing
   metadata is omitted rather than replaced with invented values.
4. **Recover from partial search:** a partial banner names the unavailable or
   empty sources and keeps successful results usable. An all-source failure
   explains that no result was available and offers a retry.
5. **Navigate and share:** previous/next controls preserve every active
   parameter. Filter changes reset to page one; browser Back/Forward restores
   the complete URL state.
6. **Choose visual mode:** the toggle is labelled `Darker mode` when Dark is
   active and `Dark mode` when Darker is active. The setting is explicit and
   local; it is never inferred from system preference.

## Labels and truthful filter behavior

- `All sources` means the service's configured automatic scope, not every
  adapter in the repository.
- Category labels come from the live capability catalog. Engine names are
  shown only when the operator exposes them to the browser.
- A filter is labelled `Applied` only when the selected scope enforces it.
  `Passed to some sources` and `Not supported by this scope` are separate
  states. Strict SafeSearch is unavailable when the selected scope cannot
  satisfy it.
- Result provenance uses `Source`/`Sources`, never a vague `verified` label.

## State inventory

| State | Required content | Action |
| --- | --- | --- |
| Landing | One clear query field, short explanation, configured-source count when available | Submit query |
| Loading | Query retained, progress cue that respects reduced motion | Wait or cancel when supported |
| Success | Results, source labels, filter/scope summary, pagination | Open result, refine, navigate |
| Empty | Query echoed, plain explanation, safe suggestions if supplied | Edit query or broaden scope |
| Partial | Successful results plus named unavailable/empty sources | Retry or continue with available results |
| All unavailable | Service/source explanation, query retained, retry | Retry |
| Invalid | Field-specific explanation near the control and in a live region | Correct input |
| Rate limited | Calm explanation without leaking policy internals | Wait and retry |
| Restricted scope | Explain that the operator has not enabled the scope | Choose another scope |

## Prototype and walkthrough

The running portal is the interactive prototype. Start the application with a
deterministic fake-engine fixture, open `/`, and walk these URLs in order:

```text
/
/search?q=valkey
/search?q=valkey&categories=packages&pageno=2
/search?q=valkey&time_range=month&safesearch=2
```

The browser review must repeat the walk at 1440×900 and 390×844, with a long
title, missing publication date, mixed engine sources, an empty response, and
a partial response. The scenario and expected assertions are maintained in
[`docs/PORTAL_ACCEPTANCE.md`](PORTAL_ACCEPTANCE.md).

## Evaluation

The first-release proxy task is: “Find one useful result, identify its source,
and share the same filtered search URL.” A reviewer passes when all three are
completed in two minutes on desktop and three minutes on a narrow viewport,
without reading documentation. This is a product acceptance target, not a
claim about unmeasured user research. Maintainer walkthroughs and any
representative participant checks are recorded in the linked issue before
release.
