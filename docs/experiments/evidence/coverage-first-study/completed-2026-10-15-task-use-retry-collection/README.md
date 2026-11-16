# Replacement research-ranking test stopped during retrieval

The one permitted replacement development attempt ended before ranking or answer evaluation. It made 30 physical search requests, and all 30 returned HTTP 200. Even so, the collected inputs did not meet the registered requirements: one research pool contained 40 cards where the protocol requires 41–80, and an exact-document navigation target was absent from its result pool.

For D27-R05, `artifact registry` returned 40 cards: 20 from GitHub, 20 from OpenAlex, and zero from arXiv and Wikipedia. For D27-N03, `backstage platform` returned 20 cards but did not include the expected `backstage/backstage` repository URL. The navigation check failed before W0 or the candidate could run. The other 11 operations met their input checks. Per-operation counts and statuses are recorded in [outcome.json](outcome.json).

The replacement collection ran from 2026-10-15 18:25:45 UTC to 18:27:20 UTC. Requests were 5 arXiv, 11 GitHub, 7 OpenAlex, and 7 Wikipedia; all returned HTTP 200. Across this replacement and the original 26-request attempt, 56 requests were made. The published 107-request ceiling included 41 requests reserved for conditional confirmation; that allowance was unused and does not authorize another development attempt. The single replacement allowance is exhausted.

The stage stopped at frozen-pool input. No capture, reference grading, Jev ranking, answer generation, answer grading, or confirmation followed. The runner's normal process exit records an orderly terminal stop; it is not a quality pass. The input shortfalls are not evidence for or against Jev ranking because neither W0 nor the candidate was evaluated.

The [original 26-request attempt](../completed-2026-10-15-task-use-collection/README.md) remains separate. The two collections are not combined into a study pool. The original registration, both terminal outcomes, and their private raw records are retained independently. The public [retention receipt](retention-receipt.json) binds the replacement terminal records and raw archive by hash; raw bodies, endpoints, credentials, and host details are not published.

The replacement and unused confirmation identities are retired. No further comparison is proposed here. Issue #516 remains unqualified and the production ranking remains unchanged.
