"""Publication grouping uses source identity, never model judgments or titles."""

from copy import deepcopy

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.formatter import _result_to_searxng
from slopsearx.merger import PresenceRanker, ReciprocalRankFusionRanker
from slopsearx.payload import build_payload
from slopsearx.scholarly import group_publications
from slopsearx.service import search_result_from_dict, search_result_to_dict


def paper(url, engine="a", **data):
    return SearchResult(
        url=url,
        title="Identical title",
        content="Public fixture",
        engine=engine,
        payload=build_payload("science", "publication", data, engine=engine),
    )


def ranked(feeds, query="", ranker=None):
    return (ranker or PresenceRanker()).rank(group_publications(feeds, query), query)


@pytest.mark.parametrize("ranker", [PresenceRanker(), ReciprocalRankFusionRanker()])
def test_same_doi_preserves_all_members_and_engine_votes_once(ranker):
    feeds = {
        "a": [paper("https://a.test/1", doi="10.1234/work"), paper("https://a.test/2", doi="10.1234/work")],
        "b": [paper("https://b.test/1", "b", doi="https://doi.org/10.1234/WORK", authors=["A. Author"])],
    }
    original = deepcopy(feeds)
    (result,) = ranked(feeds, ranker=ranker)
    assert result.engines == {"a", "b"}
    assert [(m["engine"], m["original_position"]) for m in result.work_group["members"]] == [
        ("a", 1),
        ("a", 2),
        ("b", 1),
    ]
    assert feeds == original
    wire = _result_to_searxng(result)
    assert wire["doi"] == "10.1234/work"
    assert wire["authors"] == ["A. Author"]
    assert isinstance(wire["url"], str) and isinstance(wire["engine"], str)
    assert wire["engines"] == ["a", "b"]
    assert not {"work_group", "alternate_urls", "alternate_dois"} & wire.keys()
    if isinstance(ranker, ReciprocalRankFusionRanker):
        assert result.score == pytest.approx(2 / 61)
    restored = search_result_from_dict(search_result_to_dict(result))
    assert restored.work_group == result.work_group


def test_conflicting_dois_and_equal_titles_remain_distinct():
    rows = [paper("https://a.test/1", doi="10.1234/first"), paper("https://a.test/2", doi="10.1234/second")]
    assert len(ranked({"a": rows})) == 2


@pytest.mark.parametrize("relation", ["cites", "isCorrectionOf", "isRetractionOf", "isSupplementTo"])
def test_notices_and_citations_do_not_establish_identity(relation):
    rows = [
        paper(
            "https://a.test/1", doi="10.1234/first", relations=[{"relation_type": relation, "doi": "10.1234/second"}]
        ),
        paper("https://a.test/2", doi="10.1234/second"),
    ]
    assert len(ranked({"a": rows})) == 2


@pytest.mark.parametrize("relation", ["isVersionOf", "isPreprintOf", "hasVersion"])
def test_generic_version_relation_groups_without_inventing_recency(relation):
    rows = [
        paper(
            "https://a.test/1", doi="10.1234/first", relations=[{"relation_type": relation, "doi": "10.1234/second"}]
        ),
        paper("https://a.test/2", doi="10.1234/second", indexed="2099-01-01"),
    ]
    (result,) = ranked({"a": rows})
    assert result.url == rows[0].url
    assert result.work_group["selection"] == "stable_source_order"


def test_explicit_revision_chain_selects_newest_and_retains_warning():
    rows = [
        paper("https://a.test/1", doi="10.1234/one", publication_types=["Retracted Publication"]),
        paper(
            "https://a.test/2", doi="10.1234/two", relations=[{"relation_type": "isNewVersionOf", "doi": "10.1234/one"}]
        ),
        paper(
            "https://a.test/3",
            doi="10.1234/three",
            relations=[{"relation_type": "isNewVersionOf", "doi": "10.1234/two"}],
        ),
    ]
    (result,) = ranked({"a": rows})
    assert result.url == rows[2].url
    assert "Retracted Publication" in _result_to_searxng(result)["comments"]
    assert len(result.work_group["members"]) == 3


def test_arxiv_version_numbers_and_explicit_old_version_request():
    rows = [
        paper("https://arxiv.org/abs/2401.00001v1"),
        paper("https://arxiv.org/abs/2401.00001v3"),
        paper("https://arxiv.org/pdf/2401.00001v2.pdf"),
    ]
    (result,) = ranked({"a": rows})
    assert result.url == rows[1].url
    (old,) = ranked({"a": rows}, "Read arxiv 2401.00001v1")
    assert old.url == rows[0].url
    assert old.work_group["selection"] == "requested_revision"


def test_grouping_happens_before_forty_candidate_shortlist():
    rows = [paper(f"https://a.test/copy{i}", doi="10.1234/same") for i in range(41)]
    rows += [paper(f"https://a.test/work{i}", doi=f"10.1234/distinct{i}") for i in range(45)]
    results = ranked({"a": rows})
    assert len(results) == 46
    assert len({r.work_group["paper"]["doi"] for r in results[:40]}) == 40


def test_bounded_provenance_declines_merge_instead_of_dropping_member():
    rows = [paper(f"https://a.test/{i}", doi="10.1234/same", abstract="x" * 35000) for i in range(2)]
    assert len(ranked({"a": rows})) == 2


def test_cached_paper_cannot_overwrite_required_wire_fields():
    result = paper("https://a.test/work")
    result.work_group = {"paper": {"url": ["bad"], "engine": "bad", "engines": "bad", "doi": ["bad"], "authors": "bad"}}
    wire = _result_to_searxng(result)
    assert wire["url"] == result.url and wire["engine"] == "a"
    assert "doi" not in wire and "authors" not in wire


def test_unversioned_arxiv_is_same_work_without_inventing_revision_order():
    rows = [paper("https://arxiv.org/abs/2401.00001"), paper("https://arxiv.org/abs/2401.00001v3")]
    (result,) = ranked({"a": rows})
    assert result.url == rows[0].url
    assert result.work_group["selection"] == "stable_source_order"


def test_original_tier_survives_newer_specialist_representative():
    old = paper("https://arxiv.org/abs/2401.00001v1", "brave")
    old.tier = 1
    new = paper("https://arxiv.org/abs/2401.00001v2", "arxiv")
    new.tier = 2
    (result,) = ranked({"brave": [old], "arxiv": [new]})
    assert result.url == new.url and result.tier == 1


def test_conflicting_known_pubmed_ids_do_not_merge_on_shared_doi():
    rows = [
        paper("https://a.test/1", doi="10.1234/shared", pmid="1"),
        paper("https://a.test/2", doi="10.1234/shared", pmid="2"),
    ]
    assert len(ranked({"a": rows})) == 2


@pytest.mark.parametrize(
    "url,target,data",
    [
        ("https://pubmed.ncbi.nlm.nih.gov/1/", "https://pubmed.ncbi.nlm.nih.gov/2/", {"pmid": "2"}),
        (
            "https://pmc.ncbi.nlm.nih.gov/articles/PMC1/",
            "https://pmc.ncbi.nlm.nih.gov/articles/PMC2/",
            {"pmcid": "PMC2"},
        ),
        ("https://arxiv.org/abs/2401.00001v1", "https://arxiv.org/abs/2402.00001v1", {"arxiv_id": "2402.00001v1"}),
    ],
)
def test_conflicting_payload_identifier_cannot_override_url_identity(url, target, data):
    results = ranked({"a": [paper(url, **data), paper(target, **data)]})
    assert len(results) == 2
    assert all(len(result.work_group["members"]) == 1 for result in results)


def test_scholarly_duplicates_do_not_consume_fusion_positions_or_engine_budget():
    rows = [
        paper("https://a.test/first", doi="10.1234/shared"),
        paper("https://a.test/copy", doi="10.1234/shared"),
        paper("https://a.test/distinct", doi="10.1234/distinct"),
    ]
    results = ranked({"a": rows}, ranker=ReciprocalRankFusionRanker())
    assert results[1].score == pytest.approx(1 / 62)
    assert [member["original_position"] for member in results[0].work_group["members"]] == [1, 2]
    assert results[1].work_group["members"][0]["original_position"] == 3
    budgeted = ranked({"a": rows}, ranker=PresenceRanker(per_engine_budget={"a": 2}))
    assert len(budgeted) == 2


def test_ordinary_url_duplicates_retain_existing_rrf_feed_positions():
    rows = [
        SearchResult(url=url, title="Ordinary web", content="", engine="a")
        for url in ["https://a.test/first", "https://a.test/first", "https://a.test/distinct"]
    ]
    results = ranked({"a": rows}, ranker=ReciprocalRankFusionRanker())
    assert results[1].score == pytest.approx(1 / 63)


@pytest.mark.parametrize("identifier", ["s41586-020-2649-2", "s41586-021-03819-2"])
def test_recognized_nature_article_numbers_match_reported_doi(identifier):
    rows = [
        SearchResult(
            url="https://www.nature.com/articles/" + identifier, title="Public paper", content="", engine="brave"
        ),
        paper("https://doi.org/10.1038/" + identifier, "openalex", doi="10.1038/" + identifier),
    ]
    (result,) = ranked({"brave": [rows[0]], "openalex": [rows[1]]})
    assert result.engines == {"brave", "openalex"}
    assert _result_to_searxng(result)["doi"] == "10.1038/" + identifier


def test_oversized_arxiv_version_is_not_converted_to_an_integer():
    url = "https://arxiv.org/abs/2401.00001v" + "9" * 5000
    rows = [SearchResult(url=url, title="Public adversarial identifier fixture", content="", engine="brave")]
    (result,) = ranked({"brave": rows})
    assert result.url == url
    assert result.work_group is None


@pytest.mark.parametrize("malformed", [None, False, {}, "Retracted Publication"])
def test_malformed_publication_status_metadata_does_not_crash_grouping(malformed):
    (result,) = ranked({"a": [paper("https://a.test/work", doi="10.1234/work", publication_types=malformed)]})
    assert result.work_group["warnings"] == []
    assert "comments" not in _result_to_searxng(result)
