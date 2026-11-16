# Saved-development admission failure

The saved-results development test stopped before grading or ranking. One import-admission request was made, and the issuer refused it. No permit was issued, no grading packets were created, and no search, capture, Jev or model calls occurred. Confirmation was not started.

Both zero-call setup checks had passed. The actual admission path still had two defects: it passed a static, zero-start clock to the saved-import validator, and it required the new runner revision to equal the archive's original revision. Inspection of the registered source identified these defects; the failed issuer was not retried.

After admission failed, the waiting runner was stopped. The recorded exit and issuer refusal are retained. The [summary](summary.json) records the one-shot outcome and zero scientific usage. This is an incomplete setup result, not a ranking-quality result.

The development and confirmation identities are retired. Earlier registration and setup evidence remain historical records. Repair tests must exercise the actual issuer path using offline fixtures; they do not qualify ranking. Production remains unchanged.
