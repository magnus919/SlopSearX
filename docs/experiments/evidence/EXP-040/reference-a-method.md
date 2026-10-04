# Reference A consistency pass

`reference-a-provisional.json` preserves the initial 133-row submission unchanged. `reference-a.json` is the revised independent annotation.

I rechecked every card against its visible title, URL, and snippet, in the stated query context. Lead scores reflect reading value for the purpose; visible scores reflect task-relevant substance actually present on the card. I did not treat secondary-source status as a reason to lower visibility, and did not infer page content from a citation or benchmark title. I reviewed every facet tag against its query's embedded vocabulary and only retained tags indicated by visible text. I treated reproducibility as a separate facet that needs visible reproducibility-related evidence, rather than assuming every benchmark or methods paper demonstrates it. Uncertainty remains flagged when the card offers only a weak or ambiguous indication.

Changes made during the pass:

- Cardiac: removed the unsupported `risk_factors` tag from `cardiac-r09` and `cardiac-r34`; changed `cardiac-r14` to a low-priority search-page lead; marked `cardiac-r26` irrelevant to cardiovascular prevention because its visible title is general rather than cardiovascular-specific.
- Research: added the retrieval facet to `research-r03`; changed `research-r14` from irrelevant to a partial RAG evaluation lead, with citation-only visibility and uncertainty; raised visible detail for `research-r18`, `r19`, `r23`, `r25`–`r27`, `r29`, and `r35` where snippets describe workflows, pipeline steps, or system behavior; lowered `research-r28` to topic-only visibility; removed unsupported `evidence_grounding` tags from `r09`, `r22`, and `r39`; removed the inferred orchestration tag from `r32`; added retrieval to `r41` based on its explicit RAG/document-ingestion description.
- Evaluation: removed unsupported reproducibility tags from `evaluation-r04`, `r21`, `r27`, `r34`, `r40`, and `r43`, and removed unsupported limitations/reproducibility tags from `r05`; tagged `r05` limitations because its excerpt explicitly identifies sampling risks. Raised visibility for `r09`, `r12`, and `r15` to reflect visible task/metric details. Set `r21` to citation/topic-only visibility and removed its unsupported reproducibility tag. Added measurement facets where benchmark/evaluation wording visibly indicates measurement (`r07`, `r23`, `r27`). Added software-task facet and retained measurement for `r40` based on its visible cross-file programming task text.

Final coverage is 44 cardiac, 45 research, and 44 evaluation cards. Final SHA-256 for `reference-a.json`: `9febf01af349700dfa17f6fbb7b648f68815b36cde2a7d8d56f214d0ed222b10`.

Final packaging correction: removed the `agent_orchestration` facet from `research-r10` because its visible card names a general biomedical AI agent but does not describe an orchestration method. Grades and uncertainty remain unchanged. The immediately preceding revised JSON is preserved as `reference-a-revision1.json`.
