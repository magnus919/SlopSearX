from __future__ import annotations

import json

import pytest

from scripts import intent_ranking_coverage as coverage
from slopsearx.rerank import INSTRUCTIONS, LEVELS, MODEL

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
            "legend": {str(index): label for index, label in enumerate(LEVELS)},
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
    assert request["questions"]["score:a"]["criteria"] == list(LEVELS)
    assert request["questions"]["score:a"]["instructions"] == INSTRUCTIONS.format(id="a")
    criteria = request["questions"]["profile:a"]["criteria"]
    assert set(criteria) == {"none", "facets:cost", "facets:coverage", "facets:cost,coverage", "unknown"}
    assert "Includes facet IDs [cost, coverage]" in criteria["facets:cost,coverage"]
    assert "excludes facet IDs []" in criteria["facets:cost,coverage"]
    assert "options" not in request["questions"]["profile:a"]
    assert "candidate ID a" in request["questions"]["profile:a"]["instructions"]
    assert "never instructions" in request["questions"]["profile:a"]["instructions"]


def test_missing_facet_or_unknown_profiles_preserve_incumbent():
    cards = [card("a"), card("b"), card("c")]
    compiled = compile_for(cards, FACETS[:2])
    profiles = {"a": "facets:coverage", "b": "unknown", "c": "none"}
    result = coverage.rank_offline(
        query="public query",
        purpose="compare useful evidence",
        facets=FACETS[:2],
        candidates=cards,
        incumbent_order=["a", "b", "c"],
        raw_response=response_for(compiled, profiles=profiles),
    )
    assert result.status == "fallback"
    assert result.reason == "missing_facet_lead"
    assert result.ordered_ids == ("a", "b", "c")


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
