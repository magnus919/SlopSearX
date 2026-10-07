# EXP-042 qualified execution packet

The registered 48-call development comparison is prepared and ready for serial execution. No live EXP-042 provider calls or new searches are included in this packet. All prepared input cards are saved public-document results.

- [Qualified input hashes](qualification-input-hashes.json) freeze the runner, fixtures, analysis, exact contract and grouping artifact.
- [Prepared manifest](prepared-manifest.json) binds all 48 serialized request bodies, sealed reference hashes, grouping, request sizes and operational limits.
- [Pre-call verification](pre-call-verification.json) records independent checks and zero live receipts.
- [Qualification review](qualification-review.md) records findings and their pre-call disposition.
- [Boundary check](boundary-check.json) rejects the 527,520-byte all-field-max body locally. The largest actual prepared body is 351,588 bytes and fits its 384,000-byte limit.
- [Grouping artifact](postgroup-qualification.json) preserves every original member and selected representative. The cardiac pool remains 44 cards; the nominal constructed80 pool becomes 79 after grouping.

The references and thresholds are unchanged. A successful development run only authorizes the separately registered untouched confirmation; it does not justify production adoption.

From a checkout containing this packet, with Python and the project's dependencies:

```sh
python docs/experiments/evidence/EXP-042/replay.py.txt --selftest
python docs/experiments/evidence/EXP-042/replay.py.txt --verify
python docs/experiments/evidence/EXP-042/replay.py.txt --run "$PRIVATE_TRANSPORT_ARGV"
python docs/experiments/evidence/EXP-042/replay.py.txt --analyze
```

`PRIVATE_TRANSPORT_ARGV` identifies a private JSON argv file for the already authorized transport. It is not stored in the repository. A missing, failed, interrupted, uncertain, expired or budget-exhausted attempt cannot be retried. Inspect the terminal receipt and ledger, retain the failure, and publish the result. Do not alter the frozen candidate, replace cases, fill pools, or issue additional searches to rescue this run.

The private transport was qualified with neutral mocked responses and strict duplicate/nonfinite parsing. Its sanitized [qualification receipt](transport-qualification.json) excludes operational hostnames and credentials. No deployment or Hermes configuration change is authorized by this experiment packet.
