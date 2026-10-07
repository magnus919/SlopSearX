# EXP-039 reviewer B reference qualification

## How the labels were produced

I read the frozen purpose text and facet vocabulary first, then reviewed each of the 295 shuffled cards using only its displayed title, URL, and snippet. For each card, I manually assigned the two integer grades and the visibly supported facet IDs. The per-card assignments were entered as explicit rows grouped by query and expanded into JSON by a small serialization script. The script did not score by keyword, snippet length, URL, or any other classifier. No linked page was opened, and I made no judgment about whether a displayed claim is true or whether the unseen page supports it.

A shared rationale scaffold was used for initial serialization, with the card's title, a bounded excerpt of its snippet, and its assigned facets included for traceability. This means the scores and facet choices were individually authored judgments, while most rationale sentences were not individually composed. I manually rewrote rationales for the audit cases and corrected mismatched scores or facets found during the card-level audit. I then checked each row against its card, revising 80 of 295 rows across q1–q9 where task fit, visible detail, or a facet assignment did not match the displayed evidence. The original pre-audit output is retained separately as `reference-b-provisional.json`; the current `reference-b.json` is the corrected output. The reference remains an assistant judgment set, not human gold or an adjudicated consensus.

## Ten-card semantic audit

The quotations below reproduce the visible snippet text for each audited card. Grades describe only source-selection value for the stated task (lead) and task-relevant information visible on the card (visible).

1. **q6-r04 — lead 3, visible 1; facet `semantic_conventions`.** The task asks for official OpenTelemetry semantic-convention documentation. The OpenTelemetry page title and URL make it a direct official lead even though it is a move/index notice with little convention detail visible. This is the intended high-lead/low-visible case.

   > For AI agents: a documentation index is available at /llms.txt. This page has a Markdown version at /docs/specs/semconv/gen-ai/gen-ai-spans/index.md. View Markdown View page source Edit this page Create child page Create documentation issue Create project issue ... GenAI semantic conventions have moved to the OpenTelemetry GenAI semantic conventions repository.

2. **q1-r01 — lead 3, visible 2; facets `benchmarks`, `coordination`, `conflict_resolution`.** The literature-review task explicitly seeks empirical benchmarks of software-agent coordination and conflict. The card identifies CooperBench as a collaborative-coding benchmark and gives task count, language/library coverage, and a conflict setup. That is strong reading value with meaningful but incomplete detail; asynchronous work is not visible here.

   > To test this hypothesis, we introduce <strong>CooperBench, a benchmark of over 600 collaborative coding tasks across 12 libraries in 4 programming languages</strong>. Each task assigns two agents different features that can be implemented independently but may conflict without proper coordination.

3. **q7-r00 — lead 2, visible 2; facets `support`, `evaluation`.** The task concerns scientific claims checked against cited passages. A biomedical claim-level verification framework is a useful partial methodological lead, and the snippet explicitly names unsupported and contradictory claims. It is biomedical RAG rather than a clearly stated scientific citation-alignment benchmark, so the lead is partial.

   > Biomedical retrieval-augmented generation (RAG) can ground LLM answers in medical literature, yet long-form outputs often contain isolated unsupported or contradictory claims with safety implications. We introduce MedRAGChecker, a claim-level verification and diagnostic framework for biomedical RAG.… | By: Yuelyu Ji, Min Gu Kwak, Hang Zhang et al. | Cited by: 1 | arXiv: 2601.06519

4. **q1-r10 — lead 1, visible 1; facet `coordination`.** The card is about multi-agent coordination, but its stated application is household/entity resolution rather than software-engineering collaboration. That makes it broad background at most. The coordination facet is visible in the snippet; the detailed domain description does not provide task-relevant substance about coding-agent collaboration.

   > Entity resolution in real-world datasets remains a persistent challenge, particularly for identifying households and detecting co-residence patterns within noisy and incomplete data. While Large Language Models (LLMs) show promise, monolithic approaches often suffer from limited scalability and interpretability. This study introduces a multi-agent Retrieval-Augmented Generation (RAG) framework that decomposes household entity resolution into coordinated, task-specialized agents implemented using

5. **q2-r22 — lead 2, visible 1; facet `audit_logging`.** The enterprise-governance guide is a useful partial lead because it mentions audit logging and agentic systems. The shown example focuses on sanctioned consumer AI and unsanctioned-tool risk, not the requested human approval, audit, and policy controls for enterprise agents, so card-visible detail remains limited.

   > The critical component is providing sanctioned alternatives: when employees have access to an enterprise-licensed ChatGPT instance with data retention controls and audit logging, the incentive to use a personal account disappears. Every unsanctioned tool that goes undetected represents a gap in the human risk profile that legacy governance frameworks were never instrumented to measure. Agentic AI systems are qualitatively different from the predictive and generative models that existing governance frameworks address.

6. **q5-r15 — lead 2, visible 2; facet `attestations`.** The task seeks SLSA verifier/specification material and analyses of provenance attestations. Kettle is a useful adjacent technical source because the snippet explains what its attested provenance records and how the attestation is bound. It does not identify itself as an SLSA verifier/specification, and the snippet does not establish the requested builder-identity threat analysis.

   > Kettle is an attested build system that produces cryptographically verifiable provenance for software built inside Trusted Execution Environments (TEEs). A Kettle build records the source commit, dependency set, toolchain, build environment, and output artifact digests in a provenance document produced inside a measured confidential VM. The SHA-256 digest of that document is committed to the TEE platform's attestation report-data field, so the hardware-signed attestation report is itself the sig

7. **q4-r29 — lead 0, visible 0; no facets.** This card is an EVA-spaceflight benchmark, not SWE-bench or a study of benchmark contamination, decontamination, live evaluation, or temporal splits. The snippet's discussion of autonomy and communication latency is outside the evaluation brief. The previous draft's high grade was a card-to-row assignment error found in this audit and has been corrected.

   > Future exploration EVA operations, especially Mars surface EVAs and some higher-tempo Artemis scenarios, will require greater crew autonomy than the International Space Station paradigm because of communication latency, limited bandwidth, and increased operational complexity. Although large language models (LLMs) and agentic AI systems show promise as onboard decision-support tools, their suitability for safety-critical EVA operations remains unclear due to the lack of domain-specific evaluation

8. **q3-r07 — lead 3, visible 1; facet `idempotency`.** The task requests technical documentation on idempotency and safe recovery. The AWS developer-guide title directly identifies an official documentation lead for idempotency/retries. The snippet itself is only a generic SDK description, so it exposes no substantive recovery detail. The previous draft over-tagged external side effects and undergraded the title as a lead; both have been corrected.

   > Technical documentation for the AWS <strong>Durable</strong> <strong>Execution</strong> SDKs.

9. **q9-r25 — lead 0, visible 0; no facets.** The climate reading-list task seeks research across impacts, adaptation, and mitigation. The visible GitHub card explicitly identifies itself as a personal biography with no climate research evidence, so neither the source nor snippet is relevant. The previous draft's nonzero grade and impacts tag were mismatched to this card and have been corrected.

   > [Public personal biography omitted; this card provides no climate research evidence.]

10. **q9-r33 — lead 3, visible 3; facets `impacts`, `adaptation`, `mitigation`.** The task seeks reviews and primary studies across all three climate topics. This report's title directly spans them, and its snippet describes agricultural impacts, adaptation responses, and greenhouse-gas mitigation while outlining related research. It is a strong reading lead with substantial task-relevant information visible.

   > Climate change is likely to have significant impacts on the agricultural sector to which farmers will have to adapt. While agriculture is a significant contributor to greenhouse gas emissions, it is also a source of carbon storage in soils. This report examines the economic and policy issues related to the impacts of climate change on agriculture and adaptation responses and to the mitigation of greenhouse gases from agriculture. It outlines research undertaken and underway in other national and

## Audit outcome

The ten examples demonstrate the distinction between lead priority and visible detail, including direct official leads with sparse snippets and irrelevant cards that should not receive relevance credit. The all-row pass also corrected additional mismatches throughout q1–q9 before any model call. These checks do not establish page contents, factual correctness, or human-gold quality.

## Post-qualification facet visibility pass

After preserving `reference-b.json` as `reference-b-revision1.json`, I performed a focused pass over every assigned facet in all 295 cards. I retained a facet only when the visible title, URL, or snippet provided a direct signal for it; topical similarity to the task alone was not enough. This changed facet assignments on 70 rows, including both removals of unsupported tags and additions where a displayed phrase had been missed. I did not change any lead grade during this pass. One visible-detail grade changed: q1-r26 moved from 1 to 2 because its snippet contains planning and benchmark examples and explicitly says planning can reduce conflicts; the lead stayed 1.

The root shared these four findings from its 18-card random spot-check, which I corrected:

- **q2-r01:** Removed `human_approval`. Its visible card describes runtime monitoring and policy enforcement, without a human-approval mechanism.
- **q6-r11:** Removed `tool_calls`. Its snippet discusses semantic conventions, spans, and attributes for prompts/completions/tokens, without tool-call spans.
- **q5-r05:** Removed `attestations`. Its title mentions SLSA implementation, but the visible abstract only gives general CI/CD supply-chain context.
- **q1-r26:** Raised `visible` to 2. The snippet contains substantive planning, benchmark, and conflict-reduction information, though it remains broad background rather than a direct empirical coding-agent study.

The root's 18-card random spot-check comprised the four findings above plus q1-r03, q2-r12, q3-r16, q3-r03, q4-r18, q4-r34, q5-r24, q6-r29, q7-r19, q7-r16, q8-r23, q8-r09, q9-r15, and q9-r06. The root reported no additional material errors in those fourteen cards; borderline grades and uncertainty flags remain reviewer judgments. My own all-card pass covered visible facet support, and I made no lead-grade changes to force agreement with another review. These checks are limited review evidence, not human gold or consensus.
