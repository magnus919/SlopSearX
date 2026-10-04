# Initial independent component review

These are findings on initial post-registration drafts, before any final packet publication or grading. Component authors are addressing them; this is not a passing review.

## Validator

- Positive pointers accepted whitespace-only values.
- Directory checks promised symlink rejection without enforcing it.
- Sorting chunks allowed a reordered manifest to pass.
- A self-provided manifest hash did not independently bind metadata to the qualified delivery. Live validation must require a tracked trusted manifest/integrity receipt; synthetic seams cannot silently bypass that in the CLI.
- Unhashable malformed JSON field types could escape as TypeError instead of a sanitized rejection.
- The review size cap was checked only after an unbounded read, and invalid-record hashing read the full file again.

## Builder

- Manifest filename differed from the validator's expected delivery-manifest.json.
- JSON parsing lacked duplicate-key/nonfinite strictness.
- Private abstract-bearing files needed explicit 0600 permissions.
- Output path normalization needed to resolve parent symlinks before excluding the repository; exclusive destination publication needed an explicit no-clobber guarantee.

A no-write preparation against the pinned actual projection passed with 23 chunks and all 232 cards, retained separately in initial-builder-preflight.json. That structural result does not resolve the publication/validator findings or justify grading. No reviewer, provider or search invocation occurred.
