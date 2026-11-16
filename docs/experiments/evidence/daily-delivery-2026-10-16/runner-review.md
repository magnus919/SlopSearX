Runner review of exact candidate 9ab07ac9dbfdcd6d25bc19c472ea4230519bc0ef: positive, no unresolved findings.

Reviewed HTTP GET/form POST forwarding and duplicate/unknown date rejection, shared validation before cache/dispatch, explicit audited OpenAlex-only eligibility, cache proof validation, JSON/YAML receipts and outcome status, science MCP scope/grants, and portal hidden controls/continuation links. Inclusive bounds are checked by the real OpenAlex adapter regression with mocked upstream; malformed/missing/out-of-range dates cannot silently establish a bounded result.

Droid identified missing YAML proof, fixed in 85b0c1ce. Its subsequent LGTM predates the final 9ab07ac9 commit; this review covers that final delta: CSV/RSS reject bounded requests before acquisition while their unbounded behavior remains unchanged. This intentional restriction is documented and covered for both HTTP methods. No unsupported engine is granted date enforcement and specialist grants remain unchanged.

Independent validation on this exact candidate: 319 tests passed across test_server, test_formatter, test_filter_semantics, test_mcp_enforcement and test_mcp_tools. Current-head hosted implementation, portal/browser, SearXNG, Valkey, optional Jev, quality, security and build checks are successful; disabled/conditional jobs are skipped. The green automated review job is not used as a substitute for substantive review.

This establishes the finite bounded-date contract correction, not production incidence, retrieval quality, or deployment. Existing historical local full-suite environment limitations remain as documented in the PR. Standing maintainer authorization permits normal merge after these gates; no protection bypass requested.
