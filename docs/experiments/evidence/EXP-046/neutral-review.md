# EXP-046 neutral acceptance review

I independently checked the saved neutral acceptance against the frozen runner (`replay.py.txt` SHA-256 `e8c32a5cb6c7e5276d687c9d0ed8f1e390dc313ff927b2b1c389dbd36be91ad7`) after merge `04b9a4849d06d2c2131cf51e328a3ec23884f5c2`. The runner's read-only `--verify` passed and returned 13 pins, 65 historical receipts, 21 planned operations, the expected 11-question/721-entry neutral shape, and `neutral: true`.

The raw response is 14,029 bytes with SHA-256 `fd38e3ab44b66fc255249015beb774c8186a08f1e0984a665be31785f95b33ee`; its saved receipt binds that hash and the exact neutral request body hash `91ac3c061ed99977dd964ae4d0794900eb8c5d081f0684fb8fad8f398b5216e1`. Raw JSON, saved parsed response, and receipt response agree. The pinned model is `jev-1.13.0`; all 11 expected answers are present with exact option sets and Choice type. I recomputed each probability total using the registered Decimal semantics: every question sums exactly to 1.0, all reported choices are maxima, and the accepted vectors remain unmodified.

The attempt ledger contains exactly one start and one finish for `neutral-acceptance`, with the same request hash; the finish binds raw response hash and byte count, valid status, no error, usage, and elapsed time. Receipt and attempt evidence agree on 44,566 input tokens, 7,375 output tokens, and 474.44078396074474 ms HTTP elapsed. The request is within the frozen body/state, response, input, and 1,000 ms deadline limits. Retry count is zero.

This verifies acceptance of the one synthetic neutral batch only. It does not establish search quality, candidate uplift, token fit for other requests, adoption, or production readiness. No new provider or search call was made during this review.
