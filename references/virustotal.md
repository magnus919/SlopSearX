# VirusTotal

Search file, URL, and domain reputation data. Requires API key.

- **File:** `engines/virustotal.py`
- **Type:** API
- **Auth:** `ENGINE_VIRUSTOTAL_API_KEY` (required)
- **Categories:** security, malware
- **Rate limit:** 4 req/s (public API)
- **Base URL:** `https://www.virustotal.com/api/v3`

## Usage

```
?q=<query>&engines=virustotal
?q=<query>&categories=malware
```

## Response

Returns file/URL/domain analysis results with detection ratio, malicious count, total engines scanned, and community score.

## Notes

- API key required (free tier at virustotal.com).
- Public API limited to 4 requests per minute.
- Covers file hash, URL, domain, and IP analysis from 70+ antivirus engines.
