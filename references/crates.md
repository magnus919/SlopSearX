# Crates.io

Search Rust package registry (crates.io). No auth required.

- **File:** `engines/crates.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** None published
- **Base URL:** `https://crates.io/api/v1/crates`

## Usage

```
?q=<query>&engines=crates
?q=<query>&categories=packages
```

## Response

Returns Rust crate results with name, description, latest version, downloads count, and repository URL.

## Notes

- Free, public API.
- Results include crate metadata such as homepage and documentation URLs.
