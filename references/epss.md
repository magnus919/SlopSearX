# FIRST EPSS

Search Exploit Prediction Scoring System scores for CVEs. No auth required.

- **File:** `engines/epss.py`
- **Type:** API
- **Auth:** None
- **Categories:** security, threat-intel
- **Rate limit:** None published
- **Base URL:** `https://api.first.org/data/v1/epss`

## Usage

```
?q=<query>&engines=epss
?q=<query>&categories=threat-intel
```

## Response

Returns CVE EPSS results with CVE ID, EPSS score (probability of exploitation in the wild), and percentile rank.

## Notes

- Free, public API from FIRST.org.
- EPSS scores are updated daily.
- Useful for vulnerability prioritization — higher EPSS scores indicate higher likelihood of exploitation.
