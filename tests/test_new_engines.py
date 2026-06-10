"""Tests for new engine adapters — Package, Media, Medical, Legal, GIS, Finance, Gov, Bio."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines

# ---------------------------------------------------------------------------
# Helper
# ---------------------------------------------------------------------------


class MockHTTP:
    """Context manager that patches httpx.AsyncClient to return a mock client."""

    def __init__(self, handler):
        self.transport = httpx.MockTransport(handler)

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


# ---------------------------------------------------------------------------
# PyPI
# ---------------------------------------------------------------------------


class TestPyPIAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"pypi": {"enabled": True}})
        return instances["pypi"]

    @pytest.fixture
    def sample_html(self) -> str:
        return """
        <html><body>
        <div class="package-snippet">
            <h3 class="package-snippet__title">
                <span class="package-snippet__name">requests</span>
                <span class="package-snippet__version">2.32.0</span>
            </h3>
            <p class="package-snippet__description">HTTP library for Python</p>
        </div>
        <div class="package-snippet">
            <h3 class="package-snippet__title">
                <span class="package-snippet__name">flask</span>
                <span class="package-snippet__version">3.1.0</span>
            </h3>
            <p class="package-snippet__description">Web framework for Python</p>
        </div>
        </body></html>
        """

    def test_parse_search_results(self, adapter, sample_html):
        results = adapter._parse_search_results(sample_html, 10)
        assert len(results) == 2
        assert results[0].title == "requests 2.32.0"
        assert results[1].title == "flask 3.1.0"
        assert "HTTP library" in results[0].content
        assert "pypi.org/project/requests" in results[0].url

    def test_parse_search_respects_max_results(self, adapter, sample_html):
        results = adapter._parse_search_results(sample_html, 1)
        assert len(results) == 1

    def test_parse_search_empty(self, adapter):
        results = adapter._parse_search_results("<html></html>", 10)
        assert results == []

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "pypi" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        cls = list_engines()["pypi"]
        assert "packages" in cls.categories


# ---------------------------------------------------------------------------
# npm
# ---------------------------------------------------------------------------


class TestNpmAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"npm": {"enabled": True}})
        return instances["npm"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "objects": [
                {
                    "package": {
                        "name": "express",
                        "version": "4.21.0",
                        "description": "Fast, unopinionated web framework",
                        "publisher": {"username": "expressjs"},
                    },
                    "score": {"detail": {"popularity": 0.95}},
                },
                {
                    "package": {
                        "name": "fastify",
                        "version": "5.0.0",
                        "description": "Fast and low overhead web framework",
                        "publisher": {"username": "fastify"},
                    },
                    "score": {"detail": {"popularity": 0.85}},
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("web framework")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "express" in result.results[0].title

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"objects": []})):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "npm" in list_engines()


# ---------------------------------------------------------------------------
# crates.io
# ---------------------------------------------------------------------------


class TestCratesAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"crates": {"enabled": True}})
        return instances["crates"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "crates": [
                {
                    "name": "serde",
                    "max_version": "1.0.210",
                    "description": "Serialization framework",
                    "downloads": 100000000,
                    "recent_downloads": 5000000,
                },
                {
                    "name": "tokio",
                    "max_version": "1.40.0",
                    "description": "Async runtime",
                    "downloads": 80000000,
                    "recent_downloads": 4000000,
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("async")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "serde" in result.results[0].title

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"crates": []})):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "crates" in list_engines()


# ---------------------------------------------------------------------------
# Repology
# ---------------------------------------------------------------------------


class TestRepologyAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"repology": {"enabled": True}})
        return instances["repology"]

    @pytest.fixture
    def sample_response(self) -> list[dict]:
        return [
            {
                "name": "curl",
                "repo": "homebrew",
                "visible_version": "8.10.0",
                "summary": "Command line tool for transferring data with URL syntax",
                "status": "newest",
            },
            {
                "name": "curl",
                "repo": "debian",
                "visible_version": "7.88.1",
                "summary": "Command line tool for transferring data with URL syntax",
                "status": "outdated",
            },
        ]

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("curl")
        assert result.status == EngineStatus.OK
        assert len(result.results) >= 1

    async def test_search_404(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "repology" in list_engines()


# ---------------------------------------------------------------------------
# Docker Hub
# ---------------------------------------------------------------------------


class TestDockerHubAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"dockerhub": {"enabled": True}})
        return instances["dockerhub"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "results": [
                {"name": "nginx", "description": "Official nginx image", "pull_count": 1000000000, "star_count": 15000},
                {
                    "name": "ubuntu",
                    "description": "Official Ubuntu base image",
                    "pull_count": 2000000000,
                    "star_count": 12000,
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("nginx")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].url.startswith("https://hub.docker.com")

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "dockerhub" in list_engines()


# ---------------------------------------------------------------------------
# MusicBrainz
# ---------------------------------------------------------------------------


class TestMusicBrainzAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"musicbrainz": {"enabled": True}})
        return instances["musicbrainz"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "artists": [
                {
                    "id": "b10bbbf0-c98f-4d8b-8c0a-1f5a0a3d8c9d",
                    "name": "Radiohead",
                    "type": "Group",
                    "country": "GB",
                    "life-span": {"begin": "1985"},
                },
                {
                    "id": "a74b1b7f-71a5-4011-9441-d0b5e412ea11",
                    "name": "Thom Yorke",
                    "type": "Person",
                    "country": "GB",
                    "life-span": {"begin": "1968"},
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("radiohead")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "Radiohead" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "musicbrainz" in list_engines()


# ---------------------------------------------------------------------------
# Open Library
# ---------------------------------------------------------------------------


class TestOpenLibraryAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"openlibrary": {"enabled": True}})
        return instances["openlibrary"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "docs": [
                {
                    "title": "The Great Gatsby",
                    "author_name": ["F. Scott Fitzgerald"],
                    "first_publish_year": 1925,
                    "isbn": ["9780743273565"],
                    "cover_i": 1000000,
                    "ratings_count": 5000,
                },
                {
                    "title": "1984",
                    "author_name": ["George Orwell"],
                    "first_publish_year": 1949,
                    "isbn": ["9780451524935"],
                    "cover_i": 2000000,
                    "ratings_count": 8000,
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("classic novels")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "Gatsby" in result.results[0].title

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"docs": []})):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "openlibrary" in list_engines()


# ---------------------------------------------------------------------------
# TMDB
# ---------------------------------------------------------------------------


class TestTMDBAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": "test-key"}})
        return instances["tmdb"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "results": [
                {
                    "media_type": "movie",
                    "id": 550,
                    "title": "Fight Club",
                    "release_date": "1999-10-15",
                    "overview": "A ticking time bomb of a film.",
                    "vote_average": 8.4,
                    "poster_path": "/pB8BM7pdSp6B6Ih7QZ4DrQ3PmJK.jpg",
                },
                {
                    "media_type": "tv",
                    "id": 1396,
                    "name": "Breaking Bad",
                    "first_air_date": "2008-01-20",
                    "overview": "A high school chemistry teacher turned meth producer.",
                    "vote_average": 8.9,
                    "poster_path": "/ggFHVNu6YYI5L9yRmJ2AGkX6D4.jpg",
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("fight club")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Fight Club"

    async def test_search_missing_key(self):
        instances = discover_engines({"tmdb": {"enabled": True, "api_key": ""}})
        adapter = instances["tmdb"]
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "tmdb" in list_engines()


# ---------------------------------------------------------------------------
# PubMed
# ---------------------------------------------------------------------------


class TestPubMedAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"pubmed": {"enabled": True}})
        return instances["pubmed"]

    @pytest.fixture
    def esearch_response(self) -> dict:
        return {"esearchresult": {"idlist": ["12345", "67890"], "count": "2"}}

    @pytest.fixture
    def esummary_response(self) -> dict:
        return {
            "result": {
                "12345": {
                    "title": "COVID-19 vaccine efficacy",
                    "source": "New England Journal of Medicine",
                    "pubdate": "2024 Jan",
                    "authors": [{"name": "Smith J"}, {"name": "Doe A"}],
                    "elocationid": "doi: 10.1056/test",
                },
                "67890": {
                    "title": "mRNA vaccine mechanisms",
                    "source": "Nature Medicine",
                    "pubdate": "2023 Dec",
                    "authors": [{"name": "Jones B"}],
                    "elocationid": "",
                },
            },
        }

    async def test_search_returns_results(self, adapter, esearch_response, esummary_response):
        calls = []

        def handler(r):
            calls.append(r)
            if "esearch" in str(r.url):
                return httpx.Response(200, json=esearch_response)
            return httpx.Response(200, json=esummary_response)

        async with MockHTTP(handler):
            result = await adapter.search("covid vaccine")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert "COVID-19" in result.results[0].title

    async def test_search_no_results(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"esearchresult": {"idlist": []}})):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "pubmed" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        cls = list_engines()["pubmed"]
        assert "medical" in cls.categories
        assert "health" in cls.categories


# ---------------------------------------------------------------------------
# PubChem
# ---------------------------------------------------------------------------


class TestPubChemAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"pubchem": {"enabled": True}})
        return instances["pubchem"]

    @pytest.fixture
    def name_response(self) -> dict:
        return {"IdentifierList": {"CID": [2244, 962]}}

    @pytest.fixture
    def detail_response(self) -> dict:
        return {
            "PC_Compounds": [
                {
                    "id": {"id": {"cid": 2244}},
                    "props": [
                        {"urn": {"label": "IUPAC Name", "name": "Preferred"}, "value": {"sval": "aspirin"}},
                        {"urn": {"label": "Molecular Formula"}, "value": {"sval": "C9H8O4"}},
                        {"urn": {"label": "Molecular Weight"}, "value": {"fval": 180.16}},
                    ],
                },
                {
                    "id": {"id": {"cid": 962}},
                    "props": [
                        {"urn": {"label": "Molecular Formula"}, "value": {"sval": "C17H21NO4"}},
                        {"urn": {"label": "Molecular Weight"}, "value": {"fval": 303.35}},
                    ],
                },
            ],
        }

    async def test_search_returns_results(self, adapter, name_response, detail_response):
        calls = []

        def handler(r):
            calls.append(r)
            if "name" in str(r.url) and "cids" in str(r.url):
                return httpx.Response(200, json=name_response)
            return httpx.Response(200, json=detail_response)

        async with MockHTTP(handler):
            result = await adapter.search("aspirin")
        assert result.status == EngineStatus.OK
        assert len(result.results) >= 1

    async def test_search_404(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("nonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "pubchem" in list_engines()


# ---------------------------------------------------------------------------
# ClinicalTrials.gov
# ---------------------------------------------------------------------------


class TestClinicalTrialsAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"clinicaltrials": {"enabled": True}})
        return instances["clinicaltrials"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "studies": [
                {
                    "protocolSection": {
                        "identificationModule": {
                            "nctId": "NCT00000001",
                            "briefTitle": "Study of heart disease prevention",
                        },
                        "statusModule": {"overallStatus": "RECRUITING"},
                        "designModule": {"phases": ["PHASE3"]},
                        "conditionsModule": {"conditions": ["Heart Disease", "Cardiovascular"]},
                    },
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("heart disease")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "heart disease" in result.results[0].title.lower()

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "clinicaltrials" in list_engines()


# ---------------------------------------------------------------------------
# CourtListener
# ---------------------------------------------------------------------------


class TestCourtListenerAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"courtlistener": {"enabled": True}})
        return instances["courtlistener"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "results": [
                {
                    "caseName": "Marbury v. Madison",
                    "court": "scotus",
                    "court_string": "Supreme Court of the United States",
                    "dateFiled": "1803-02-24",
                    "citation": ["5 U.S. 137"],
                    "absolute_url": "/opinion/12345/marbury-v-madison/",
                    "plain_text": "It is emphatically the province of the judicial department to say what the law is.",
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("marbury")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Marbury" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "courtlistener" in list_engines()


# ---------------------------------------------------------------------------
# USPTO
# ---------------------------------------------------------------------------


class TestUSPTOAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"uspto": {"enabled": True}})
        return instances["uspto"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "patents": [
                {
                    "patent_id": "12345678",
                    "patent_title": "System and method for machine learning",
                    "patent_abstract": "A system for training neural networks using distributed computing.",
                    "patent_date": "2024-01-15",
                    "patent_type": "utility",
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("machine learning")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "machine learning" in result.results[0].title.lower()

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "uspto" in list_engines()


# ---------------------------------------------------------------------------
# Nominatim
# ---------------------------------------------------------------------------


class TestNominatimAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"nominatim": {"enabled": True}})
        return instances["nominatim"]

    @pytest.fixture
    def sample_response(self) -> list[dict]:
        return [
            {
                "place_id": "123",
                "display_name": "Paris, Île-de-France, France",
                "type": "city",
                "category": "place",
                "lat": "48.8566",
                "lon": "2.3522",
                "osm_type": "relation",
                "osm_id": 12345,
                "importance": 0.9,
            },
        ]

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("Paris")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Paris" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "nominatim" in list_engines()


# ---------------------------------------------------------------------------
# SEC EDGAR
# ---------------------------------------------------------------------------


class TestEdgarAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"edgar": {"enabled": True}})
        return instances["edgar"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "hits": {
                "hits": [
                    {
                        "_source": {
                            "display_names": ["Apple Inc."],
                            "form_type": "10-K",
                            "description": "Annual report for fiscal year 2024",
                            "filed_at": "2024-10-31T00:00:00Z",
                            "cik": "320193",
                        },
                        "_score": 10.5,
                    },
                ],
            },
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("Apple 10-K")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Apple" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "edgar" in list_engines()


# ---------------------------------------------------------------------------
# UniProt
# ---------------------------------------------------------------------------


class TestUniProtAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"uniprot": {"enabled": True}})
        return instances["uniprot"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "results": [
                {
                    "primaryAccession": "P04637",
                    "uniProtkbId": "P53_HUMAN",
                    "proteinDescription": {
                        "recommendedName": {
                            "fullName": {"value": "Cellular tumor antigen p53"},
                        },
                    },
                    "organism": {"scientificName": "Homo sapiens"},
                    "genes": [{"geneName": {"value": "TP53"}}],
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("p53")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "P04637" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "uniprot" in list_engines()


# ---------------------------------------------------------------------------
# Data.gov
# ---------------------------------------------------------------------------


class TestDataGovAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"data_gov": {"enabled": True}})
        return instances["data_gov"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "result": {
                "results": [
                    {
                        "title": "USDA Farmers Market Directory",
                        "notes": "List of farmers markets across the United States",
                        "organization": {"title": "Department of Agriculture"},
                        "metadata_created": "2024-01-01T00:00:00Z",
                        "name": "farmers-market-directory",
                        "resources": [{"url": "test"}],
                        "tags": [{"display_name": "agriculture"}],
                        "score": 50.0,
                    },
                ],
            },
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("farmers market")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "USDA" in result.results[0].title

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "data_gov" in list_engines()


# ---------------------------------------------------------------------------
# FRED
# ---------------------------------------------------------------------------


class TestFredAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"fred": {"enabled": True, "api_key": "test-key"}})
        return instances["fred"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "seriess": [
                {
                    "id": "GDP",
                    "title": "Gross Domestic Product",
                    "observation_start": "1947-01-01",
                    "units": "Billions of Dollars",
                    "frequency": "Quarterly",
                    "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
                    "popularity": 100,
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("GDP")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "GDP" in result.results[0].title

    async def test_search_missing_key(self):
        instances = discover_engines({"fred": {"enabled": True, "api_key": ""}})
        adapter = instances["fred"]
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "fred" in list_engines()
