# EXP-076 canonical population capacity preview

Status: **hypothetical offline serialization preview; not registered, qualified, or a ranking result.**

The preview reuses pinned EXP-075 inputs and the ordinary W0 query instructions, while projecting two complete question populations against one canonical full-pool state. Candidate IDs remain the frozen global IDs. It writes all query questions first, then all purpose questions. Four permutations (base, reverse, rotate by 20, and deterministic shuffle) are serialized for each pool.

No provider, search, or credential access occurred. No scores, ranks, or fusion outputs were calculated.

## Capacity results

Across the 13 pinned natural pools (largest n=59), projected requests range from 36,179 to 251,111 bytes; state from 4,173 to 34,140 bytes; total questions from 18 to 118. Every base/reverse/rotate/shuffle request and global-ID mapping is identical within each pool.

The synthetic 80-card fixture serializes to 270,847 request bytes and 10,462 state bytes with 80 query plus 80 purpose questions (160 total). Those byte lengths fit EXP-075's 384,000-byte candidate and 128,000-byte state envelopes, but 160 questions exceed its 120-question cap.

The synthetic card text is short, while the largest pinned natural pool has 59 cards. These measurements do not establish natural 80-card envelope size, token context fit, or provider acceptance.

## Relation to saved q4 evidence

The source-bound saved q4 candidate bodies were 219,753 bytes (base), 219,753 bytes (repeat), and 221,465 bytes (rotated). Base and repeat SHA-256 values are equal; rotation differs. Canonical complete-pool bodies remove this deterministic permutation effect in the projection, but cannot explain or prevent the score changes and same-request repeat variation already observed.

## Frozen source bindings

Base revision: `c9f2b48437c65c4ebc17f2c7ee18011ae440c119`. EXP-075 protocol SHA-256: `4db746a5784c778c6977ad90632e680182e86bcbea36a709d36f92eaea32052c`. Registration SHA-256: `fbff07e8a14fb99608c838a2ef05207664901fcd69e540518b996e089d0c357d`. The JSON records all seven EXP-075 frozen input hashes and the builder source hashes.

This projection changes the question inventory from at most 120 to 160 at 80 cards. Any experiment using it needs a separate protocol and fresh admission/quality gates; no EXP-075 protocol or result is changed here.

Reproduce with `python3 docs/experiments/evidence/EXP-076/capacity-preview.py.txt` from the repository root. The script reads pinned files and local source only.
