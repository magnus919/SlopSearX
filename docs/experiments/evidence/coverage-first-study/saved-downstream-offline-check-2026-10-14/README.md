# Offline check of later saved-results stages

An offline audit found that later test stages were missing the receipt identifying the saved-results import. The first registration request included it, but the ranking and answer requests did not. Those requests would therefore have failed verification.

The runner now carries the same import receipt and original archive identity into every later request. It validates that reference and rejects conflicting caller-supplied fields before dispatch.

The [check summary](summary.json) records the result. Using the complete real archive, the old source failed downstream reconstruction. The fix allowed all three later request types to reconstruct the same 11 materials and eight research tasks. The archive still prepared all 39 ranking requests and 198 captured contexts.

These checks use temporary, non-authoritative registration and receipt fixtures. They exercise the actual request-context and archive-reconstruction code, but do not execute ranking or grading. No scientific call, study invocation or production change occurred. Ranking quality remains unproven; the earlier failed study remains terminal.
