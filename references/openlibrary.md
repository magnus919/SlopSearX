# Open Library

Search books, authors, and editions via the Open Library API. No auth required.

- **File:** `engines/openlibrary.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, books, reference
- **Rate limit:** None published
- **Base URL:** `https://openlibrary.org`

## Usage

```
?q=<query>&engines=openlibrary
?q=<query>&categories=books
```

## Response

Returns book results with title, author, publication year, ISBN, cover image URL, and edition count.

## Notes

- Free, public API from the Internet Archive.
- Covers millions of book records including metadata, editions, and availability.
