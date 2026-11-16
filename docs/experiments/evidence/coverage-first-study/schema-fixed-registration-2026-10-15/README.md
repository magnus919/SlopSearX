# Ranking test with native output-schema enforcement

Status at registration: both stages passed setup checks; neither had started. Ranking quality remained unproven.

Later outcome: the development stage is terminal. Two grader requests were rejected by strict-schema validation because the `propertyNames` keyword is unsupported for `facet_anchors`; no grades completed, and 52 packets remained unstarted. Jev, answer, search, capture, and confirmation phases did not run. See the [terminal record](../completed-2026-10-15-native-schema-rejection/README.md). These registration identities are retired.

Development reuses the complete saved real search results and page captures, preserves collection failures, and uses fresh blind grades. It makes no new search or capture calls. Confirmation uses separate cases and fresh results, and can start only if every development quality and usage check passes. The ranking candidate, thresholds, model aliases, concurrency and usage ceilings remain unchanged.

Source `43e5fa5a217c0366c33203c0b8ffcc9d3dbec728` adds native structured-output schema binding and validates responses before accepting them. Both runner and independent issuer setup checks passed for both stages with no scientific calls. These files fix the source, identities and input settings before execution; late-phase files remain placeholders until execution produces them.

The earlier run is [terminal](../completed-2026-10-15-native-grade-schema/README.md): one malformed response prevented grade closure. Its responses were not reused. Production behavior and deployment remain unchanged. Offline simulations provide engineering evidence only.

The [readable cohort view](cohorts-readable.json) omits historical coordination wording. It is for reading, not execution. All task and limit fields are identical to the frozen cohort. The [cohort reference](cohort-reference.json) identifies the original bytes pinned by the protocol and registrations; execution continues to use those unchanged bytes.
