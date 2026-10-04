# EXP-037 qualification readout

## Frozen scope

Registration commit: `e6524b7` (registration SHA recorded before implementation).
Baseline source: `bc03032322d3f801454eb2f8119f705e9e1fd2cb`.

This finite synthetic qualification changes only transport/parser bounds and direct input validation. The `jev-1.13.0` model, ten-level `LEVELS` and `INSTRUCTIONS`, first-40 candidate membership, stable ordinal-score tie policy, one-second deadline, two-call semaphore, no-retry behavior, and service routing are unchanged. The cache identity moved from v1 to v2 and fingerprints the decoded response byte limit and strict parser policy.

## Baseline reproduction

Environment: Python 3.12.11, HTTPX 0.28.1, pytest 9.0.3.

The new fixtures were copied into a separate detached checkout of the registered baseline. The baseline command selected five deterministic test functions and exited 1 with **5 failed, 43 deselected**. Each failure is a reproduced baseline behavior, not a missing-constant or setup error:

- Oversized title, URL, and snippet values reached the HTTP handler instead of being rejected before transport.
- Duplicate JSON object keys and non-finite values (`NaN`, `Infinity`, `1e999`, `-1e999`) were accepted by Python's permissive decoder when the required response fields remained valid.
- The baseline accepted response bodies beyond an injected small decoded-byte cap, including a multibyte split across streamed chunks and a gzip-decoded body beyond the cap.
- The baseline accepted a valid response whose announced identity-body `Content-Length` exceeded the injected bound.

The small injected cap makes the byte-boundary fixtures fast while preserving the candidate guard's semantics. The baseline has no corresponding guard and ignores that injected constant. The standard production cap is 2,000,000 decoded bytes.

## Candidate results

Using the same Python 3.12.11 / HTTPX 0.28.1 / pytest 9.0.3 environment:

- `PYTHONPATH=. python -m pytest --no-cov -q tests/test_rerank.py`: **48 passed**.
- `PYTHONPATH=. python -m pytest --no-cov -q tests/test_formatter.py tests/test_server.py`: **114 passed**.
- `PYTHONPATH=. pre-commit run --all-files`: **passed** all hooks, including JSON syntax validation, Ruff, vulture, import-linter, and mypy.
- `PYTHONPATH=. python -m pytest --cov=slopsearx --cov=engines --cov-report=term-missing`: **2,312 passed, 57 skipped, 7 warnings**, 85.41% total coverage, 58.64 seconds. This run followed the finite-float parser addition and used the configured loopback-enabled test environment.

The test matrix covers exact and over-bound direct fields, exact response boundary, streamed and gzip-decoded overflow, announced identity-body oversize, malformed/non-object JSON, duplicate keys, non-finite constants and positive/negative exponent overflow, response model and exact membership, answer type and score validation, stable ties, request-size limits, queue deadline, queued and in-flight cancellation, slot release after failure, redirect rejection, and one-attempt behavior. The baseline reproducer subset selected five failing test functions and observed the described direct-field, ambiguous/non-finite JSON, streamed-body, gzip-decoded-body, and announced-length baseline defects. All 48 candidate `test_rerank.py` cases pass. Valid ordinary and sparse score responses retain their exact expected order.

## Implementation and limits

The response reader rejects an oversized identity `Content-Length` before consuming the body and independently caps accumulated decoded bytes while iterating `aiter_bytes()`. Encoded lengths are not used as a decoded-size proxy for compressed responses. JSON parsing rejects duplicate keys and all non-finite numeric values, including exponent overflow that decodes to infinity. Direct caller text that exceeds existing byte limits is rejected; it is not silently truncated.

This is an application-level bound on accumulated decoded response bytes. HTTPX or its content decoder may transiently allocate a yielded chunk before the application checks its size, so the guard does not claim a strict process-wide peak-memory bound. Provider behavior, response prevalence, ranking quality, and production latency are not evaluated here. No live provider calls or external search requests were made.

The two pre-commit mutation hooks exclude raw JSON/TXT evidence captures and the one historical `EXP-003/reproduce.md` capture because those files intentionally preserve original bytes. All syntax, code-quality, and other checks still run over the repository; the exclusion does not alter the evidence files. `pre-commit run --all-files` passed every hook, including JSON syntax validation, Ruff, vulture, import-linter, and mypy.
