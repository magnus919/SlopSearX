# EXP-075 prospective design review

The independent source audit found no identical earlier one-transaction
query/purpose dual-Score ranking comparison. Related batching experiments do not
establish this mechanism's context behavior, maximum acceptance or ranking gain.

Independent review identified and the draft addressed:

- Explicit caller purpose/facets are evaluator inputs and a proposed contract;
  current service/research schemas do not receive them. Full caller propagation,
  cache and artifact identity work remains required before production acceptance.
- The candidate wrapper's newly declared 384 KB cap is separate from the legacy
  128 KB W0/default bound. It is not a documented provider permission or proof of
  compatibility with the current reranker. Provider acceptance is unproven.
- Candidate timing includes actual construction, projection/canonicalization,
  namespacing, serialization, byte comparison, exchange, parsing, fusion and
  durable receipt. Admission previews cannot substitute for measured work.
- Query ties explicitly preserve first-40 input positions before appending the
  untouched native tail. Purpose and fusion ties remain canonical UTF-8.

Bounded follow-up confirmed these design corrections. The wording that implied
a provider byte allowance was corrected to an experimental wrapper cap. The
two separate Score ranges, exact RRF, full-pool purpose projection and atomic
native-incumbent fallback are explicit. Shared context remains an empirical
unknown; synthetic acceptance does not prove isolated judgments or quality.

No reviewer or source-audit agent executed a candidate builder, accessed a key,
made a provider call or performed a new search in this design pass. These are
prospective design findings, not implementation qualification.
