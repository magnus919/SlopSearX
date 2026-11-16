from __future__ import annotations

import json
import math

import pytest

from scripts import intent_ranking_coverage as coverage
from slopsearx.rerank import INSTRUCTIONS as PRODUCTION_INSTRUCTIONS
from slopsearx.rerank import LEVELS as PRODUCTION_LEVELS
from slopsearx.rerank import MODEL

FACETS = [
    {"id": "coverage", "description": "Directly useful evidence about coverage."},
    {"id": "cost", "description": "Directly useful evidence about cost."},
    {"id": "risk", "description": "Directly useful evidence about risk."},
]


def card(cid: str, *, url: str | None = None) -> dict:
    return {
        "id": cid,
        "title": f"Title {cid}",
        "url": url or f"https://example.test/{cid}",
        "snippet": f"Snippet {cid}",
    }


def compile_for(cards=None, facets=None, incumbent=None, **kwargs):
    cards = cards or [card("a"), card("b"), card("c")]
    facets = facets or FACETS[:2]
    incumbent = incumbent or [item["id"] for item in cards]
    return coverage.compile_request(
        query="public query",
        purpose="compare useful evidence",
        facets=facets,
        candidates=cards,
        incumbent_order=incumbent,
        **kwargs,
    )


def response_for(compiled, *, scores=None, profiles=None, probabilities=None, model=MODEL, usage=None):
    scores = scores or {}
    profiles = profiles or {}
    probabilities = probabilities or {}
    answers = {}
    for candidate in compiled.candidates:
        cid = candidate["id"]
        score = scores.get(cid, 7)
        score_probabilities = {str(level): float(level == score) for level in range(10)}
        answers[f"score:{cid}"] = {
            "type": "score",
            "score": score,
            "legend": {str(index): label for index, label in enumerate(coverage.LEVELS)},
            "probabilities": score_probabilities,
            "confidence": 0.75,
        }
        qid = f"profile:{cid}"
        choice = profiles.get(cid, "facets:" + compiled.facets[0]["id"])
        if choice.startswith("facets:"):
            chosen_ids = choice.removeprefix("facets:").split(",")
            choice = "facets:" + ",".join(sorted(chosen_ids, key=lambda item: item.encode()))
        probs = probabilities.get(cid)
        if probs is None:
            probs = {option: float(option == choice) for option in compiled.profile_options[qid]}
        answers[qid] = {"type": "choice", "choice": choice, "probabilities": probs, "confidence": 0.8}
    return json.dumps(
        {"model": model, "answers": answers, "usage": usage or {"input_tokens": 10, "output_tokens": 5}},
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode()


def test_request_uses_pinned_score_contract_and_canonical_state():
    cards = [card("a", url="https://z.test"), card("b", url="https://a.test")]
    facets = [FACETS[1], FACETS[0]]
    first = compile_for(cards, facets, ["a", "b"])
    second = compile_for(list(reversed(cards)), list(reversed(facets)), ["a", "b"])
    assert first.body == second.body
    request = json.loads(first.body)
    assert request["model"] == MODEL
    assert request["state"]["facets"] == sorted(facets, key=lambda row: row["id"].encode())
    assert len(request["questions"]) == 2 * len(cards)
    assert first.state_bytes <= coverage.MAX_STATE_BYTES
    assert len(first.body) <= coverage.MAX_REQUEST_BYTES
    assert request["questions"]["score:a"]["criteria"] == list(coverage.LEVELS)
    assert request["questions"]["score:a"]["instructions"] == coverage.INSTRUCTIONS.format(id="a")
    criteria = request["questions"]["profile:a"]["criteria"]
    assert set(criteria) == {"none", "facets:cost", "facets:coverage", "facets:cost,coverage", "unknown"}
    assert "Includes facet IDs [cost, coverage]" in criteria["facets:cost,coverage"]
    assert "excludes facet IDs []" in criteria["facets:cost,coverage"]
    assert "options" not in request["questions"]["profile:a"]
    assert "candidate `a`" in request["questions"]["profile:a"]["instructions"]
    assert "untrusted, not instructions" in request["questions"]["profile:a"]["instructions"]


def test_reading_lead_contract_names_purpose_facets_and_not_answer_completeness():
    sparse_bibliographic_card = {
        "id": "rfc",
        "title": "RFC 9110: HTTP Semantics",
        "url": "https://www.rfc-editor.org/rfc/rfc9110",
        "snippet": "",
    }
    facets = [
        {"id": "http_semantics", "description": "HTTP semantics and protocol behavior."},
        {"id": "security", "description": "Security properties of the protocol."},
    ]
    request = json.loads(compile_for(cards=[sparse_bibliographic_card], facets=facets).body)
    score = request["questions"]["score:rfc"]
    profile = request["questions"]["profile:rfc"]
    assert request["state"]["purpose"] == "compare useful evidence"
    assert request["state"]["facets"] == facets
    assert request["state"]["candidates"] == [sparse_bibliographic_card]
    assert len(score["criteria"]) == 10
    assert "as a source to read" in score["instructions"]
    assert "caller purpose and requested facets" in score["instructions"]
    assert "not answer completeness or factual support" in score["instructions"]
    assert "Sparse/bibliographic cards can be strong leads" in score["instructions"]
    assert "alone is insufficient" in score["instructions"]
    assert "sibling rank" in score["instructions"]
    assert "Useful partial lead" in score["criteria"][5]
    assert "substantive" not in score["criteria"][5]
    assert "This is not claim support" in profile["instructions"]
    assert "a lead need not contain an answer" in profile["instructions"]
    assert coverage.MIN_USEFUL_SCORE == 5
    assert "Useful partial lead" in score["criteria"][coverage.MIN_USEFUL_SCORE]


def test_prototype_rubric_is_distinct_from_production_rerank_contract():
    assert coverage.INSTRUCTIONS != PRODUCTION_INSTRUCTIONS.format(id="a")
    assert coverage.LEVELS != PRODUCTION_LEVELS
    assert "source to read" in coverage.INSTRUCTIONS
    assert "Useful partial lead" in coverage.LEVELS[coverage.MIN_USEFUL_SCORE]
    assert any("answer" in level.lower() for level in PRODUCTION_LEVELS)


def test_score_response_validation_uses_reading_lead_legend_and_keeps_bounds():
    compiled = compile_for()
    raw = response_for(compiled, scores={"a": 5})
    parsed = coverage.parse_response(raw, compiled)
    assert parsed.scores["a"] == 5.0
    assert parsed.score_audit["a"]["legend"] == {str(index): level for index, level in enumerate(coverage.LEVELS)}

    document = json.loads(raw)
    document["answers"]["score:a"]["legend"]["5"] = "old production meaning"
    with pytest.raises(coverage.CoverageError, match="response-score-legend"):
        coverage.parse_response(json.dumps(document).encode(), compiled)

    document = json.loads(raw)
    document["answers"]["score:a"]["score"] = 10
    with pytest.raises(coverage.CoverageError, match="response-score-range"):
        coverage.parse_response(json.dumps(document).encode(), compiled)


def test_missing_score_or_profile_question_is_rejected():
    compiled = compile_for()
    document = json.loads(response_for(compiled))
    del document["answers"]["score:a"]
    with pytest.raises(coverage.CoverageError, match="response-question-inventory"):
        coverage.parse_response(json.dumps(document).encode(), compiled)


def test_partial_facet_reservations_rank_tail_and_audit_unknown_separately_from_none():
    cards = [card("a"), card("b"), card("c"), card("d")]
    compiled = compile_for(cards, FACETS)
    profiles = {"a": "facets:coverage", "b": "none", "c": "unknown", "d": "facets:coverage"}
    result = coverage.select(
        compiled,
        coverage.parse_response(
            response_for(compiled, scores={"a": 6, "b": 9, "c": 8, "d": 5}, profiles=profiles), compiled
        ),
    )
    assert result.status == "selected"
    assert result.reason == "partial_facet_lead_reservation"
    assert result.ordered_ids == ("a", "b", "c", "d")
    assert result.audit["reserved_facets"] == {"coverage": "a"}
    assert result.audit["unreserved_facets"] == [
        {"facet_id": "cost", "reason": "no_eligible_lead"},
        {"facet_id": "risk", "reason": "no_eligible_lead"},
    ]
    assert result.audit["unknown_profile_ids"] == ["c"]
    assert result.audit["none_profile_ids"] == ["b"]


def test_no_eligible_facet_leads_still_score_ranks_and_keeps_canonical_ties():
    cards = [card("u", url="https://b.test"), card("z", url="https://a.test"), card("a")]
    compiled = compile_for(cards, FACETS[:2])
    result = coverage.select(
        compiled,
        coverage.parse_response(
            response_for(
                compiled,
                scores={"u": 9, "z": 9, "a": 7},
                profiles={"u": "none", "z": "unknown", "a": "none"},
            ),
            compiled,
        ),
    )
    assert result.status == "selected"
    assert result.reason == "no_eligible_facet_leads_score_ranked"
    assert result.ordered_ids == ("z", "u", "a")
    assert result.audit["reserved_facets"] == {}
    assert result.audit["unreserved_facets"] == [
        {"facet_id": "cost", "reason": "no_eligible_lead"},
        {"facet_id": "coverage", "reason": "no_eligible_lead"},
    ]
    assert result.audit["unknown_profile_ids"] == ["z"]
    assert result.audit["none_profile_ids"] == ["a", "u"]


def test_existing_useful_lead_cutoff_still_controls_facet_reservation():
    cards = [card("coverage"), card("cost"), card("tail")]
    compiled = compile_for(cards, FACETS[:2])
    result = coverage.select(
        compiled,
        coverage.parse_response(
            response_for(
                compiled,
                scores={"coverage": 4, "cost": 5, "tail": 6},
                profiles={"coverage": "facets:coverage", "cost": "facets:cost", "tail": "none"},
            ),
            compiled,
        ),
    )
    assert result.ordered_ids == ("cost", "tail", "coverage")
    assert result.audit["reserved_facets"] == {"cost": "cost"}
    assert result.audit["unreserved_facets"] == [{"facet_id": "coverage", "reason": "no_eligible_lead"}]


def test_tied_maximum_is_policy_unknown_and_original_probabilities_remain_audit():
    cards = [card("tie"), card("cover"), card("tail")]
    compiled = compile_for(cards, FACETS[:2])
    options = compiled.profile_options["profile:tie"]
    probs = {option: 0.0 for option in options}
    probs["facets:coverage"] = 0.5
    probs["unknown"] = 0.5
    raw = response_for(
        compiled,
        profiles={"tie": "facets:coverage", "cover": "facets:coverage,cost", "tail": "none"},
        probabilities={"tie": probs},
    )
    parsed = coverage.parse_response(raw, compiled)
    assert parsed.selected_facets["tie"] is None
    assert parsed.profile_audit["tie"]["choice"] == "facets:coverage"
    assert parsed.profile_audit["tie"]["probabilities"] == probs
    assert parsed.profile_audit["tie"]["confidence"] == 0.8
    result = coverage.select(compiled, parsed)
    assert result.status == "selected"
    assert result.ordered_ids == ("cover", "tail", "tie")
    assert result.audit["profiles"]["tie"]["tied_maximum"] is True


def test_multifacet_lead_is_reserved_once_and_full_pool_tail_is_preserved():
    cards = [card("low"), card("both"), card("cost"), card("rest")]
    compiled = compile_for(cards, FACETS[:2], ["rest", "cost", "both", "low"])
    raw = response_for(
        compiled,
        scores={"both": 9, "cost": 8, "low": 7},
        profiles={"both": "facets:coverage,cost", "cost": "facets:cost", "low": "facets:coverage", "rest": "none"},
    )
    result = coverage.select(compiled, coverage.parse_response(raw, compiled))
    assert result.ordered_ids == ("both", "cost", "low", "rest")
    assert result.audit["reserved_facets"] == {"coverage": "both", "cost": "both"}
    assert result.audit["unreserved_facets"] == []
    assert len(result.ordered_ids) == len(cards)
    assert set(result.ordered_ids) == {row["id"] for row in cards}


def test_score_then_canonical_tie_break_and_candidate_input_reversal_invariance():
    cards = [card("z", url="https://b.test"), card("a", url="https://a.test"), card("tail")]
    facets = FACETS[:2]
    incumbent = ["tail", "z", "a"]
    compiled = compile_for(cards, facets, incumbent)
    reversed_compiled = compile_for(list(reversed(cards)), facets, incumbent)
    profiles = {"z": "facets:coverage,cost", "a": "facets:coverage,cost", "tail": "none"}
    raw = response_for(compiled, scores={"z": 8, "a": 8}, profiles=profiles)
    reverse_raw = response_for(reversed_compiled, scores={"z": 8, "a": 8}, profiles=profiles)
    assert compiled.body == reversed_compiled.body
    result = coverage.select(compiled, coverage.parse_response(raw, compiled))
    reverse_result = coverage.select(reversed_compiled, coverage.parse_response(reverse_raw, reversed_compiled))
    assert result.ordered_ids == reverse_result.ordered_ids == ("a", "z", "tail")
    assert result.audit["reserved_facets"] == {"coverage": "a", "cost": "a"}


def test_eighty_cards_compile_to_exactly_160_questions_and_keep_full_pool():
    cards = [card(f"c{i:02d}") for i in range(80)]
    incumbent = [row["id"] for row in reversed(cards)]
    compiled = compile_for(cards, FACETS[:2], incumbent)
    assert len(compiled.question_ids) == 160
    profiles = {row["id"]: "facets:coverage,cost" if row["id"] == "c00" else "none" for row in cards}
    raw = response_for(compiled, profiles=profiles)
    result = coverage.select(compiled, coverage.parse_response(raw, compiled))
    assert len(result.ordered_ids) == 80
    assert result.ordered_ids[0] == "c00"
    assert result.ordered_ids[1:] == tuple(f"c{i:02d}" for i in range(1, 80))


@pytest.mark.parametrize(
    "mutation,reason",
    [
        (lambda d: d.update(model="wrong-model"), "response-model-mismatch"),
        (lambda d: d["answers"].pop(next(iter(d["answers"]))), "response-question-inventory"),
        (lambda d: d["answers"]["score:a"].update(score=True), "response-score-range"),
        (lambda d: d["answers"]["score:a"].update(score=float("nan")), "invalid-response-number:NaN"),
        (lambda d: d["usage"].update(input_tokens=True), "response-usage-invalid"),
        (lambda d: d["usage"].update(output_tokens=-1), "response-usage-invalid"),
    ],
)
def test_strict_response_schema_rejects_wrong_model_answers_scores_and_usage(mutation, reason):
    compiled = compile_for([card("a"), card("b")], FACETS[:2])
    data = json.loads(response_for(compiled))
    mutation(data)
    raw = json.dumps(data, allow_nan=True, separators=(",", ":")).encode()
    with pytest.raises(coverage.CoverageError, match=reason):
        coverage.parse_response(raw, compiled)


def test_probability_maps_must_be_exact_finite_sums_and_chosen_maximum():
    compiled = compile_for([card("a"), card("b")], FACETS[:2])
    qid = "profile:a"
    options = compiled.profile_options[qid]
    cases = []
    wrong_inventory = {option: 0.0 for option in options[:-1]}
    cases.append(
        (
            {"type": "choice", "choice": "none", "probabilities": wrong_inventory, "confidence": 0.5},
            "response-probability-inventory",
        )
    )
    bad_sum = {option: 0.0 for option in options}
    bad_sum["none"] = 0.5
    cases.append(
        ({"type": "choice", "choice": "none", "probabilities": bad_sum, "confidence": 0.5}, "response-probability-sum")
    )
    not_max = {option: 0.0 for option in options}
    not_max["unknown"] = 1.0
    cases.append(
        (
            {"type": "choice", "choice": "none", "probabilities": not_max, "confidence": 0.5},
            "response-choice-not-maximum",
        )
    )
    bool_probability = {option: 0.0 for option in options}
    bool_probability["none"] = True
    bool_probability["unknown"] = 0.0
    cases.append(
        (
            {"type": "choice", "choice": "none", "probabilities": bool_probability, "confidence": 0.5},
            "response-probability-range",
        )
    )
    for bad_answer, reason in cases:
        data = json.loads(response_for(compiled))
        data["answers"][qid] = bad_answer
        with pytest.raises(coverage.CoverageError, match=reason):
            coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


@pytest.mark.parametrize(
    "mutate,reason",
    [
        (lambda answer: answer.pop("legend"), "response-score-shape"),
        (lambda answer: answer["legend"].update({"0": "stale"}), "response-score-legend"),
        (lambda answer: answer["probabilities"].pop("9"), "response-score-probability-inventory"),
        (lambda answer: answer["probabilities"].update({"0": 0.5}), "response-score-probability-sum"),
        (lambda answer: answer.update(score=8), "response-score-mean-mismatch"),
        (lambda answer: answer.update(confidence=1.1), "response-score-confidence"),
    ],
)
def test_documented_score_wire_is_fully_validated(mutate, reason):
    compiled = compile_for([card("a"), card("b")], FACETS[:2])
    data = json.loads(response_for(compiled))
    mutate(data["answers"]["score:a"])
    with pytest.raises(coverage.CoverageError, match=reason):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_score_is_probability_weighted_mean_and_confidence_does_not_rank():
    cards = [card("a"), card("b")]
    compiled = compile_for(cards, FACETS[:2])
    data = json.loads(
        response_for(
            compiled, scores={"a": 6, "b": 7}, profiles={"a": "facets:coverage,cost", "b": "facets:coverage,cost"}
        )
    )
    score_answer = data["answers"]["score:a"]
    score_answer["score"] = 6.25
    score_answer["probabilities"] = {str(level): 0.0 for level in range(10)}
    score_answer["probabilities"]["6"] = 0.75
    score_answer["probabilities"]["7"] = 0.25
    score_answer["confidence"] = 0.99
    data["answers"]["score:b"]["confidence"] = 0.01
    compiled_response = json.dumps(data, separators=(",", ":")).encode()
    parsed = coverage.parse_response(compiled_response, compiled)
    assert parsed.scores["a"] == 6.25
    assert parsed.score_audit["a"]["confidence"] == 0.99
    result = coverage.select(compiled, parsed)
    assert result.ordered_ids == ("b", "a")


def test_score_mean_audit_distinguishes_exact_and_tolerance_compatible_values():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    data["answers"]["score:a"]["score"] = 7.0000005
    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.score_audit["a"]["mean_validation"] == "within-tolerance"
    assert parsed.score_audit["b"]["mean_validation"] == "exact"


def test_constructed_hundredth_precision_tuples_keep_reported_scores():
    cards = [card("generated-low"), card("generated-high")]
    compiled = compile_for(cards, FACETS)
    data = json.loads(response_for(compiled))
    # Independently constructed symmetric distributions both have mean 4.5.
    constructed_tuples = {
        "generated-low": (4.45, [0.1] * 10),
        "generated-high": (4.56, [0.05, 0.05, 0.1, 0.1, 0.2, 0.2, 0.1, 0.1, 0.05, 0.05]),
    }
    for cid, (reported, values) in constructed_tuples.items():
        answer = data["answers"][f"score:{cid}"]
        answer["score"] = reported
        answer["probabilities"] = {str(index): probability for index, probability in enumerate(values)}
    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.scores == {"generated-low": 4.45, "generated-high": 4.56}
    for cid in constructed_tuples:
        assert parsed.score_audit[cid]["mean_validation"] == "hundredth-rounding-interval"


def test_score_rounding_accepts_only_one_float_neighbor_and_preserves_wire_values():
    compiled = compile_for([card("a"), card("b")], FACETS)
    probabilities = [0.05, 0.05, 0.1, 0.1, 0.2, 0.2, 0.1, 0.1, 0.05, 0.05]
    data = json.loads(response_for(compiled))
    answer = data["answers"]["score:a"]
    below = math.nextafter(4.56, -math.inf)
    above = math.nextafter(4.56, math.inf)
    answer["score"] = above
    probabilities[0] = math.nextafter(probabilities[0], math.inf)
    probabilities[9] = math.nextafter(probabilities[9], -math.inf)
    answer["probabilities"] = {str(index): value for index, value in enumerate(probabilities)}

    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.scores["a"] == above
    assert parsed.score_audit["a"]["probabilities"] == answer["probabilities"]
    assert parsed.score_audit["a"]["mean_validation"] == "hundredth-rounding-interval"

    answer["score"] = below
    parsed_below = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed_below.scores["a"] == below
    assert parsed_below.score_audit["a"]["mean_validation"] == "hundredth-rounding-interval"

    second_above = math.nextafter(above, math.inf)
    answer["score"] = second_above
    with pytest.raises(coverage.CoverageError, match="response-score-mean-mismatch"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_probability_rounding_rejects_second_float_neighbor():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["score:a"]
    answer["score"] = 4.56
    probabilities = [0.05, 0.05, 0.1, 0.1, 0.2, 0.2, 0.1, 0.1, 0.05, 0.05]
    first_above = math.nextafter(probabilities[0], math.inf)
    probabilities[0] = math.nextafter(first_above, math.inf)
    answer["probabilities"] = {str(index): value for index, value in enumerate(probabilities)}
    with pytest.raises(coverage.CoverageError, match="response-score-mean-mismatch"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


@pytest.mark.parametrize("mass_delta", [-0.01, 0.01])
def test_score_rounding_interval_can_reconcile_displayed_mass(mass_delta):
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["score:a"]
    answer["score"] = 4.56
    probabilities = [0.05, 0.05, 0.1, 0.1, 0.2, 0.2, 0.1, 0.1, 0.05, 0.05]
    probabilities[0] += mass_delta
    answer["probabilities"] = {str(index): value for index, value in enumerate(probabilities)}

    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.scores["a"] == 4.56
    assert parsed.score_audit["a"]["probabilities"] == answer["probabilities"]
    assert parsed.score_audit["a"]["mean_validation"] == "hundredth-rounding-interval"


def test_score_rounding_interval_still_rejects_infeasible_mass_and_noncent_mass():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["score:a"]
    answer["score"] = 4.56
    answer["probabilities"] = {str(index): 0.11 for index in range(10)}
    with pytest.raises(coverage.CoverageError, match="response-score-probability-sum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)

    answer["probabilities"] = {str(index): 0.1 for index in range(10)}
    answer["probabilities"]["0"] = 0.111
    with pytest.raises(coverage.CoverageError, match="response-score-probability-sum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


@pytest.mark.parametrize("mass_delta", [-0.01, 0.01])
def test_choice_rounding_interval_can_reconcile_displayed_mass(mass_delta):
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["profile:a"]
    options = compiled.profile_options["profile:a"]
    answer["choice"] = "none"
    probabilities = {option: 0.0 for option in options}
    probabilities["none"] = 0.6
    probabilities["unknown"] = 0.4 + mass_delta
    answer["probabilities"] = probabilities

    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.selected_facets["a"] == ()
    assert parsed.profile_audit["a"]["choice"] == "none"
    assert parsed.profile_audit["a"]["probabilities"] == probabilities


def test_choice_rounding_interval_still_rejects_infeasible_and_noncent_mass():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["profile:a"]
    options = compiled.profile_options["profile:a"]
    answer["choice"] = options[0]
    answer["probabilities"] = {option: 0.12 for option in options}
    with pytest.raises(coverage.CoverageError, match="response-probability-sum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)

    probabilities = {option: 0.0 for option in options}
    probabilities[options[0]] = 0.6
    probabilities[options[1]] = 0.401
    answer["probabilities"] = probabilities
    with pytest.raises(coverage.CoverageError, match="response-probability-sum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_rounding_interval_mass_constraints_and_endpoint_clipping():
    at_zero = {str(index): float(index == 0) for index in range(10)}
    at_nine = {str(index): float(index == 9) for index in range(10)}
    uniform = {str(index): 0.1 for index in range(10)}
    no_mass = {str(index): 0.0 for index in range(10)}
    assert coverage._rounded_score_mean_is_feasible(0.0, at_zero)
    assert coverage._rounded_score_mean_is_feasible(9.0, at_nine)
    assert coverage._rounded_score_mean_is_feasible(4.5, uniform)
    assert not coverage._rounded_score_mean_is_feasible(0.0, no_mass)


def test_score_rounding_feasibility_still_rejects_infeasible_or_non_grid_values():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["score:a"]
    answer["score"] = 2.0
    answer["probabilities"] = {str(index): 0.1 for index in range(10)}
    with pytest.raises(coverage.CoverageError, match="response-score-mean-mismatch"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)

    answer["score"] = 4.45
    answer["probabilities"]["0"] = 0.101
    answer["probabilities"]["1"] = 0.099
    with pytest.raises(coverage.CoverageError, match="response-score-mean-mismatch"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_choice_one_ulp_wire_tie_is_unknown_but_nearby_non_ties_remain_invalid():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["profile:a"]
    maximum = 0.30
    answer["choice"] = "facets:coverage"
    answer["probabilities"] = {option: 0.0 for option in compiled.profile_options["profile:a"]}
    answer["probabilities"]["facets:coverage"] = math.nextafter(maximum, 0.0)
    answer["probabilities"]["none"] = maximum
    remainder_options = [option for option in answer["probabilities"] if option not in {"facets:coverage", "none"}]
    for option in remainder_options[:4]:
        answer["probabilities"][option] = 0.10
    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.selected_facets["a"] is None
    assert parsed.profile_audit["a"]["tied_maximum"] is True

    answer["probabilities"]["facets:coverage"] = 0.29
    for option in remainder_options[:4]:
        answer["probabilities"][option] = 0.1025
    with pytest.raises(coverage.CoverageError, match="response-choice-not-maximum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_choice_power_of_two_uses_exactly_one_downward_float_neighbor():
    compiled = compile_for([card("a"), card("b")], FACETS)
    data = json.loads(response_for(compiled))
    answer = data["answers"]["profile:a"]
    answer["choice"] = "facets:coverage"
    probabilities = {option: 0.0 for option in compiled.profile_options["profile:a"]}
    maximum = 0.5
    first_predecessor = math.nextafter(maximum, 0.0)
    probabilities["facets:coverage"] = first_predecessor
    probabilities["none"] = maximum
    probabilities["unknown"] = 1.0 - maximum - first_predecessor
    answer["probabilities"] = probabilities
    parsed = coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)
    assert parsed.selected_facets["a"] is None
    assert parsed.profile_audit["a"]["tied_maximum"] is True

    second_predecessor = math.nextafter(first_predecessor, 0.0)
    probabilities["facets:coverage"] = second_predecessor
    probabilities["unknown"] = 1.0 - maximum - second_predecessor
    with pytest.raises(coverage.CoverageError, match="response-choice-not-maximum"):
        coverage.parse_response(json.dumps(data, separators=(",", ":")).encode(), compiled)


def test_choice_confidence_is_required_and_bounded_but_not_used_for_policy():
    compiled = compile_for([card("a"), card("b")], FACETS[:2])
    for replacement, reason in (({"type": "choice", "choice": "none", "probabilities": {}}, "response-choice-shape"),):
        data = json.loads(response_for(compiled))
        data["answers"]["profile:a"] = replacement
        with pytest.raises(coverage.CoverageError, match=reason):
            coverage.parse_response(json.dumps(data).encode(), compiled)
    data = json.loads(response_for(compiled))
    data["answers"]["profile:a"]["confidence"] = -0.1
    with pytest.raises(coverage.CoverageError, match="response-choice-confidence"):
        coverage.parse_response(json.dumps(data).encode(), compiled)


def test_duplicate_keys_nonfinite_overbound_and_input_errors_fallback_safely():
    cards = [card("a"), card("b")]
    incumbent = ["a", "b"]
    valid = compile_for(cards, FACETS[:2], incumbent)
    for raw in (
        b"{}",
        b'{"model":"x","model":"y"}',
        b'{"model":"x","answers":NaN,"usage":{}}',
        b"x" * (coverage.MAX_RESPONSE_BYTES + 1),
    ):
        result = coverage.rank_offline(
            query="public query",
            purpose="compare useful evidence",
            facets=FACETS[:2],
            candidates=cards,
            incumbent_order=incumbent,
            raw_response=raw,
        )
        assert result.status == "fallback" and result.reason == "invalid_response"
        assert result.ordered_ids == tuple(incumbent)
    assert (
        coverage.rank_offline(
            query="", purpose="p", facets=FACETS[:2], candidates=cards, incumbent_order=incumbent, raw_response=b""
        ).reason
        == "invalid_input"
    )
    assert (
        coverage.rank_offline(
            query="q",
            purpose="p",
            facets=FACETS[:2],
            candidates=[cards[0], cards[0]],
            incumbent_order=incumbent,
            raw_response=b"",
        ).reason
        == "invalid_input"
    )
    with pytest.raises(coverage.CoverageError, match="incumbent-fallback-invalid"):
        coverage.rank_offline(
            query="q", purpose="p", facets=FACETS[:2], candidates=cards, incumbent_order=["a", "a"], raw_response=b""
        )
    assert len(valid.question_ids) == 4


@pytest.mark.parametrize(
    "change,reason",
    [
        ({"title": "x" * (coverage.MAX_TITLE_BYTES + 1)}, "candidate-title-too-large"),
        ({"url": "x" * (coverage.MAX_URL_BYTES + 1)}, "candidate-url-too-large"),
        ({"snippet": "x" * (coverage.MAX_SNIPPET_BYTES + 1)}, "candidate-snippet-too-large"),
    ],
)
def test_oversized_card_fields_are_rejected_not_trimmed(change, reason):
    cards = [card("a"), card("b")]
    cards[0].update(change)
    with pytest.raises(coverage.CoverageError, match=reason):
        compile_for(cards, FACETS[:2])


def test_query_purpose_facet_bounds_and_context_inventory_are_enforced():
    cards = [card("a"), card("b")]
    with pytest.raises(coverage.CoverageError, match="query-too-large"):
        coverage.compile_request(
            query="q" * (coverage.MAX_QUERY_BYTES + 1),
            purpose="p",
            facets=FACETS[:2],
            candidates=cards,
            incumbent_order=["a", "b"],
        )
    with pytest.raises(coverage.CoverageError, match="purpose-too-large"):
        coverage.compile_request(
            query="q",
            purpose="p" * (coverage.MAX_PURPOSE_BYTES + 1),
            facets=FACETS[:2],
            candidates=cards,
            incumbent_order=["a", "b"],
        )
    with pytest.raises(coverage.CoverageError, match="facet-description-too-large"):
        compile_for(cards, [{"id": "x", "description": "x" * 513}, FACETS[1]])
    with pytest.raises(coverage.CoverageError, match="duplicate-facet-id"):
        compile_for(cards, [FACETS[0], FACETS[0]])
    with pytest.raises(coverage.CoverageError, match="facet-id-invalid"):
        compile_for(cards, [{"id": "bad,facet", "description": "x"}, FACETS[1]])
    with pytest.raises(coverage.CoverageError, match="incumbent-order-not-permutation"):
        compile_for(cards, FACETS[:2], ["a", "missing"])


def test_input_and_response_bounds_fallback_without_trimming():
    cards = [card("a"), card("b")]
    incumbent = ["a", "b"]
    result = coverage.rank_offline(
        query="public query",
        purpose="compare useful evidence",
        facets=FACETS[:2],
        candidates=cards,
        incumbent_order=incumbent,
        raw_response=b"x" * (coverage.MAX_RESPONSE_BYTES + 1),
    )
    assert result.reason == "invalid_response" and result.ordered_ids == tuple(incumbent)
    huge = [card(f"c{i:02d}") for i in range(80)]
    for row in huge:
        row["title"] = "t" * coverage.MAX_TITLE_BYTES
        row["url"] = "u" * coverage.MAX_URL_BYTES
        row["snippet"] = "s" * coverage.MAX_SNIPPET_BYTES
    with pytest.raises(coverage.CoverageError, match="state-too-large"):
        compile_for(huge, FACETS, [row["id"] for row in huge])


@pytest.mark.parametrize("raw", [b"[" * 2000 + b"]" * 2000, b'{"usage":' + b"9" * 5000 + b"}"])
def test_extreme_json_shapes_fail_to_incumbent_without_parser_exception(raw):
    result = coverage.rank_offline(
        query="public query",
        purpose="compare useful evidence",
        facets=FACETS[:2],
        candidates=[card("a"), card("b")],
        incumbent_order=["b", "a"],
        raw_response=raw,
    )
    assert result.status == "fallback"
    assert result.reason == "invalid_response"
    assert result.ordered_ids == ("b", "a")


def test_duplicate_or_blank_research_needs_are_rejected():
    duplicate = [
        {"id": "first", "description": "same requirement"},
        {"id": "second", "description": " same requirement "},
    ]
    with pytest.raises(coverage.CoverageError, match="duplicate-facet-description"):
        compile_for(facets=duplicate)
    blank = [{"id": "first", "description": "   "}, FACETS[1]]
    with pytest.raises(coverage.CoverageError):
        compile_for(facets=blank)


def test_eighty_cards_with_three_needs_use_documented_nine_profile_choices():
    cards = [card(f"c{i:02d}") for i in range(80)]
    compiled = compile_for(cards=cards, facets=FACETS)
    request = json.loads(compiled.body)
    assert len(request["questions"]) == 160
    assert len(request["questions"]["profile:c00"]["criteria"]) == 9
    assert len(compiled.body) <= coverage.MAX_REQUEST_BYTES
