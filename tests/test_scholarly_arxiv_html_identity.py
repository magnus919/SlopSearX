"""Frozen offline matrix for arXiv's official HTML publication representation."""

from copy import deepcopy

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.formatter import _result_to_searxng
from slopsearx.merger import PresenceRanker, ReciprocalRankFusionRanker
from slopsearx.payload import build_payload
from slopsearx.scholarly import POLICY_VERSION, _url_ids, group_publications
from slopsearx.service import search_result_from_dict, search_result_to_dict


def publication(url, engine="arxiv", **data):
    return SearchResult(
        url=url,
        title="Identical title",
        content="Offline public fixture",
        engine=engine,
        payload=build_payload("science", "publication", data, engine=engine),
    )


def plain_card(url, title, snippet, engine):
    """Saved-card projection: identity derives from the URL, not fabricated payload."""
    return SearchResult(url=url, title=title, content=snippet, engine=engine)


@pytest.mark.parametrize(
    "html_url,other_url,identifier",
    [
        (
            "https://arxiv.org/html/2501.09136",
            "https://arxiv.org/abs/2501.09136",
            "2501.09136",
        ),
        (
            "https://www.arxiv.org/html/2501.09136v2",
            "https://export.arxiv.org/abs/2501.09136v2",
            "2501.09136v2",
        ),
        (
            "https://export.arxiv.org/html/hep-th/9901001v3",
            "https://arxiv.org/abs/hep-th/9901001v3",
            "hep-th/9901001v3",
        ),
        (
            "https://arxiv.org/html/2501.09136v2",
            "https://arxiv.org/pdf/2501.09136v2.pdf",
            "2501.09136v2",
        ),
    ],
)
@pytest.mark.parametrize("ranker", [PresenceRanker(), ReciprocalRankFusionRanker()])
def test_html_and_abstract_or_pdf_group_preserving_members(html_url, other_url, identifier, ranker):
    html = publication(html_url, engine="web")
    other = publication(other_url, engine="arxiv")
    feeds = {"web": [html], "arxiv": [other]}
    original = deepcopy(feeds)

    grouped = group_publications(feeds, query="")
    result = ranker.rank(grouped, "")

    assert len(result) == 1
    assert result[0].engines == {"web", "arxiv"}
    assert len(result[0].work_group["members"]) == 2
    assert {member["url"] for member in result[0].work_group["members"]} == {html_url, other_url}
    assert {member["identifiers"]["arxiv"] for member in result[0].work_group["members"]} == {identifier}
    assert feeds == original


@pytest.mark.parametrize(
    "url",
    [
        "https://arxiv.org/html/2501.09136.pdf",
        "https://arxiv.org/html/2501.09136/extra",
        "https://arxiv.org/html/2501.09136v0",
        "https://arxiv.org/html/2501.09136v9999999",
        "https://arxiv.org.evil.test/html/2501.09136",
        "https://evil.test/arxiv.org/html/2501.09136",
    ],
)
def test_malformed_extra_suffix_and_deceptive_hosts_are_not_html_identifiers(url):
    assert "arxiv" not in _url_ids(url)


@pytest.mark.parametrize("ranker", [PresenceRanker(), ReciprocalRankFusionRanker()])
def test_saved_2501_09136_abstract_html_pair_is_one_work_with_complete_provenance(ranker):
    # Exact visible source cards from the frozen EXP-040 research pool. The
    # minimal URL projection intentionally carries no synthetic source metadata.
    abstract = plain_card(
        "https://arxiv.org/abs/2501.09136",
        "[2501.09136] Agentic Retrieval-Augmented Generation: A Survey on Agentic RAG",
        (
            "Finally, it identifies key open research challenges related to evaluation, coordination, "
            "memory management, "
            "efficiency, and governance, outlining directions for future research. From: Abul Ehtesham [view email] "
            "[v1] Wed, 15 Jan 2025 20:40:25 UTC (20,962 KB) [v2] Mon, 3 Feb 2025 04:01:36 UTC (22,453 KB) "
            "[v3] Tue, 4 Feb 2025 04:48:00 UTC (22,430 KB) [v4] Wed, 1 Apr 2026 15:51:06 UTC (13,996 KB) "
            "... View a PDF of the paper titled Agentic Retrieval-Augmented Generation: A Survey on Agentic RAG, "
            "by Aditi Singh and 4 other authors"
        ),
        "arxiv",
    )
    html = plain_card(
        "https://arxiv.org/html/2501.09136v4",
        "Agentic Retrieval-Augmented Generation: A Survey on Agentic RAG",
        (
            "The convergence of RAG and agentic intelligence has given rise to Agentic Retrieval-Augmented Generation "
            "(Agentic RAG) [15], which <strong>integrates autonomous agents directly into the RAG pipeline to enable "
            "dynamic retrieval, iterative context refinement, and adaptive workflow "
            "orchestration</strong> [16]. This ..."
        ),
        "arxiv",
    )
    duplicate_same_engine = plain_card(abstract.url, abstract.title, abstract.content, "arxiv")
    cross_engine_copy = plain_card(abstract.url, abstract.title, abstract.content, "web")
    feeds = {"arxiv": [abstract, html, duplicate_same_engine], "web": [cross_engine_copy]}
    original = deepcopy(feeds)

    grouped = group_publications(feeds, "retrieval augmented generation research agents")
    results = ranker.rank(grouped, "retrieval augmented generation research agents")

    assert len(results) == 1
    result = results[0]
    assert result.engines == {"arxiv", "web"}
    if isinstance(ranker, ReciprocalRankFusionRanker):
        assert result.score == pytest.approx(2 / 61)
    else:
        assert result.score == 2
    assert [member["url"] for member in result.work_group["members"]] == [
        abstract.url,
        html.url,
        duplicate_same_engine.url,
        cross_engine_copy.url,
    ]
    assert [member["title"] for member in result.work_group["members"]] == [
        abstract.title,
        html.title,
        duplicate_same_engine.title,
        cross_engine_copy.title,
    ]
    assert feeds == original
    round_trip = search_result_from_dict(search_result_to_dict(result))
    assert round_trip.work_group == result.work_group
    wire = _result_to_searxng(result)
    assert wire["url"] == result.url
    assert "work_group" not in wire


def test_html_payload_identifier_conflict_does_not_merge_distinct_versions():
    conflict = publication("https://arxiv.org/html/2501.09136v2", arxiv_id="2501.09136v3")
    matching = publication("https://arxiv.org/abs/2501.09136v3", arxiv_id="2501.09136v3")

    results = PresenceRanker().rank(group_publications({"a": [conflict, matching]}), "")

    assert len(results) == 2
    assert all(len(result.work_group["members"]) == 1 for result in results)


def test_html_versions_group_and_explicit_older_version_request_is_honored():
    rows = [
        publication("https://arxiv.org/html/2501.09136v1"),
        publication("https://arxiv.org/html/2501.09136v3"),
        publication("https://arxiv.org/abs/2501.09136v2"),
        publication("https://arxiv.org/html/2501.09137v2"),
    ]

    results = PresenceRanker().rank(group_publications({"arxiv": rows}), "")
    old = PresenceRanker().rank(group_publications({"arxiv": rows}, "Read arxiv 2501.09136v1"), "")

    assert len(results) == 2
    selected = next(result for result in results if "2501.09136" in result.url)
    assert selected.url.endswith("2501.09136v3")
    assert len(selected.work_group["members"]) == 3
    assert len(old) == 2
    old_selected = next(result for result in old if "2501.09136" in result.url)
    assert old_selected.url.endswith("2501.09136v1")
    assert old_selected.work_group["selection"] == "requested_revision"


def test_html_unversioned_and_versioned_routes_group_without_inventing_revision():
    rows = [
        publication("https://arxiv.org/html/2501.09136"),
        publication("https://arxiv.org/html/2501.09136v2"),
        publication("https://arxiv.org/html/2501.09137v2"),
    ]

    results = PresenceRanker().rank(group_publications({"arxiv": rows}), "")

    assert len(results) == 2
    grouped = next(result for result in results if "2501.09136" in result.url)
    assert grouped.url.endswith("2501.09136")
    assert grouped.work_group["selection"] == "stable_source_order"
    assert len(grouped.work_group["members"]) == 2


def test_equal_titles_and_distinct_zenodo_dois_remain_distinct():
    arxiv = [
        publication("https://arxiv.org/html/2501.09136"),
        publication("https://arxiv.org/html/2501.09137"),
    ]
    zenodo = [
        publication("https://zenodo.org/records/100001", doi="10.5281/zenodo.100001"),
        publication("https://zenodo.org/records/100002", doi="10.5281/zenodo.100002"),
    ]

    assert len(PresenceRanker().rank(group_publications({"a": arxiv + zenodo}), "")) == 4


def test_arxiv_identity_policy_version_changes_for_cache_invalidation():
    assert POLICY_VERSION == "scholarly-work-v3"
