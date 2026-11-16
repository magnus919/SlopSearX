# Saved-results test stopped before grading

The registered development run stopped at saved-data import admission. The importer checked for a cohort hash in the initial registration, but that format stores the hash in the protocol. No import receipt was issued. No grading, ranking, answer, search or capture calls ran, and confirmation did not start.

The failed runner was stopped after the issuer refusal. This run is terminal and was not retried. Its four stage identities are retired. Production remains unchanged.

The repair checks the protocol cohort hash, matching the existing registration format. Regression coverage removes the extra cohort pin that an earlier fixture had supplied and verifies that an incorrect protocol cohort hash is rejected.
