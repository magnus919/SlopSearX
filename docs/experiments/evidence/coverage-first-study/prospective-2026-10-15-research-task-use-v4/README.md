# Test registration: usefulness of research results

**Subsequent outcome:** development stopped during retrieval after 26 requests. No ranking, answer or grading calls followed; confirmation remains unstarted. See the [terminal record](../completed-2026-10-15-task-use-collection/README.md). The registration files below preserve the pre-run snapshot.

This comparison asks whether purpose-aware Jev ranking helps a researcher complete a task using the sources returned. It compares the candidate with the production ranking on identical result pools. It has not produced a quality result yet.

The test uses the [version-4 method](../research-task-use-method-materials/README.md) without changing its questions, thresholds, or budgets. Development has eight research questions and five exact-document navigation cases. A separate confirmation set has the same size. Development reserves at most 40 physical search requests; confirmation reserves at most 41. Neither uses Brave. Results are collected once and then frozen for the comparison. Failures remain in the record; there are no replacement pools or scientific retries.

Both stages are bound to source `943f0b7ef670b3ffe4a3abb03b2aee54570e6a0c`, which includes the setup fix in PR #872. The runner and issuer preflights passed for both stages in the isolated locked environment. The [preflight summary](static-preflight-summary.json) also records the earlier setup failures. No stage execution directories, search calls, captures, or model calls were created by these preflights.

The committed registration fixes the source, method, cohorts and four distinct stage identities before measurement. The identities exclude the 121 retired identities. Files for later phases are placeholders until collection, capture and blind assessment complete. Private configuration, endpoints, credentials, and raw health responses are excluded.

The development run must pass every registered quality, navigation, stability, resource and completion check before confirmation can run. Confirmation remains unstarted. Passing setup checks does not qualify the candidate or authorize production deployment.
