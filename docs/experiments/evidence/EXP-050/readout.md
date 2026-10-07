# EXP-050 result: identifier-bound metadata is available

The bounded lookup completed in one HTTP operation, with no retry or Brave/Jev search. It returned an exact 217-item response and preserved all 423 saved cards across twelve pools.

- 178 cards have identity-matched metadata.
- 41 identifier-bearing cards remain unresolved; this does not establish that their papers are absent.
- 204 cards have no supported URL identifier and remain in the corpus unchanged.
- No conflicting query identities or unmatched returned records were accepted.

The frozen information screen passes: 178 cards exceed the 106-card minimum, and every pool has at least one qualifying card. Cardiac has 30/44 qualifying cards; research 22/44; evaluation 23/44. The other nine pools have 5–20 qualifying cards each. These are metadata-availability counts, not accuracy, usefulness or ranking results.

Reported field coverage among all 423 rows is: title 178, year 178, authors 178, publication date 170, publication types 169, journal 168 and abstract 158. A reported field may still be incomplete. The bounded private projection clipped abstract text in 153 rows and author lists/items in one row; clipping receipts remain explicit. Publication type, venue and indexing do not establish peer review, primary study design, authority or truth. arXiv lookups remain work-base scoped with exact-version content unverified. Original engine provenance remains unknown.

## Publication and execution integrity

Execution used clean reviewed qualification commit `f4b13aa4556ffdce53cab958cf434d98c6dece02`, after PR #559 merged as `7f01500387184a4fca1ab74e733d8635a0a3c4f4`. Preflight verified frozen file hashes, all seventeen source pins, exact body and private transport fingerprints, and all qualification gates before the one-shot acquisition. The published qualification manifest is historical evidence for that execution version; later roadmap/README changes do not retroactively change the frozen run.

Private evidence retains the unchanged registered projection, raw response, raw receipt and durable attempt ledger. Public artifacts contain unchanged original cards, allowlisted factual metadata, status/count receipts and the fixed information-screen outcome. Abstract text is withheld from the public packet; its registered private projection and experiment criteria are unchanged. Recomputing the screen from the public view still gives 178 qualifying cards across all twelve pools. This publication restriction is not a post-result change to the experiment or its assessment thresholds. Credentials, private endpoints, deployment details and private raw receipt hashes are excluded.

## Decision and remaining production scope

This passes the registered information-coverage screen and permits a separately registered enriched visible-support study. It does not qualify another replacement-quality run, approve a selector or finish implementation. EXP-046 remains rejected; EXP-047/048 remain incomplete. No older labels, gates or rankings are revised.

Next establish whether the added metadata enables consistent, properly grounded judgments, explicitly separating discovery value, facet coverage, publication identity, reported study type and unsupported authority claims. Independent assessments must precede Jev scoring. The full [production acceptance checklist](../../complete-pool-production-acceptance.md) remains open: qualified caller-purpose-aware complete-pool policy, untouched confirmation, shared SlopSearX HTTP/MCP/portal/cache/snapshots, experimental GroktoCrawl X forwarding, compatibility/resource/failure tests, substantive review and verified main merges. No deployment or Hermes change occurred.

Independent result review is retained in result-review.md. It recomputed the private raw-derived projection, ledger and published counts; root resolved its publication requirement by binding all post-run artifacts in result-integrity.json before commit.
