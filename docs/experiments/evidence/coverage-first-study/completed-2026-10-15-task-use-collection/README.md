# Research-ranking test stopped during retrieval

The development run could not assemble the complete input set required by its [registration](../prospective-2026-10-15-research-task-use-v4/README.md). It ended after 26 physical requests and about 112 seconds. The candidate's ranking and research usefulness were not measured.

The first arXiv request returned HTTP 429. Later arXiv requests were suppressed by the existing rate-limit handling rather than retried. Five research cases reported an arXiv engine failure; three of those produced 40 cards, below their registered 41-card minimum. The GitHub query `backstage platform` returned 20 repositories without the official `backstage/backstage` target. That navigation case therefore also failed its input requirement. This happened before either W0 or the candidate ran.

The other 25 HTTP requests returned 200: 11 GitHub, seven OpenAlex and seven Wikipedia. The per-case counts and failures are in [outcome.json](outcome.json). A successful HTTP response alone did not make the complete study eligible.

The runner stopped as required. No source captures, reference grades, Jev calls, research answers or answer grades followed. Confirmation was not started. There were no retries, replacement queries, substituted pools, or Brave requests. The earlier setup fixes and passing CI do not turn this retrieval failure into a quality result.

The original records are retained privately with the hashes in [retention-receipt.json](retention-receipt.json). Private endpoints, filesystem paths, process details, credentials and raw response bodies are excluded here. The development and unused confirmation identities are retired.

Issue #516 remains unqualified. Keep the existing production ranking. The retained pools are exposed diagnostic material; they cannot stand in for an untouched confirmation run. This record does not propose or authorize another comparison. Shared-service adoption still requires the missing quality evidence and compatibility work.
