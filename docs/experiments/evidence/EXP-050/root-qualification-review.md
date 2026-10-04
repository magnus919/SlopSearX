# Root qualification review

The root reran all 61 planner checks. The initial draft stripped API namespace prefixes and hashed a body that differed from the saved bytes by a newline. Both flaws were corrected before any request; regression checks now cover documented prefixes, namespace distinctness, wire ordering and exact bytes. All 423 cards and their order, purpose and facets remain intact.

The credential-free private transport source is bound only by its source SHA and frozen public body SHA. Eleven local-wrapper and ten remote-wrapper fixture checks passed without real credentials or network. These cover fixed request identity, secret exclusion from the public envelope, missing/ambiguous configuration, redirect refusal, bounded Retry-After projection, response cap and timeout classification. Its source and deployment details remain private. Fixture results do not prove live availability.

Runner and projection qualification and cross-component integration remain pending. No live metadata, Jev or Brave request occurred.

The first combined fixture failed: its parsed-object adapter lacked the projector's exact raw receipt and record-wire sizes. The failure is retained in initial-integration-failure.json. The runner and root adapter are being aligned to an explicit raw-byte callback; complete combined qualification remains required.

After component fixes, the root independently reran the 21-check runner suite, 11-group projector suite and 13-check combined fixture. All passed. The combined fixture uses the exact 217-query body and complete 423-card plan, reverses response order, verifies all 219 mapped cards and 204 unsupported cards, preserves card content, checks null responses, and rejects wrong cardinality, duplicate keys and oversized records atomically. These are synthetic structural tests, not live metadata or ranking-quality evidence.

The explicit raw-byte callback removes the prior adapter mismatch without synthesizing response hashes or record-wire sizes. The exact raw response receipt belongs only to private runner evidence; public metadata excludes it. The matching-key type correction is prospective and documented separately. Final independent component review is retained in component-integrity-review.md and found no remaining component-level blocker. Current publication integrity must be verified before live acquisition.
