# Research comparison plan

This plan records the software, cases and limits for a new research-ranking comparison. It has not started and has no quality result.

Development reuses saved public search results and captured pages, including their recorded failures. It makes no new search or capture requests. Reference judgments will be made afresh; previous grades are not reused. If development passes every check, confirmation will collect different public cases and test the same software independently.

The source version is `0974b19dbadf80b07f9be9bb20fec1de5ada4b47`. The questions, case groups, spending limits and ranking-quality thresholds are unchanged. The comparison now uses the corrected Jev response parser. The JSON files record the exact versions and separate run identifiers, excluding all 103 retired identifiers.

Both configurations passed the source-only static preflight in a clean checkout with locked dependencies; see `static-preflight-results.json`. Before starting, verify the complete runtime inputs. The remaining runtime files are placeholders until their inputs exist. Only a complete development pass allows confirmation to start. This plan makes no production-readiness or deployment claim.
