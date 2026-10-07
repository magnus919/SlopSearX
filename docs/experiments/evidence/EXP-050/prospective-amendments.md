# Prospective corrections before acquisition

Original registration: PR #557, commit `5e8dbb574c27717c4321f58532cb3584275c47fa`, merged as `f3c7a12842ac1bbc6bf81dc8f549d34fab803c1d`. The original registration-integrity receipt is historical and binds files at that commit; it must not be applied to amended current files.

A concurrent workflow used EXP-049 for snapshot corruption. The identifier-metadata study will use EXP-050. This is an administrative rename; snapshot work is unrelated. Legacy `exp049-*` schema identifiers remain stable implementation labels.

Before any lookup, offline review of the pinned official API schema found an `externalIds` example with integer `CorpusId`. The original blanket string/null check would reject a legitimate provider response. The amended contract checks string/null types only for the four documented matching namespaces DOI, ArXiv, PubMed and PubMedCentral; other namespaces are bounded but ignored and never projected or used to match. This correction is made prospectively, before any observed provider result.

The twelve pools, all 423 cards, query-selection rules, batch membership, requested fields, HTTP budget, clipping, identity conflicts, information-coverage thresholds and no-quality-claim boundary remain unchanged. No metadata, Brave or Jev request preceded these corrections. Offline qualification and a new current publication-integrity receipt bind amended artifacts before acquisition.
