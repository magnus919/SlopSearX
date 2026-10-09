# EXP-104 — recover finite JSON results from non-finite cached scores

## Registration

Cycle start 2026-10-09T13:00:44.820Z; isolated baseline `e3eb6b4`.
Hypothesis: reject non-finite deserialized result scores so an optional corrupt
cache cannot prevent healthy search results reaching the JSON API. Code evidence:
`search_result_from_dict` accepts float NaN/Infinity, whereas standard JSON cannot
represent them. Production prevalence is unknown. Consequential contract recovery
across HTTP/portal/MCP shared rehydration; not retrieval-quality uplift.

Primary: correct successful HTTP recovery count / six fixed score faults, requiring
results equal the fresh result and cached=false. Faults: NaN, +Infinity, -Infinity
as JSON numeric constants and strings. Require 6/6 candidate and at least +3
recoveries versus baseline. Every recovery dispatches exactly once and subsequent
request is a healthy cache hit without dispatch. Healthy and intentional negative
cache controls must retain their outcomes. All previously registered EXP-032
corruption/JSON controls are retained as broader regression evidence. No cases
excluded, tuned or retried; deterministic corpus, no population interval.

Use existing EXP-032 normal FastAPI/SearchService/SearchCache/disposable real
Valkey worker adapted solely to this fault list, two arms once each; one fixed
healthy adapter, no provider calls. Origin: repository-authored public fixture,
no personal data. Store all attempts/raw bodies/worker and checksums in EXP-104.
Candidate scope: reject non-finite typed result score at shared rehydration;
existing cache recovery and snapshot fail-closed handlers own the outcome. Keep
finite legacy numeric strings, finite negative/zero/positive values and all other
fields. No query/payload/exception text in new logs. No new alert/SLO/deployment.

Measurement readiness: project Python environment and disposable Valkey worker
available; normal GET /search path reachable in local test client. Commands:
PYTHONPATH=. EXP032_OUTPUT=/private/tmp/exp104-ARM .venv/bin/python replay worker;
then service/snapshot/formatter/server contracts, explicit portal browser test,
full regression, type check, hooks and normal exact-SHA CI/review if supported.
Budget: 45 minutes from cycle start; zero upstream/paid calls. Register before
candidate work, retain failures before discarding unsupported code.

Supported only if primary and all guards pass. Portal impact: shared rehydration,
no UI change; contract/browser CI required. Ready implementation PR only if
supported with normal checks; automatic merge requires substantive review+CI.
Documentation and evidence retained for every outcome, DCO [skip ci].

Review: latest completed ranking studies EXP-097 through EXP-103 failed setup,
reference, selector or scientific gates; immutable historical outcomes preserved.
New coverage-ranking amendment awaits decision, so no speculative live retry.
EXP-095 finite prerequisite already delivered; supported query-planning capability
#685 already merged, no other supported runtime candidate awaits delivery.
