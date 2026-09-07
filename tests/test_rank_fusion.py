"""Rank fusion invariants and small, explicitly synthetic relevance judgments."""

import copy
import math

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.merger import ReciprocalRankFusionRanker, create_ranker


def result(url: str, tier: int = 1) -> SearchResult:
    return SearchResult(url=url, title=url, content="", engine="source", tier=tier)


def test_fusion_preserves_inputs_tiers_and_single_engine_contribution() -> None:
    feeds = {
        "b": [result("https://shared?utm_source=b", 2), result("https://special", 2)],
        "a": [result("https://broad", 1), result("https://shared", 1), result("https://shared", 1)],
    }
    original = copy.deepcopy(feeds)
    ranked = ReciprocalRankFusionRanker().rank(feeds, "")
    assert feeds == original
    assert [r.url for r in ranked] == ["https://shared", "https://broad", "https://special"]
    assert ranked[0].score == pytest.approx(1 / 61 + 1 / 62)
    assert ranked[0].engines == {"a", "b"}
    assert ranked[0].tier == 1
    assert [r.position for r in ranked] == [1, 2, 3]
    assert ReciprocalRankFusionRanker().rank(dict(reversed(list(feeds.items()))), "") == ranked


def ndcg(ranked: list[SearchResult], judgments: dict[str, int], k: int = 3) -> float:
    def dcg(grades: list[int]) -> float:
        return sum((2**grade - 1) / math.log2(position + 2) for position, grade in enumerate(grades[:k]))

    return dcg([judgments[r.url] for r in ranked]) / dcg(sorted(judgments.values(), reverse=True))


# Author-assigned grades: 3 directly answers intent; 1 related; 0 irrelevant.
# Deliberately constructed feeds, NOT captured engine output or human study.
CASES = [
    (
        "general: why does the sky look blue?",
        {"a": ["shop", "weather", "rayleigh"], "b": ["rayleigh", "weather", "shop"]},
        {"rayleigh": 3, "weather": 1, "shop": 0},
    ),
    (
        "code: Python asyncio TaskGroup cancellation semantics",
        {"a": ["tutorial", "docs", "release"], "b": ["docs", "release", "tutorial"]},
        {"docs": 3, "tutorial": 1, "release": 0},
    ),
    (
        "science: randomized trial evidence for intervention X",
        {"a": ["trial", "review", "advert"], "b": ["advert", "review", "trial"]},
        {"trial": 3, "review": 1, "advert": 0},
    ),
]


@pytest.mark.parametrize("query,feeds,judgments", CASES)
def test_judged_relevance_evaluation(query: str, feeds: dict[str, list[str]], judgments: dict[str, int]) -> None:
    scores = {}
    for strategy in ("presence", "reciprocal_rank_fusion"):
        ranked = create_ranker(strategy).rank(
            {name: [result(url) for url in urls] for name, urls in feeds.items()}, query
        )
        scores[strategy] = ndcg(ranked, judgments)
    print(f"{query}: {scores}")
    # Pin the evidence, including a regression, rather than assert universal gain.
    expected = {
        "general": (0.5413402936435214, 0.9828422279067397),
        "code": (0.7098097413968655, 1.0),
        "science": (1.0, 0.6442869262030828),
    }
    baseline, fusion = expected[query.split(":")[0]]
    assert scores["presence"] == pytest.approx(baseline)
    assert scores["reciprocal_rank_fusion"] == pytest.approx(fusion)


async def test_service_strategy_and_cache_identity() -> None:
    from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus
    from slopsearx.service import AppContext, SearchRequest, SearchService, _routing_cache_digest

    class Feed(EngineAdapter):
        name = "feed"

        async def search(self, query: str, params: dict[str, object] | None = None) -> AdapterResponse:
            return AdapterResponse(status=EngineStatus.OK, results=[result("https://z"), result("https://a")])

    context = AppContext(active_engines={"feed": Feed({})}, ranking_strategy="reciprocal_rank_fusion")
    fusion_digest = _routing_cache_digest(context)
    response = await SearchService(context).search(SearchRequest(query="query", engines=["feed"]))
    assert response.results[0].url == "https://z"
    assert response.results[0].score == pytest.approx(1 / 61)
    context.ranking_strategy = "presence"
    assert _routing_cache_digest(context) != fusion_digest
