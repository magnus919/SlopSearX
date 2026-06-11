# npm

Search npm package registry. No auth required.

- **File:** `engines/npm.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** None published
- **Base URL:** `https://registry.npmjs.org/-/v1/search`

## Usage

```
?q=<query>&engines=npm
?q=<query>&categories=packages
```

## Response

Returns npm package results with name, description, version, publisher, and weekly downloads.

## Notes

- Free, public API.
- Results include package metadata like keywords and links to homepage/repository.
