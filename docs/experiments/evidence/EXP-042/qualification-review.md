# EXP-042 pre-call qualification review

This packet qualifies the registered experiment runner, not a production ranking change. No EXP-042 provider calls or fresh searches had occurred when these checks were performed. The Brave allowance remains unused (0/10).

## Independent checks

The primary reviewer independently verified all 19 registered source hashes and exercised five hand-worked selector cases: greedy facet count before score, inclusive score/probability boundaries, later-input eviction ties, protection of sole representatives and reserved insertions, unchanged ordering when no outsider qualifies, and short/empty pools. All passed. The analysis tests include an aggregate noninferiority failure despite passing individual limits and a complete passing conjunction using actual nDCG calculations.

The maximum-field serialization probe uses every allowed field size, including 80-byte candidate IDs and 64-byte facet IDs. It measures 527,520 bytes; the expected result is rejection without transport dispatch and preservation of the full incumbent order. Earlier short-identifier measurements are retained with their correction. The saved constructed stress pool has 79 representatives after grouping and its actual mixed request is 351,588 bytes. It must not be described as successful endpoint qualification for 80 candidates.

## Findings and disposition

The analysis initially omitted the registered mean E−D* noninferiority guard. This was fixed before provider calls, with a regression test. Review also identified that structured receipts needed to be bound to reparsed raw responses and completion-ledger token usage. The qualified runner must enforce those bindings and reject orphan, uncertain, failed, or tampered attempts before any later dispatch.

The registered useful-card guard compares counts and preserves useful reference facets; it does not require retention of particular card IDs. A reviewer suggestion to add such an identity requirement was withdrawn after checking the registered wording. Lost and gained IDs remain diagnostics.

## Scope limits

The corpus is a synthetic projection of previously saved public result cards. This qualification establishes deterministic mechanics, input integrity, and failure handling. It supplies no evidence of relevance uplift or endpoint feasibility. All 48 real requests must still complete and satisfy the predeclared development gates before untouched confirmation may begin. No runtime defaults, deployments, Hermes configuration, or production search behavior change in this packet.

## Final frozen-code review

The independent Luna reviewer found no remaining blocker in the final runner, response validation, terminal history handling, selector checks, or analysis gates. Root independently ran the same frozen runner's self-test, verified the ten replay fixtures and nine analysis tests, and verified all 48 prepared requests against the qualified manifest and sealed references. The final runner SHA-256 is `3f6c2306e252dee8fdd49e594b16273235b3b16388ab413098c86da3ca7c73f6`; fixture SHA-256 is `5793d7aab21bc49f3bf94dd36c8f7107d2827f7ac62b5cb0ffa694e6a7291a0b`.

If an answer fails validation after its envelope parses, the invalid response receipt retains any supplied usage; the completion ledger may omit it. The run is terminal in either case, so no later request can be dispatched. Inspect the invalid receipt for that usage instead of treating ledger nulls as zero.
