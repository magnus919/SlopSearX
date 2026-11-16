# Ranking comparison after the file-publication repair

Both stages passed runner and independent issuer setup checks. This folder preserves the setup-time snapshot: no stage had started at registration, and ranking quality was unproven. The development stage was later invoked, completed 54 native reference grades and independent closure, then stopped at selector-map admission. The coordinator inventory remains in progress; see the [terminal record](../completed-2026-10-15-selector-map-admission/README.md) and the [execution plan](../../../coverage-frozen-input-execution-plan.md) for current status. The [setup receipt](setup-checks.json) records zero scientific calls and no execution directories created at preflight time.

Development reuses the complete saved search results and page captures, including collection failures, with fresh blind grading. It makes no new search or capture calls. Confirmation uses separate cases and fresh results, and starts only if every development quality and usage check passes. The candidate, thresholds, model aliases, concurrency and usage limits are unchanged.

Source `d01c335615906f28e6f28dff5b9cd6eb5534d5ff` fixes a race where a reader could see a request file before its JSON was written. Files now become visible only when complete, and the collector waits for the dispatch summary to finish publishing before checking its directory. The [source inventory](current-qualified-source-closure.json) binds all 18 study modules. These engineering repairs do not establish ranking quality.

This replaces an earlier prepared registration that was never run. Its four identities are retired. The [previous executed attempt](../completed-2026-10-15-native-schema-rejection/README.md) remains a terminal failure and will not be resumed. Late-phase files remain placeholders until execution produces them. Production behavior and deployment are unchanged.

The [readable cohort view](cohorts-readable.json) is for reading; all task and limit fields match the frozen cohort. The [cohort reference](cohort-reference.json) identifies the original bytes used for execution.
