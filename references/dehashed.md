# DeHashed

Search breached credentials and compromised data. Requires API key.

- **File:** `engines/dehashed.py`
- **Type:** API
- **Auth:** `ENGINE_DEHASHED_API_KEY` (required)
- **Categories:** security, threat-intel
- **Rate limit:** 1 req/s
- **Base URL:** `https://dehashed.com/api/v1`

## Usage

```
?q=<query>&engines=dehashed
?q=<query>&categories=threat-intel
```

## Response

Returns breached credential results with email, username, password (hash/plaintext), IP address, and breach source.

## Notes

- API key required (paid service at dehashed.com).
- Use responsibly — handles sensitive credential data.
