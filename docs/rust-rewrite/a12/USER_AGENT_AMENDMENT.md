# A12 shared default request identity amendment

The reviewed native HTTP session adds `http::DEFAULT_USER_AGENT` when no request or session override is supplied. A12's injected `Transport` observes this merged request, so its exact expected header map includes that same versioned default alongside the unchanged per-engine authentication and Accept headers.

This corrects a native test expectation; it does not alter production HTTP behavior or claim that the original Python request identity was identical. The final HeaderMap equality remains exact: missing, changed, or extra headers still fail.

The frozen `qualification-v1.json`, `PARITY.md`, original mappings, test names, and original assertions remain immutable. The local A12 gate accepts only the original test-source pin and absent byte-count field, validates the exact reviewed successor digest, removes exactly one fixed default-identity insertion, and reconstructs the original source digest. Every other source path keeps its existing verification route.

The existing A12 xtask control exercises the real current test bytes and rejects changed caller digests/byte-counts, removed or repeated insertions, altered identity, and changed authentication assertions. No new test identity is introduced.

The focused A12 security and qualification checks passed on the applied three-file amendment, and the current full workspace also passed with the exact expected-header equality intact. This changes only the native test expectation and its finite source verification route; production request precedence remains request override, session override, then the shared default. Final hosted CI is a separate exact-commit requirement.
