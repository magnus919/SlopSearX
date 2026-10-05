# Q4 rotation: bounded offline diagnosis

This is a post-hoc descriptive inspection of EXP-075 saved results. It makes
no new calls and does not revise the rejected outcome, weights or quality
gates. The source run hash is bound in the accompanying JSON.

Q4 contains 59 cards. Rotating by 20 changes which 40 cards receive query
scores: only 21 remain shared between the two prefixes. The remaining 19
scored cards change, along with local IDs and shared state.

| Component | Top-ten overlap | Shared scored cards | Changed shared scores |
| --- | ---: | ---: | ---: |
| Independent W0 | 0.0 | 21 | 21 |
| Composite query order | 0.0 | 21 | 20 |
| Composite purpose order | 0.7 | 59 | 58 |
| Fused candidate | 0.1 | not a score population | not applicable |

The q4 repeat used byte-identical control and composite requests and retained
nine of the base top ten (overlap 0.9). This is a limited contrast with rotation,
not a causal estimate or a guarantee for a revised request.

The query branch changes coverage before any score noise is considered.
Its unscored tail inherits incumbent order, while equal-weight fusion uses
that resulting full ranking. Purpose scoring sees all cards, but its shared
query state still changes with the prefix. These observations cannot isolate
context effects from model randomness; they identify a mechanism to investigate.

A possible next hypothesis is canonical complete-pool query and purpose
question populations, with stable local mapping and identical request bytes
under provenance rotation, retaining the independent ordinary W0 comparator.
At 80 cards it would require up to 160 questions and fresh capacity/context
admission, development gates and untouched confirmation. This is unregistered
and unqualified; do not implement it or send provider calls on this diagnosis.
Prior whole-pool studies and the wrapper/token bounds must be checked before
registration. It is not a claim that canonical ordering produces better ranks.
