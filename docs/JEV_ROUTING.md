# TypeSafe Jev specialist routing

SlopSearX can use [TypeSafe Jev](https://typesafe.ai/) to decide which
specialist engines are worth adding to an unscoped search. This is a shipped,
optional routing path: setting `TYPESAFE_API_KEY` enables it, while an absent
key preserves the deterministic router and requires no TypeSafe account.

## Behavior

- The ordinary general-search base remains selected by SlopSearX.
- One Jev request scores every currently eligible specialist in parallel.
- Every specialist scoring at least `0.65` is added. There is no result-count
  or engine-count cap in the Jev selection step.
- Explicit `engines`, `categories`, and media searches remain caller-directed
  and do not invoke Jev.
- Disabled, unauthenticated, circuit-open, and sensitive engines are excluded
  before candidate scoring. Jev cannot bypass SlopSearX policy.
- A timeout, malformed response, provider error, or model mismatch fails open
  to the original deterministic scope. Search remains usable without Jev.
- The first result from each responding Jev-selected specialist is promoted
  into the visible result set in score order; remaining results retain the
  configured ranker's order.

The response records successful additions under `meta.jev_routing`, including
the selected engine names and their scores. The API key is never included in
logs, cache keys, responses, or routing metadata.

JSON and YAML HTTP output, including `ssx` output, carry this metadata. MCP
search and scope-preview output carry the same additions under `jev_routing`.
The web portal names added specialists beside the result count. Routing scores
describe estimated retrieval fit; they are not confidence that a result is true.
MCP scope preview does not dispatch search engines, but may incur one billable
Jev request on a cache miss. A later search can reroute if eligibility changes.

## Configuration

```bash
export TYPESAFE_API_KEY='...'
```

No separate enablement flag is required. Docker/Compose deployments may pass
the variable through their environment or configured env file. A project-root
`.env` file is ignored by Git, but the application process must actually load
or receive that file; a bare Python process does not automatically read it.
The HTTP API/portal and direct SlopSearX MCP run as separate services in the
GroktoCrawl production Compose stack (the MCP companion uses the `mcp` profile).
The stack must pass the key to each container that should use Jev. Compose's
interpolation `.env` is not, by itself, a container environment declaration;
the GroktoCrawl stack maps a nonempty `TYPESAFE_API_KEY` to both services as the
operator's explicit opt-in. Verify presence without printing the key.

Optional tuning variables are available for operators who need them:

| Variable | Default | Meaning |
|---|---:|---|
| `JEV_SPECIALIST_THRESHOLD` | `0.65` | Inclusive score required to add a specialist. |
| `JEV_TIMEOUT_MS` | `1000` | Maximum time allowed for the routing request. |
| `JEV_ROUTING_CACHE_TTL_SECONDS` | `3600` | Valkey lifetime for a query's routing decision. |

The `0.65` default is the registered winner from the synthetic threshold
experiment documented in [EXP-014](experiments/EXP-014-jev-specialist-threshold/README.md).

## Routing cards

The production cards live in `slopsearx/jev.py`. Broad engines have an explicit
`general` or `broad_support` role and are not sent to Jev. Every built-in
specialist has a concise `purpose` and `use_when` card. The completeness test
fails when a built-in adapter is added or removed without updating this routing
metadata.

When adding an engine:

1. Decide whether it is `general`, `broad_support`, or `specialist`.
2. For a specialist, add a factual `purpose` and a discriminating `use_when`;
   describe when it adds evidence that ordinary web search is unlikely to.
3. Keep policy, credentials, cost, and runtime health out of the prose card;
   those are enforced from live SlopSearX state before Jev sees candidates.
4. Run `pytest --no-cov -q tests/test_jev.py`.

Changing cards, the model, or the threshold changes routing behavior and must
include focused tests plus a portal impact review.
