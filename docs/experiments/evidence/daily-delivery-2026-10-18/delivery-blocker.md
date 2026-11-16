Fresh delivery qualification on warning-fix PR #1014 head `9a7a240a8b6c08a2c5c79d53a6095adf84b628e4` reproduces the original ARM tiny-deadline mismatch. GitHub tested merge `d4714ac5c4c4299bac6d22807005d7a49cecd9f0` has parents current main `b430739d...` and that head, with source tree `f214040b03e7dec3e61ddabbee338aba61bc3f28`, identical to the reviewed candidate.

[ARM foundation job 117684595805](https://github.com/magnus919/SlopSearX/actions/runs/39130533356/job/117684595805) failed its F03 `cargo test --locked --workspace --exclude xtask` command. Retained artifact: `rust-foundation-aarch64-unknown-linux-gnu-d4714ac5c4c4299bac6d22807005d7a49cecd9f0`.

Exact original assertion:
```
full_frozen_subset_and_argument_calls_over_native_stdio_process
slopsearx_search/null args={"query":"q","interactive_timeout_ms":true}
$.content[0].text:json.results: lengths 3 != 6
```
Nine t06_transport tests passed; one failed. This is not the previous W02 AuthorityLost failure, which #1016 corrected and which the initial workspace pass cleared here. The focused local T06 gate and both new warning regressions passed, but they do not override this failed Linux full-workspace observation. No source-regression cause or legitimacy of the three-result envelope is established by the count mismatch alone.

The warning-preservation PR remains unmerged, with original input/captures/assertions retained. No retries, weakened comparison, inflated deadline, protection bypass or new live-provider calls. The outstanding causal/complete-envelope qualification criteria in this issue still apply. Daily delivery documentation will preserve this fresh evidence and distinguish branch progress from changes delivered to main.
