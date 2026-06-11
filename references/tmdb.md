# TMDB (The Movie Database)

Search movies, TV shows, and people. Requires API key.

- **File:** `engines/tmdb.py`
- **Type:** API
- **Auth:** `ENGINE_TMDB_API_KEY` (required)
- **Categories:** general, movies, entertainment
- **Rate limit:** 50 req/s (with key)
- **Base URL:** `https://api.themoviedb.org/3`

## Usage

```
?q=<query>&engines=tmdb
?q=<query>&categories=movies
```

## Response

Returns movie/TV results with title, release date, overview, vote average, poster URL, and genre names.

## Notes

- API key required (free at themoviedb.org).
- Covers movies, TV shows, and people across the TMDB catalog.
- Results include popularity and vote metrics for relevance.
