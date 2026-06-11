# Have I Been Pwned

Check if email addresses have appeared in data breaches. Requires API key.

- **File:** `engines/hibp.py`
- **Type:** API
- **Auth:** `ENGINE_HIBP_API_KEY` (required)
- **Categories:** security, reference
- **Rate limit:** 1 req/s
- **Base URL:** `https://haveibeenpwned.com/api/v3/breachedaccount`

## Usage

```
?q=<query>&engines=hibp
?q=<query>&categories=security
```

## Response

Returns breach results with breach name, domain, breach date, data classes (emails, passwords, etc.), and description.

## Notes

- API key required (free tier available at haveibeenpwned.com/API/Key).
- Uses k-Anonymity model for password lookup — no plaintext passwords transmitted.
- Covers over 12B breached records from known security incidents.
