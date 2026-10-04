# Independent static review

Luna reviewer inspected the runtime patch and focused fixture before PR creation. No runtime or test findings. The recognized HTML route is confined to the existing three arXiv hosts and existing full identifier regex; malformed/deceptive paths remain excluded. Source metadata, ranking, version selection and public result schema remain on existing paths. Policy version v3 flows into the shared cache digest.

The reviewer checked the exact URL-only saved-card case, both rankers, duplicate engine vote bounds, all-member preservation, conflicts, revision selection, serialization and SearXNG projection. This was static review, not an independent test execution or review of all experiment evidence.

Runtime source SHA-256: `cfa7ab4c0326b0a3c3da97e527766a65c657a9865897193d3ae59964779b77d5`. Reviewed fixture SHA: `ede1d18cb9684f6eccc5e051c35bc0751c708f288c8ccf88e66c3d876d714b06`. Pre-commit later changed test formatting only; root verified identical AST and reran the focused tests. Required substantive Droid review and full CI remain pending.

## CI fixture repair review

Python 3.13 exposed a race in the existing Jev queue test: occupied requests had earlier deadlines and could legitimately free admission before the later queued request expired. The repaired test holds both admission slots until queued timeout, confirms no HTTP call, then releases slots in finally and confirms normal admission works. Independent Luna static review found no issues. The test does not change runtime code and would fail if queue acquisition were outside the timeout or cancellation poisoned later admission. Combined grouping/reranking checks passed 103 tests; type checks passed again.

Droid first-pass review on `af8c16bde44698ba7341d65958a7e23104cf26ef` found no verified runtime correctness/security findings. Its ledger-placement finding is fixed by moving EXP-041 into the existing table. CI is rerun for the repaired candidate before merging.
