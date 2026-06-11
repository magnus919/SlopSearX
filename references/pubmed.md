# PubMed

Search biomedical literature via NCBI E-utilities. No auth required.

- **File:** `engines/pubmed.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference, medical, health
- **Rate limit:** 3 req/s (NCBI guideline)
- **Base URL:** `https://eutils.ncbi.nlm.nih.gov/entrez/eutils`

## Usage

```
?q=<query>&engines=pubmed
?q=<query>&categories=medical
```

## Response

Returns biomedical article results with title, authors (up to 3), journal source, PMID, and publication date.

## Notes

- Free, public API from NCBI (National Center for Biotechnology Information).
- Two-stage pipeline: ESearch for PMIDs, then ESummary for article details.
- Covers over 36M citations from MEDLINE and life science journals.
- Handle with `User-Agent: SlopSearX/0.1.0` header for responsible usage.
