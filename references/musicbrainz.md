# MusicBrainz

Search music metadata: artists, releases, recordings, and labels. No auth required.

- **File:** `engines/musicbrainz.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, music, reference
- **Rate limit:** 1 req/s (MusicBrainz policy)
- **Base URL:** `https://musicbrainz.org/ws/2`

## Usage

```
?q=<query>&engines=musicbrainz
?q=<query>&categories=music
```

## Response

Returns music results with artist name, release title, recording title, release date, country, and MusicBrainz ID.

## Notes

- Free, public API from the MusicBrainz community.
- Strict rate limit of 1 request per second enforced.
- Use `User-Agent` header identifying your application as required by MB policy.
- Covers artist discographies, release groups, recordings, and label data.
