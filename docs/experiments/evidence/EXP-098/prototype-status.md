# EXP-098 prototype status

This is an unqualified successor prototype, not a registered experiment or live-call authorization. EXP-097 remains terminal and reference-incomplete. The production-readiness goal is not complete.

The citation contract makes the existing evidence-available citation requirement explicit in the prompt and schema. The original semantic validator and quality thresholds remain unchanged. The checkpoint retains all 23 original accepted assignments and excludes the rejected assignment; no grades are repaired or scored.

Seven citation tests, nine transcript tests, and eighteen carry entry-point checks have passed. Carry checks were also run under optimized Python. These are mechanics results, not scientific quality evidence.

The third independent carry review found one remaining blocker: historical verifier source is compared against the checkpoint and subsequently loaded from the checkout. Qualification requires verifying and executing the same bytes, or executing only the owned verified source snapshot. The review loop stopped at the installed three-pass limit; the user then approved one bounded re-entry pass. The fix now executes historical validators and their transitive dependencies from owned verified bytes. Two adversarial tests mutate caller-owned checkpoint data and checkout code after verification and confirm that neither can affect replay. Independent review of this fixed change remains pending.

Assessment preparation wiring is incomplete and unqualified. Protocol metadata is marked prototype-only. No new graders, Jev requests, answer-model calls, searches, or deployment changes are authorized by these files.

After the remaining source-loading blocker is resolved and independently verified, complete preparation/controller wiring, strict qualification, registration, required CI and review before any fresh stage. Preserve cumulative call accounting and require untouched confirmation before production adoption.
