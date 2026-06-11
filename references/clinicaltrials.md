# ClinicalTrials.gov

Search clinical studies from the US National Library of Medicine. No auth required.

- **File:** `engines/clinicaltrials.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, medical, health, science
- **Rate limit:** None published
- **Base URL:** `https://clinicaltrials.gov/api/query/study`

## Usage

```
?q=<query>&engines=clinicaltrials
?q=<query>&categories=medical
```

## Response

Returns clinical study results with title, status, conditions, interventions, sponsor, phase, and enrollment count.

## Notes

- Free, public API.
- Covers over 450K clinical studies from 221 countries.
