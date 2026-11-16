# Jev response compatibility checks

The experimental parser now accepts the observed numeric representation of Jev
Score and Choice answers. Both the 40-result and 80-result synthetic requests
completed within the existing one-second limit. This establishes software
compatibility; it does not establish that the ranking improves research.

## What changed

Reported decimal hundredths can arrive as an immediately adjacent floating-point
value. Displayed probabilities can also total 0.99 or 1.01 after rounding. The
parser previously rejected these responses before completing the comparison.

Validation now checks whether the displayed hundredths could represent an
underlying probability distribution that sums to exactly one. Each displayed
probability has a bounded interval of plus or minus 0.005, clipped to zero and
one. Score answers must also admit the reported weighted mean within its
rounding interval. A nominal hundredth or its immediately adjacent float is
accepted for this calculation; a second adjacent value is rejected. Values
outside this contract remain errors. The parser preserves reported scores and
probabilities, choice maxima, and tie handling.

The [provider Score reference](https://docs.typesafe.ai/primitives/score) does not
promise this rounding precision. This is a bounded compatibility rule supported
by archived synthetic responses, not a claim about the provider's internals.

## Results retained during development

Every failed check remains a failed check. Each source-version check used a
fresh manifest, stopped on its first failure and made no retries.

| Source | Request | Result |
| --- | --- | --- |
| `20a2a5d5d900` | 40 results | Transport/contract failure; response unavailable, cause unknown; 80-result request skipped. |
| `20a2a5d5d900` | Separate two-result diagnostic | HTTP 200; score and probability mean rejected. |
| `12923b96df3f` | 40 results | HTTP 200; adjacent-float hundredths rejected; 80-result request skipped. |
| `5c8f5ea10203` | 40 results | HTTP 200; one rounded Score probability total rejected; 80-result request skipped. |
| `9bcb3ca4a0b3` | 40 then 80 results | 40 passed in 380 ms; 80 returned HTTP 200 in 457 ms but two rounded Choice probability totals were rejected. |
| `f236d4f14209` | 40 then 80 results | Both passed, in 438 ms and 518 ms. |

The final check used `jev-1.13.0`, the existing questions and serial schedule,
and the existing one-second whole-phase deadline, 384,000-byte request limit
and 2,000,000-byte response limit. No context-extension header was added.

| Final request | Questions | Input tokens | Output tokens | Result |
| --- | ---: | ---: | ---: | --- |
| 40 results | 80 | 32,348 | 6,724 | Valid response and complete unique ordering |
| 80 results | 160 | 64,308 | 13,484 | Valid response and complete unique ordering |

The final source revision was
`f236d4f142095bc7607cbcbe290a94826e438f96`; parser SHA-256 was
`3be05771ebd32fcf18433ee87db031bd190ee9615237a95e4bbd96108487067a`.
The private final manifest SHA-256 was
`6a1bf2885a7f27b240867342c6ef38df912d906d22fd75f06ada894bd04a33c1`.
Complete response hashes were:

- 40 results: `a84f022937291013e7f60171fbb1c3b8e6713894e02afc36a7e3f85b26eafc3f`.
- 80 results: `f26baf3dfd44d9859936acabd8625849a9ad7a47696b24eef84a262750c97ea2`.

Responses remain private. No search, capture or Brave calls were used by these
engineering checks. No deployment changed. Offline replay also validates the
five saved diagnostic responses while preserving every reported score. Replays
do not turn failed live checks into passes.

## Next step

Run the separately registered development comparison from the corrected source.
Only a complete development pass permits the untouched confirmation comparison.
These engineering results provide no ranking-quality credit or study admission.
