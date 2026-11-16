# F15 native reranking and read-boundary advisories

This package is implemented and awaiting full independent package review and maintainer acceptance. Historical Python source is pinned at `4dfc0f5691b6a5a1c039a2837372b1977422ec27`; its 13 source blobs remain unchanged after normal integration of accepted A04 main. The [coverage map](coverage.json) retains all 77 parameterized original tests, each source assertion and its native evidence or named successor boundary.

The actual native `JevReranker` performs one bounded HTTP POST, two concurrent requests, a one-second queue-inclusive deadline, a 128,000-byte request ceiling, a 2,000,000-byte decoded response ceiling, and an additional 8MiB wire ceiling. `HostReranker` exposes only bounded public candidate fields and accepts an exact permutation of provider IDs. It rebuilds results from original host objects, preserving the unshortlisted tail. Configured reranking preserves the existing specialist-promotion interaction. Errors retain the fallback and prevent canonical cache admission. Concrete provider and host policy identities automatically namespace cache and flights; a caller's manual routing identity cannot mask them.

The optional native response-header limit rejects an oversized identity Content-Length before reading its body. Other HTTP consumers retain their existing default. Repeated encodings are combined, malformed Python integer text skips this early guard, and streaming decoded/wire limits still apply.

Advisories are produced from the full canonical result count at the read boundary, outside canonical cache and snapshots. The private immutable capture keeps each caller's query ID, view and scope; authorization and client quota execute once through the original shared path. The trusted optional `AdvisoryCatalog` supplies current, secret-free eligibility facts. T02/T03/T06 must wire the live runtime catalog and policy, then qualify their own public serializers and portal HTML. An injected test catalog is not production composition evidence.

`cargo xtask check F15` replays immutable source JSON and actual local sockets/private Valkey only. Native tests cover exact request bytes, malformed/duplicate/nonfinite documents, identity isolation, sensitive no-send, cache admission, full original results, cancellation/transport EOF, concurrent shutdown and current catalog refresh. Parser tests additionally qualify deep flat arenas, error destruction and cooperative current-thread deadlines. There are no provider calls or measured relevance/performance claims.

Developer-only reproduction (pinned CPython 3.14.7, mock HTTP; never a runtime or mandatory gate dependency):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f15/capture.py --output /private/tmp/f15-source.json
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f15/capture_headers.py --output /private/tmp/f15-headers.json
```

The 150-case corpus preserves its original 124-case prefix exactly. A separate 12-case header corpus records actual body reads, including malformed and duplicate headers. Both exporters are temporary developer oracles owned by F15/G08; retire executable Python before backend removal while retaining immutable observations. See [PARITY.md](PARITY.md) for explicit finite boundaries and numeric consumer limits.
