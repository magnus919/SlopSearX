# Repology

Search package versions across multiple OS repositories. No auth required.

- **File:** `engines/repology.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** None published
- **Base URL:** `https://repology.org/api/v1`

## Usage

```
?q=<query>&engines=repology
?q=<query>&categories=packages
```

## Response

Returns package results across repositories with name, versions, repository count, and status badges.

## Notes

- Free, public API.
- Useful for checking package availability across Linux distributions and other package managers.
