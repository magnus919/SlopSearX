# RubyGems

Search Ruby gem registry. No auth required.

- **File:** `engines/rubygems.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** None published
- **Base URL:** `https://rubygems.org/api/v1/search`

## Usage

```
?q=<query>&engines=rubygems
?q=<query>&categories=packages
```

## Response

Returns Ruby gem results with name, version, description, downloads, and authors.

## Notes

- Free, public API.
- Results include gem metadata like homepage URL and documentation link.
