# Docker Hub

Search Docker container images. No auth required.

- **File:** `engines/dockerhub.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** 100 req/6h (unauthenticated)
- **Base URL:** `https://hub.docker.com/v2/repositories/library`

## Usage

```
?q=<query>&engines=dockerhub
?q=<query>&categories=packages
```

## Response

Returns container image results with name, description, star count, pull count, and registry URL.

## Notes

- Free, public API. Authenticated rate limits are higher.
- Official images are under the `library/` namespace.
