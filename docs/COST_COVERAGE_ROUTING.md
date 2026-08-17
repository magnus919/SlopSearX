# Cost- and coverage-aware routing (issue 192)

The automatic router chooses the engine set for a request when the caller
does **not** pass an explicit `engines` list (and, on the MCP surface, does
not name explicit engines or a targeted evidence boundary). Before this
feature it was first-match and keyword-oriented: the topic router matched a
keyword to a fixed engine list, and the fallbacks dispatched the tier-1 set
or every active engine. This feature adds a **bounded, fully explainable**
selection pass over the live capability catalog so the automatic source mix
is budget-aware and evidence-coverage-aware while remaining deterministic.

This document covers the routing inputs and precedence, the operator budget,
the machine-readable explanation surface, the deterministic fallback, and
the scope of the evaluation. It is the companion to
`slopsearx/routing.py` (the strategy) and the routing pass in
`slopsearx/service.py` (`ScopeResolver`).

## What does not change

- **Explicit `engines` lists** and **targeted searches**
  (`slopsearx_search_targeted`, jobs, security, science, research subqueries)
  never enter the routing pass. Explicit source scope is preserved verbatim
  (out of scope: replacing explicit source scope with adaptive routing).
- **The deterministic keyword topic router** still runs first; the new pass
  operates on the topic/category/tier candidate sets.
- **No autonomous spending**: the cost/engine bounds only bite when an
  operator configures them.

## Routing inputs and precedence

The pass consumes, in this order:

1. **Policy eligibility** — disabled and sensitive engines are excluded by
   `ScopeResolver` before this pass (the shared policy gate; sensitive
   engines are only reachable via an explicit list or grant).
2. **Capability fit** — the candidate set is already the engines that serve
   the requested categories, matched topic, or evidence intent.
3. **Authentication/readiness** — an engine declaring `auth_class: required`
   without configured credentials is excluded (stage `auth`).
4. **Current health** — a circuit-open engine is excluded (stage `health`).
5. **Cost class** — engines beyond the configured budget `max_cost_class`
   are excluded (stage `budget`).
6. **Evidence coverage** — the scope is capped at `max_engines_per_intent`
   preferring healthy, cheaper, tier-1 engines; shortfalls against the
   coverage target are reported.

## Operator budget

Configured under `routing.budget` in `config.yaml`, with `ROUTING_*`
environment overrides (env wins):

```yaml
routing:
  enabled: true
  budget:
    max_engines_per_intent: 6   # 0 = unlimited
    max_cost_class: freemium    # free | freemium | paid (paid = allow all)
    coverage_target: 3          # preferred minimum engine count; 0 = off
```

| Env var                       | Overrides              |
| ----------------------------- | ---------------------- |
| `ROUTING_MAX_ENGINES_PER_INTENT` | `max_engines_per_intent` |
| `ROUTING_MAX_COST_CLASS`      | `max_cost_class`       |
| `ROUTING_COVERAGE_TARGET`     | `coverage_target`      |

Defaults are permissive: no engine-count cap, all cost classes allowed, no
coverage target. A typo can never invent a bound — invalid values are
ignored. Cost classes are the audited `COST_CLASSES` vocabulary
(`free` < `freemium` < `paid`; unknown sorts cheapest so an unassessed
engine is never penalized).

## Explainability

Every automatic routing decision is explained through the scope preview and
result metadata:

- **`excluded_engines`** — each entry carries `{engine, reason, stage}` where
  `stage` is one of `policy | auth | health | budget`.
- **`routing`** — `{fallback, budget_applied, tradeoffs}`:
  - `fallback: true` when no capability catalog was available and the
    deterministic fallback produced the scope;
  - `budget_applied: true` only when configured budget bounds actually
    excluded engines;
  - `tradeoffs: [{kind, detail}]` reports coverage traded for `cost`
    (budget) or `availability` (health), plus `coverage` shortfalls against
    the configured target.

These surface on `slopsearx_explain_search_scope` (dry-run preview) and on
every search envelope's `scope` block, and they survive the cache round-trip
(the canonical cached scope carries the routing explanation). The scope
preview and the executed scope share the same resolver, catalog, and budget,
so a preview always matches execution.

## Deterministic fallback

When the capability catalog (telemetry) is missing — for example a
deployment that wires the context without one — the automatic paths return
their candidates unchanged and record `routing.fallback: true`. When the
catalog is present but has no entry for a candidate engine, that engine is
**kept** (missing telemetry is conservative — never speculatively dropped).
Stale health observations are treated as unknown, never as current `ok`.

## Evaluation

`tests/test_routing_eval.py` measures the strategy against the
context-equivalent deterministic baseline (the candidate set the fallback
would dispatch) under declared fixtures and budgets. The rubric asserts:

- R1 the routed scope is a subset of the baseline (routing never adds
  engines);
- R2 configured bounds (engine count, cost class) are respected;
- R3 no ineligible (unauthenticated / circuit-open) engine is selected;
- R4 determinism across repeated runs;
- R5 when the cost bound actually excludes engines, the routed mix is never
  more expensive on average than the baseline (when the cost bound is not
  constraining, no cost claim is made);
- R6 coverage is not reduced below the budget floor.

**Scope of this evaluation** — it covers the declared fixtures (healthy /
degraded, authenticated / unauthenticated, cheap / expensive, high / low
coverage mixes) and query families (general, code, science) with **no
models** (the strategy is a pure deterministic function), scoped to the date
captured by `eval_scope()`. It claims only what it measures and does not
claim universal quality improvement from a single benchmark.
