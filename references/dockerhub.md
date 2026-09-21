# Docker Hub

Search Docker container images. No auth required.

- **File:** `engines/dockerhub.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, it, reference, packages
- **Rate limit:** 100 req/6h (unauthenticated)
- **Base URL:** `https://hub.docker.com/v2/namespaces/{namespace}/repositories`

## Usage

```
?q=<query>&engines=dockerhub
?q=<query>&categories=packages
```

The adapter sends Docker Hub's documented `name` query parameter (a partial
repository-name filter) together with `page_size`. A simple query such as
`python` uses the `library` namespace; a qualified query such as
`acme/python` uses the `acme` namespace. Docker Hub's API endpoint is scoped
to the selected namespace.

## Response

Returns container image results with name, description, star count, pull count, and registry URL.

## Notes

- Free, public API. Authenticated rate limits are higher.
- Official images are under the `library/` namespace.
- Exact repository-name matches are prioritized locally while legitimate
  partial matches remain in the response.
- A configured `base_url` is used as the endpoint override; it must accept the
  adapter's `name` and `page_size` parameters.

See Docker's [list repositories in a namespace](https://docs.docker.com/reference/api/hub/latest/operations/listNamespaceRepositories/)
documentation. The legacy `/v2/repositories/{namespace}` endpoint is
[deprecated](https://docs.docker.com/reference/api/hub/deprecated/).
