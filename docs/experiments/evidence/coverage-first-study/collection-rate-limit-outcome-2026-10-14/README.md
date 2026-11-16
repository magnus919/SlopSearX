# Development collection stopped on upstream rate limits

The development run on source revision `2240dc906836c8cb722f2f47334985e43746e658` stopped before capture, grading, ranking, or answers. GitHub returned six API rate-limit responses; arXiv returned HTTP 429. Only three of eight research cases and three of five navigation cases had eligible inputs. The remaining pools did not meet the registered requirements.

There were 28 public HTTP requests and no Brave searches. No requests were retried. Confirmation was not invoked and production was unchanged.

This is an inconclusive collection result, not a ranking-quality result. It provides no evidence that the candidate improves or worsens ranking. No quality credit is claimed, and the missing cases must not be dropped or replaced to produce a passing result.

The sanitized [summary](summary.json) records the source and method hashes, request counts, affected cases, and phases that did not run. Its observations were checked against the saved collection summary, HTTP receipts, GitHub error body, and terminal stage inventory. Private raw artifacts are retained separately; they are not included here.

[PR #837](https://github.com/magnus919/SlopSearX/pull/837) repaired the earlier callback failure. This run stopped before reaching that callback, so it does not establish live ranking or product qualification.
