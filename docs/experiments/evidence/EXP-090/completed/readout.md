# EXP-090 — complete delivery, protocol-invalid submission

All 43 assigned pages were delivered byte-exactly. The grader returned a 28,435-byte final JSON object. After the description and before the correct page reads, it made one invalid request for pages `0,1`; page indices actually start at one. The instructions and small description did not state that starting index explicitly.

The registered transcript gate required exactly the declared calls with no retry. All 24 actual calls remain retained; the required submission failed with `transcript-call-count`. Comparing successful outputs with the expected byte hashes is diagnostic only. No filtered transcript, repaired grade, accepted assessment, downstream answer or quality score was produced. No new search or source fetch occurred. This stage is terminal and inconclusive.

The remaining evidence gap is one protocol-compliant source assessment and then the still-unfinished independent source/answer assessments, frozen ranking/task-use analysis and untouched confirmation. This result establishes complete byte delivery, not semantic grade validity, research usefulness or production readiness.

The first two setup-only failures remain separately recorded. Nineteen historical grader calls include sixteen valid carried card assignments and three rejected source submissions across EXP-088/089/090. No failed call is refunded.

The bounded next proposal is to make one-based page indexing explicit in both viewer description and pinned instructions, and provide the exact complete command schedule before dispatch. Preserve every input byte, all scoring/coverage/resource gates and the no-repair rule. After three non-converging diagnostic passes, report this gap instead of automatically launching another assessment retry. No model/task competence conclusion follows from this procedural failure.
