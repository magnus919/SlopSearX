"""Unit tests for the cost/coverage-aware routing strategy (issue 192).

Locks in the bounded, deterministic selection contract:

- routing inputs and precedence (policy → capability fit → auth →
  current health → cost class → evidence coverage);
- hard exclusions before dispatch: unauthenticated (auth) and circuit-open
  (health) engines are never selected;
- configured budget bounds (cost class, per-intent engine count) shape the
  source mix with machine-readable reasons, and coverage-for-cost /
  coverage-for-availability trade-offs are reported;
- missing telemetry is conservative: an engine the catalog cannot assess is
  kept, never speculatively dropped;
- selection is fully deterministic: identical inputs produce identical output.
"""

from __future__ import annotations

import pytest

from slopsearx.capabilities import EngineCapability
from slopsearx.config import Config, RoutingConfig
from slopsearx.routing import (
    EXCLUSION_STAGE_AUTH,
    EXCLUSION_STAGE_BUDGET,
    EXCLUSION_STAGE_HEALTH,
    TRADEOFF_AVAILABILITY,
    TRADEOFF_COST,
    TRADEOFF_COVERAGE,
    RoutingBudget,
    load_routing_budget,
    routing_inputs,
    select_cost_coverage,
)


class _FakeCatalog:
    """Minimal catalog stand-in exposing ``get(name) -> EngineCapability | None``."""

    def __init__(self, caps: dict[str, EngineCapability]) -> None:
        self._caps = caps

    def get(self, name: str) -> EngineCapability | None:
        return self._caps.get(name)


def _cap(
    name: str,
    *,
    auth_class: str = "none",
    auth_configured: bool = False,
    circuit_open: bool = False,
    cost_class: str = "free",
    status: str = "unknown",
    stale: bool = False,
) -> EngineCapability:
    return EngineCapability(
        name=name,
        display_name=name,
        engine_type="api",
        categories=["general"],
        enabled=True,
        auth_class=auth_class,
        auth_configured=auth_configured,
        scope_hints=[],
        caveats=[],
        cost_class=cost_class,
        last_known_status=status,
        last_known_status_stale=stale,
        circuit_open=circuit_open,
    )


def _catalog(*caps: EngineCapability) -> _FakeCatalog:
    return _FakeCatalog({cap.name: cap for cap in caps})


class TestRoutingInputsPrecedence:
    def test_precedence_is_documented_in_order(self) -> None:
        assert routing_inputs() == [
            "policy eligibility",
            "capability fit",
            "authentication/readiness",
            "current health",
            "cost class",
            "evidence coverage",
        ]


class TestHardExclusions:
    def test_unauthenticated_required_key_engine_excluded(self) -> None:
        catalog = _catalog(_cap("shodan", auth_class="required", auth_configured=False))
        result = select_cost_coverage(["shodan"], catalog)
        assert result.selected == []
        assert result.exclusions[0].engine == "shodan"
        assert result.exclusions[0].stage == EXCLUSION_STAGE_AUTH
        assert "credentials" in result.exclusions[0].reason

    def test_configured_key_engine_kept(self) -> None:
        catalog = _catalog(_cap("shodan", auth_class="required", auth_configured=True))
        result = select_cost_coverage(["shodan"], catalog)
        assert result.selected == ["shodan"]
        assert result.exclusions == []

    def test_circuit_open_engine_excluded(self) -> None:
        catalog = _catalog(_cap("brave", circuit_open=True))
        result = select_cost_coverage(["brave"], catalog)
        assert result.selected == []
        assert result.exclusions[0].stage == EXCLUSION_STAGE_HEALTH
        assert "circuit" in result.exclusions[0].reason

    def test_circuit_closed_engine_kept(self) -> None:
        catalog = _catalog(_cap("brave", circuit_open=False))
        result = select_cost_coverage(["brave"], catalog)
        assert result.selected == ["brave"]

    def test_healthy_cheap_mix_preserved(self) -> None:
        catalog = _catalog(
            _cap("wikipedia"),
            _cap("brave", auth_class="required", auth_configured=True, cost_class="freemium"),
        )
        result = select_cost_coverage(["wikipedia", "brave"], catalog)
        assert result.selected == ["wikipedia", "brave"]

    def test_missing_telemetry_is_conservative(self) -> None:
        """An engine the catalog cannot assess is kept, never speculatively dropped."""
        catalog = _catalog(_cap("wikipedia"))
        result = select_cost_coverage(["wikipedia", "no_such_engine"], catalog)
        assert result.selected == ["wikipedia", "no_such_engine"]
        assert result.exclusions == []


class TestBudgetBounds:
    def test_cost_class_bound_excludes_expensive_engine(self) -> None:
        catalog = _catalog(
            _cap("wikipedia", cost_class="free"),
            _cap("dehashed", cost_class="paid"),
        )
        budget = RoutingBudget(max_cost_class="free")
        result = select_cost_coverage(["wikipedia", "dehashed"], catalog, budget)
        assert result.selected == ["wikipedia"]
        assert result.exclusions[0].stage == EXCLUSION_STAGE_BUDGET
        assert result.budget_applied is True

    def test_cost_bound_allows_at_or_below(self) -> None:
        catalog = _catalog(
            _cap("brave", auth_class="required", auth_configured=True, cost_class="freemium"),
            _cap("wikipedia", cost_class="free"),
        )
        budget = RoutingBudget(max_cost_class="freemium")
        result = select_cost_coverage(["wikipedia", "brave"], catalog, budget)
        assert set(result.selected) == {"wikipedia", "brave"}

    def test_unknown_cost_class_is_not_penalized(self) -> None:
        catalog = _catalog(_cap("mystery", cost_class=""))
        budget = RoutingBudget(max_cost_class="free")
        result = select_cost_coverage(["mystery"], catalog, budget)
        assert result.selected == ["mystery"]

    def test_max_engines_cap_keeps_cheapest_healthy(self) -> None:
        catalog = _catalog(
            _cap("wikipedia", cost_class="free"),
            _cap("arxiv", cost_class="free"),
            _cap("dehashed", cost_class="paid"),
        )
        budget = RoutingBudget(max_engines=2)
        result = select_cost_coverage(["wikipedia", "arxiv", "dehashed"], catalog, budget)
        assert len(result.selected) == 2
        assert "dehashed" not in result.selected  # most expensive dropped
        assert result.budget_applied is True
        assert result.exclusions[0].stage == EXCLUSION_STAGE_BUDGET

    def test_max_engines_cap_prefers_healthier_then_cheaper(self) -> None:
        catalog = _catalog(
            _cap("sick_free", status="error"),
            _cap("ok_free", status="ok"),
            _cap("ok_paid", status="ok", cost_class="paid"),
        )
        budget = RoutingBudget(max_engines=2)
        result = select_cost_coverage(["sick_free", "ok_free", "ok_paid"], catalog, budget)
        assert result.selected == ["ok_free", "ok_paid"]

    def test_stale_ok_is_not_trusted_as_current_health(self) -> None:
        catalog = _catalog(
            _cap("fresh_ok", status="ok", stale=False),
            _cap("stale_ok", status="ok", stale=True),
            _cap("other", status="unknown"),
        )
        budget = RoutingBudget(max_engines=2)
        result = select_cost_coverage(["fresh_ok", "stale_ok", "other"], catalog, budget)
        # fresh_ok first; stale_ok and other are both treated as unknown and
        # tie-broken by name order ("other" < "stale_ok").
        assert result.selected == ["fresh_ok", "other"]

    def test_permissive_default_budget_is_inert(self) -> None:
        catalog = _catalog(_cap("wikipedia"), _cap("dehashed", cost_class="paid"))
        budget = RoutingBudget()  # unlimited count, paid allowed
        result = select_cost_coverage(["wikipedia", "dehashed"], catalog, budget)
        assert result.selected == ["wikipedia", "dehashed"]
        assert result.budget_applied is False


class TestTradeoffs:
    def test_cost_tradeoff_reported(self) -> None:
        catalog = _catalog(
            _cap("wikipedia", cost_class="free"),
            _cap("dehashed", cost_class="paid"),
        )
        budget = RoutingBudget(max_cost_class="free")
        result = select_cost_coverage(["wikipedia", "dehashed"], catalog, budget)
        kinds = {t.kind for t in result.tradeoffs}
        assert TRADEOFF_COST in kinds

    def test_availability_tradeoff_reported_for_circuit_open(self) -> None:
        catalog = _catalog(_cap("wikipedia"), _cap("brave", circuit_open=True))
        result = select_cost_coverage(["wikipedia", "brave"], catalog)
        kinds = {t.kind for t in result.tradeoffs}
        assert TRADEOFF_AVAILABILITY in kinds

    def test_coverage_shortfall_reported_against_target(self) -> None:
        catalog = _catalog(_cap("wikipedia"))
        budget = RoutingBudget(coverage_target=3)
        result = select_cost_coverage(["wikipedia"], catalog, budget)
        assert any(t.kind == TRADEOFF_COVERAGE for t in result.tradeoffs)

    def test_no_tradeoffs_when_nothing_excluded(self) -> None:
        catalog = _catalog(_cap("wikipedia"), _cap("arxiv"))
        result = select_cost_coverage(["wikipedia", "arxiv"], catalog)
        assert result.tradeoffs == []


class TestDeterminism:
    def test_identical_inputs_produce_identical_output(self) -> None:
        catalog = _catalog(
            _cap("wikipedia", cost_class="free"),
            _cap("brave", auth_class="required", auth_configured=True, cost_class="freemium"),
            _cap("dehashed", cost_class="paid"),
            _cap("shodan", auth_class="required", auth_configured=False, circuit_open=False),
        )
        budget = RoutingBudget(max_engines=2, max_cost_class="freemium", coverage_target=2)
        first = select_cost_coverage(["wikipedia", "brave", "dehashed", "shodan"], catalog, budget)
        second = select_cost_coverage(["wikipedia", "brave", "dehashed", "shodan"], catalog, budget)
        assert first.selected == second.selected
        assert [(e.engine, e.stage) for e in first.exclusions] == [(e.engine, e.stage) for e in second.exclusions]
        assert [(t.kind, t.detail) for t in first.tradeoffs] == [(t.kind, t.detail) for t in second.tradeoffs]

    def test_candidate_order_does_not_change_selection(self) -> None:
        catalog = _catalog(
            _cap("wikipedia", cost_class="free"),
            _cap("arxiv", cost_class="free"),
            _cap("brave", cost_class="freemium"),
        )
        budget = RoutingBudget(max_engines=2)
        forward = select_cost_coverage(["wikipedia", "arxiv", "brave"], catalog, budget)
        backward = select_cost_coverage(["brave", "wikipedia", "arxiv"], catalog, budget)
        assert forward.selected == backward.selected


class TestLoadRoutingBudget:
    def test_defaults_are_permissive(self) -> None:
        budget = load_routing_budget(Config(routing=RoutingConfig()))
        assert budget.max_engines == 0
        assert budget.max_cost_class == "paid"
        assert budget.coverage_target == 0

    def test_yaml_section_applied(self) -> None:
        config = Config(
            routing=RoutingConfig(budget={"max_engines_per_intent": 4, "max_cost_class": "free", "coverage_target": 2})
        )
        budget = load_routing_budget(config)
        assert budget.max_engines == 4
        assert budget.max_cost_class == "free"
        assert budget.coverage_target == 2

    def test_env_overrides_win(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = Config(routing=RoutingConfig(budget={"max_engines_per_intent": 4, "max_cost_class": "free"}))
        monkeypatch.setenv("ROUTING_MAX_ENGINES_PER_INTENT", "9")
        monkeypatch.setenv("ROUTING_MAX_COST_CLASS", "paid")
        budget = load_routing_budget(config)
        assert budget.max_engines == 9
        assert budget.max_cost_class == "paid"

    def test_invalid_values_ignored(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = Config(routing=RoutingConfig(budget={"max_engines_per_intent": "nope", "max_cost_class": "bogus"}))
        monkeypatch.setenv("ROUTING_MAX_ENGINES_PER_INTENT", "also-bogus")
        monkeypatch.setenv("ROUTING_MAX_COST_CLASS", "not-a-class")
        budget = load_routing_budget(config)
        assert budget.max_engines == 0
        assert budget.max_cost_class == "paid"

    def test_zero_engines_means_unlimited(self) -> None:
        config = Config(routing=RoutingConfig(budget={"max_engines_per_intent": 0}))
        budget = load_routing_budget(config)
        assert budget.max_engines == 0

    def test_boolean_budget_value_is_not_a_bound(self) -> None:
        """A boolean (e.g. ``true``) can never be coerced into an engine bound."""
        config = Config(routing=RoutingConfig(budget={"max_engines_per_intent": True}))
        budget = load_routing_budget(config)
        assert budget.max_engines == 0
