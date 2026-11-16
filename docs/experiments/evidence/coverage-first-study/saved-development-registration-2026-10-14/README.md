# Ranking test using saved development results

Status at registration: setup checks passed; neither study stage had started. Ranking quality was unproven.

Later outcome: development stopped at import admission before any scientific calls. Confirmation was not started, and these identities are retired. See the [terminal record](../completed-2026-10-14-saved-admission/README.md). The JSON registration materials below remain the original prospective snapshot.

The development test reuses the complete real search results and page captures from the earlier collection. It uses new blind grades and makes no new search or capture requests. Old grades are excluded. Original collection failures remain in the data.

Confirmation uses separate, untouched cases and fresh search results. It can start only after development passes every quality and usage check. The candidate, thresholds, models and usage ceilings are unchanged.

Both stages passed the runner and independent issuer setup checks in a clean, locked environment at source `49858fd75b0197b3b3421bb688cd4dfb3c2cae2d`. The [setup summary](static-preflight-summary.json) records the zero-call checks and earlier setup errors. An offline check of the complete saved archive compiled all 39 ranking requests and prepared grading packets from 198 captured contexts. This proves setup only.

These files fix the source revision, inputs, stage identities and test settings before execution. The late-phase files are placeholders until the stage creates its actual materials. The [method](../saved-development-method-materials/README.md) explains how saved development inputs differ from live confirmation.

Production behavior and deployment remain unchanged. Simulated engine replay is for engineering tests and does not count as research-quality evidence.
