# Nominatim (OpenStreetMap)

Search geographic locations and addresses via OpenStreetMap data. No auth required.

- **File:** `engines/nominatim.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, geography, reference
- **Rate limit:** 1 req/s (usage policy)
- **Base URL:** `https://nominatim.openstreetmap.org/search`

## Usage

```
?q=<query>&engines=nominatim
?q=<query>&categories=geography
```

## Response

Returns location results with display name, latitude, longitude, place type (city, street, building, etc.), importance score, and OSM ID.

## Notes

- Free, public API from OpenStreetMap.
- Rate limit of 1 request per second (usage policy).
- Use `User-Agent` header identifying your application as required by Nominatim policy.
- Results sorted by importance by default.
