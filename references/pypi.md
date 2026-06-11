# PyPI

Search Python Package Index. No auth required.

- **File:** `engines/pypi.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** None published
- **Base URL:** `https://pypi.org/simple/`

## Usage

```
?q=<query>&engines=pypi
?q=<query>&categories=packages
```

## Response

Returns Python package results with name, version, summary, author, and project URL.

## Notes

- Free, public API.
- Results include package metadata like license, classifiers, and release history.
