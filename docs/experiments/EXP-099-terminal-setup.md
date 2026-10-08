# EXP-099 terminal setup outcome

EXP-099 stopped during setup before any health HTTP request, Jev call, answer-generation call, or fresh grader submission. The research comparison did not run, and no quality or adoption conclusion is available.

The private operator launcher called the controller's asynchronous health-check function without awaiting its returned coroutine. It then printed an incorrect health-success message. Initialization correctly rejected the missing durable health-completion record and made the stage terminal. The original clock and failure are retained; that clock must not be resumed or credited to a successor.

The merged controller and its phase-order guard behaved as designed. The defect was in the unpinned operator invocation. A prospective successor must make the awaited health invocation part of the qualified runtime path and test both asynchronous success and failure at that use site before creating a fresh clock. A launcher message alone is never health evidence.

All 48 original accepted source assessments remain unchanged. This attempt added zero source assessments, answer assessments, or provider calls. The historical 72 grader submissions remain charged. Any successor retains the frozen scientific criteria, budgets and independent-reference requirements, with zero prior time credit and a new registration, qualification and stage identity.
