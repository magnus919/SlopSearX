#!/usr/bin/env python3
"""Create vault clippings, atoms, and molecule for evaluation-bias paper batch."""
import os
from datetime import date

VAULT = "/Users/magnus/Obsidian/Magnus v2"
TODAY = "2026-06-11"

def sanitize(title):
    """Sanitize title for filesystem."""
    return title.replace("/", "-").replace("\\", "-").replace(":", " - ").replace('"', "'").replace("?", "").replace("<", "(").replace(">", ")").replace("|", "or")

def write_file(path, content):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as f:
        f.write(content)
    print(f"  Wrote: {path}")

# ============================================================
# PAPER 1: RealMath-Eval
# ============================================================
P1_CLIP = "RealMath-Eval - Why SOTA Judges Struggle with Real Human Reasoning"
P1_AUTHORS="Yiteng Mao, Kenan Xu, Yijia Lyu, Wenhao Li, Jianlong Chen, Xiangfeng Wang"

clipping_1 = f"""---
title: "{P1_CLIP}"
source: "https://arxiv.org/abs/2606.10254"
author: "{P1_AUTHORS}"
published: "2026-06-08"
created: "{TODAY}"
description: "Introduces RealMath-Eval, a benchmark of 224 real-world high school exam responses. SOTA LLM judges achieve MSE ~2.96 vs expert grading on real human answers but MSE ~1.17 on synthetic LLM-generated solutions, revealing a systematic Evaluation Gap. Synthetic errors exhibit structural collapse into low-dimensional subspaces while human errors occupy more diverse space. Surface-level style transfer fails to close the gap."
tags:
  - clippings
topics:
  - "[[LLM Evaluation]]"
  - "[[Benchmarking]]"
  - "[[Mathematical Reasoning]]"
  - "[[Evaluation Bias]]"
related:
  - "[[Who Brought Easter Eggs to Eid - Auditing Cultural Translation of Math Word Problems]]"
  - "[[The Shibboleth Effect - Auditing the Cross-Lingual Distributional Skew of LLMs]]"
  - "[[Who Gets Flagged - The Pluralistic Evaluation Gap in AI Content Watermarking]]"
publish: true
---

# RealMath-Eval: Why SOTA Judges Struggle with Real Human Reasoning

**Authors:** {P1_AUTHORS}
**arXiv:** 2606.10254
**Date:** 2026-06-08

## Summary

RealMath-Eval is a rigorously annotated benchmark of 224 real-world exam responses from high schools, designed to evaluate how well LLM judges assess authentic human mathematical reasoning — not just synthetic or textbook solutions.

## Key Findings

1. **The Evaluation Gap:** SOTA LLM judges achieve MSE ~2.96 against expert human grading on real student responses, but only MSE ~1.17 on synthetic LLM-generated solutions — a nearly 2.5x error difference.

2. **Structural Collapse in Synthetic Errors:** Through semantic embedding analysis, synthetic errors exhibit "structural collapse" into predictable, low-dimensional linear subspaces, while human errors form a more diverse error space.

3. **Higher Surprisal in Human Reasoning:** Generative probability probes reveal that human reasoning involves significantly higher information-theoretic surprisal, indicating student reasoning transitions are more out-of-distribution for current models.

4. **Style Transfer Failure:** Surface-level style transfer (making synthetic solutions mimic human writing style) fails to close the evaluation gap, suggesting the gap stems from deeper reasoning structure differences rather than surface presentation.

## Implications

The findings suggest that current LLM evaluation pipelines relying heavily on synthetic data may not adequately capture the diversity of authentic student mathematical reasoning. This has direct implications for automated grading systems, educational technology, and the validity of LLM-as-judge benchmarks.
"""

# ============================================================
# PAPER 2: Easter Eggs to Eid
# ============================================================
P2_CLIP = "Who Brought Easter Eggs to Eid - Auditing Cultural Translation of Math Word Problems"
P2_AUTHORS = "Parisa Suchdev, Juniper Lovato"

clipping_2 = f"""---
title: "{P2_CLIP}"
source: "https://arxiv.org/abs/2606.11009"
author: "{P2_AUTHORS}"
published: "2026-06-09"
created: "{TODAY}"
description: "Audits how Claude Opus 4, GPT-4.1, and Gemini 2.5 Pro culturally adapt 60 English math word problems into 7 languages (Bengali, Hindi, Punjabi, Urdu, Sindhi, Italian, Sicilian). Annotates 6,489 entity transformations across 76 entity types. Models agree on transformation type in 62.5% of cases but specific substitutions in only 33.5%. All 21 language-model combinations show entropy collapse — adaptation paradoxically compresses cultural diversity. Models prioritize surface markers (names, foods, currencies) while preserving structural features that embed culturally specific assumptions. Documents cross-regional misattribution (Bangladeshi taka for Indian Bengali students) and cross-cultural contamination (Easter egg hunts reframed as Eid activities)."
tags:
  - clippings
topics:
  - "[[Cultural Bias]]"
  - "[[LLM Evaluation]]"
  - "[[Cross-Lingual Bias]]"
  - "[[Mathematical Reasoning]]"
  - "[[Evaluation Bias]]"
related:
  - "[[RealMath-Eval - Why SOTA Judges Struggle with Real Human Reasoning]]"
  - "[[The Shibboleth Effect - Auditing the Cross-Lingual Distributional Skew of LLMs]]"
  - "[[Who Gets Flagged - The Pluralistic Evaluation Gap in AI Content Watermarking]]"
publish: true
---

# Who Brought Easter Eggs to Eid? Auditing Cultural Translation of Math Word Problems

**Authors:** {P2_AUTHORS}
**arXiv:** 2606.11009
**Date:** 2026-06-09

## Summary

This paper audits how three frontier LLMs culturally adapt 60 English math word problems (from GSM8K) into seven target languages across three regions, producing 1,260 translated problems and an entity-level dataset of 6,489 transformations.

## Key Findings

1. **Cross-Model Instability (RQ1):** Models agree on transformation type in 62.5% of cases but on specific substitutions in only 33.5%. Claude and GPT are most similar (77% action agreement); Gemini and GPT least similar (71.5%). Model choice is a cultural decision, not merely a technical one.

2. **Entropy Collapse (RQ2):** All 21 language-model combinations show negative entropy differences (0.12-0.37 bits reduction). Person Names collapse most severely (2.64 bits). Claude localizes most aggressively; GPT preserves most conservatively; Gemini shows highest type-change rates.

3. **Surface vs Deep Culture (RQ3):** Models focus on surface markers (names, foods, currencies — localization rates 0.78-1.00) while preserving structural features like grade-level designations that embed culturally specific assumptions. Vehicle parts, meal types, and plants are never localized.

4. **Cross-Regional Misattribution:** Despite prompts specifying India, models used Bangladeshi taka in 76.2% of Bengali currency instances. Hindi translations used Indian rupees 100% correctly. Easter egg hunts were reframed as "egg search competition on Eid" — substituting holiday name while preserving activity structure.

5. **Geographic Clustering:** Adaptation patterns cluster by geographic region (Sindhi-Urdu JSD=0.18; Italian-Sicilian JSD=0.23), suggesting models internalize region-specific cultural schemas.

## Implications

The surface plausibility that makes adapted problems look correct is precisely what makes deeper failures easy to overlook. LLM-generated adaptation creates an automation irony: easier to produce, harder to review.
"""

# ============================================================
# PAPER 3: Shibboleth Effect
# ============================================================
P3_CLIP = "The Shibboleth Effect - Auditing the Cross-Lingual Distributional Skew of LLMs"
P3_AUTHORS = "Hakan Mehmetcik"

clipping_3 = f"""---
title: "{P3_CLIP}"
source: "https://arxiv.org/abs/2606.11082"
author: "{P3_AUTHORS}"
published: "2026-06-09"
created: "{TODAY}"
description: "Demonstrates cross-lingual distributional skew (Shibboleth Effect) in frontier LLMs via a multi-agent geopolitical wargame (Cerulean Sea Crisis). Tests 6 models (GPT-4o, Llama-4, Mistral-Large, Gemini-3.1-Pro, Qwen3.6-Plus, DeepSeek-R1) across English vs Turkish simulations. Llama-4 shows +0.800 coercive rhetoric increase under Turkish; Gemini-3.1-Pro shows -0.750 decrease; DeepSeek-R1 exhibits -0.860 decrease with chain-of-thought buffering evidence; GPT-4o shows null effect. Identifies two buffering mechanisms: CoT institutional anchoring and multilingual RLHF alignment."
tags:
  - clippings
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[LLM Safety]]"
  - "[[Geopolitical AI]]"
  - "[[Evaluation Bias]]"
  - "[[DeepSeek-R1]]"
  - "[[Llama-4]]"
  - "[[GPT-4o]]"
related:
  - "[[RealMath-Eval - Why SOTA Judges Struggle with Real Human Reasoning]]"
  - "[[Who Brought Easter Eggs to Eid - Auditing Cultural Translation of Math Word Problems]]"
  - "[[Who Gets Flagged - The Pluralistic Evaluation Gap in AI Content Watermarking]]"
publish: true
---

# The Shibboleth Effect: Auditing the Cross-Lingual Distributional Skew of LLMs

**Author:** {P3_AUTHORS}
**arXiv:** 2606.11082
**Date:** 2026-06-09

## Summary

This research demonstrates cross-lingual distributional skew (the Shibboleth Effect) in frontier LLMs by engineering a multi-agent geopolitical wargame — the Cerulean Sea Crisis — a synthetic maritime territorial dispute structurally isomorphic to Eastern Mediterranean conflicts.

## Key Findings

1. **Heterogeneous Effects:** The Shibboleth Effect is architecture- and training-regime-contingent, not a universal property of Western-origin models. Llama-4 shows +0.800 coercive rhetoric increase under Turkish (p=.002); Gemini-3.1-Pro shows -0.750 decrease (p=.005); DeepSeek-R1 shows -0.860 decrease (p=.006); GPT-4o is effectively null (δ=+0.130, p=.614).

2. **Buffering Mechanisms Identified:** Two distinct mechanisms mitigate cross-lingual skew: (a) chain-of-thought institutional anchoring — DeepSeek-R1's explicit reasoning process buffers against language-driven shifts, and (b) multilingual RLHF alignment — Gemini-3.1-Pro's multilingual training reverses rather than merely attenuates the effect.

3. **Inter-Model Variance Within Western Class:** GPT-4o, Llama-4, Mistral-Large, and Gemini-3.1-Pro — all broadly classified as Western-origin — exhibit radically different and directionally inconsistent responses to the Turkish language anchor.

4. **Methodology Innovation:** Multi-agent wargaming with synthetic statecraft masking (SHA-256 verified briefings) provides a high-fidelity alternative to static Q&A benchmarks for measuring latent ideological skew.

## Implications

If a model's geopolitical guardrails shift as a function of operating language, such systems may function less as neutral mediators than as algorithmic echo chambers, amplifying rather than attenuating international security dilemmas. The finding that GPT-4o is null while Llama-4 shows a large effect, despite both being English-primary models, challenges the assumption that alignment is uniform across models.
"""

# ============================================================
# PAPER 4: Who Gets Flagged
# ============================================================
P4_CLIP = "Who Gets Flagged - The Pluralistic Evaluation Gap in AI Content Watermarking"
P4_AUTHORS = "Alexander Nemecek, Osama Zafar, Yuqiao Xu, Wenbiao Li, Erman Ayday"

clipping_4 = f"""---
title: "{P4_CLIP}"
source: "https://arxiv.org/abs/2604.13776"
author: "{P4_AUTHORS}"
published: "2026-04-15"
created: "{TODAY}"
description: "Examines how watermarking detection varies across languages, cultural visual traditions, and demographic groups. Reviews major watermarking benchmarks across text, image, audio, and video modalities — finding that with one exception (AudioMarkBench), none report performance across languages, cultural content types, or population groups. Proposes three evaluation dimensions: cross-lingual detection parity, culturally diverse content coverage, and demographic disaggregation of detection metrics. Argues watermarking is part of the pluralistic alignment pipeline and should be held to the same evaluation standards as generative models."
tags:
  - clippings
topics:
  - "[[Watermarking]]"
  - "[[AI Safety]]"
  - "[[Evaluation Bias]]"
  - "[[Cross-Lingual Bias]]"
related:
  - "[[RealMath-Eval - Why SOTA Judges Struggle with Real Human Reasoning]]"
  - "[[Who Brought Easter Eggs to Eid - Auditing Cultural Translation of Math Word Problems]]"
  - "[[The Shibboleth Effect - Auditing the Cross-Lingual Distributional Skew of LLMs]]"
publish: true
---

# Who Gets Flagged? The Pluralistic Evaluation Gap in AI Content Watermarking

**Authors:** {P4_AUTHORS}
**arXiv:** 2604.13776v2 (updated 2026-06-09)
**Date:** 2026-04-15

## Summary

This paper argues that watermarking, increasingly mandated as the verification layer for AI-generated content, carries the same potential for disparate impact as the generative systems it is meant to authenticate. Content-dependent embedding produces content-dependent outcomes, and content varies systematically across languages, cultures, and communities.

## Key Findings

1. **Evaluation Vacuum:** All major watermarking benchmarks — MarkMyWords, WaterBench, WaterPark (text); WAVES, W-Bench (image); VideoMarkBench (video) — evaluate exclusively on English/Western content and report no demographic disaggregation. Only AudioMarkBench tests across 25 languages and stratifies by sex/age.

2. **Text Modality Bias:** Non-native English speakers' essays carry stronger watermark signals than native speakers' under identical AI assistance, producing higher false positive rates. Tokenizers trained on English-heavy corpora fragment non-Latin scripts into more tokens, altering detection statistics. Cross-lingual translation removes watermark detectability entirely.

3. **Image Modality Risk:** No published work evaluates image watermark detection by cultural content or subject demographics. JPEG compression degrades face recognition on darker skin tones by up to 34%. Non-Western compression codecs (WeChat, LINE) are untested.

4. **Audio Modality Evidence:** AudioMarkBench found female speech exhibits higher false positive rates than male speech under certain perturbations. Tonal languages (Mandarin, Vietnamese, Yoruba) have fundamentally different acoustic profiles that could interact with frequency-domain watermarking.

5. **Governance Gap:** EU AI Act, US Executive Order 14110, and China's AI labeling measures all mandate watermarking without requiring fairness evaluation. The EU AI Act requires bias evaluation for high-risk AI systems yet exempts watermarking.

## Proposed Framework

Three evaluation dimensions: (1) Cross-lingual detection parity, (2) Culturally diverse content coverage, (3) Demographic disaggregation of detection metrics. Pluralistic alignment is incomplete without extending scrutiny to the verification layer.
"""

# ============================================================
# ATOMS — 8 per paper = 32 atoms
# ============================================================
atoms = []

# === PAPER 1: RealMath-Eval (8 atoms) ===
atoms.append(f"""---
title: "LLM Judges Exhibit 2.5x Higher Error on Real vs Synthetic Math Responses"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Benchmarking]]"
  - "[[Evaluation Bias]]"
summary: "SOTA LLM judges achieve MSE ~2.96 on real human exam responses but MSE ~1.17 on synthetic LLM solutions — a 2.5x error gap."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

State-of-the-art LLM judges evaluated on the RealMath-Eval benchmark (224 real-world high school exam responses) exhibit Mean Squared Error of approximately 2.96 against expert human grading. On a matched control set of synthetic LLM-generated solutions, the same judges achieve MSE of approximately 1.17 — nearly 2.5 times more accurate. This Evaluation Gap persists across model architectures and judgment methodologies.
""")

atoms.append(f"""---
title: "Synthetic Math Errors Exhibit Structural Collapse Into Low-Dimensional Subspaces"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Benchmarking]]"
summary: "Semantic embedding analysis reveals synthetic LLM errors collapse into predictable low-dimensional linear subspaces, while human errors occupy more diverse representational space."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

Through semantic embedding analysis of error patterns, synthetic LLM-generated solutions exhibit a phenomenon termed "structural collapse": their errors project into predictable, low-dimensional linear subspaces. Human student errors, by contrast, form a more diverse error space distributed across higher-dimensional representations. This suggests synthetic benchmarks systematically underestimate the complexity of authentic human reasoning evaluation.
""")

atoms.append(f"""---
title: "Human Math Reasoning Shows Higher Information-Theoretic Surprisal Than LLM Outputs"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Mathematical Reasoning]]"
summary: "Generative probability probes show human reasoning involves significantly higher information-theoretic surprisal than LLM-generated solutions, indicating student reasoning transitions are more out-of-distribution for current models."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

Using generative probability probes, researchers measured the information-theoretic surprisal of reasoning transitions in both human and synthetic solutions. Human reasoning exhibited significantly higher surprisal values, indicating that the step-by-step transitions in authentic student thinking are more diverse and less predictable. This out-of-distribution character of human reasoning likely contributes to LLM judges' difficulty in evaluating real student work.
""")

atoms.append(f"""---
title: "Surface-Level Style Transfer Cannot Close the LLM Evaluation Gap for Math"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Benchmarking]]"
summary: "Applying surface-level style transfer to make synthetic solutions mimic human writing style fails to improve LLM judge accuracy, confirming the evaluation gap stems from deeper reasoning structure differences."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

Researchers tested whether the Evaluation Gap could be closed by applying surface-level style transfer — rewriting synthetic solutions to mimic human writing patterns including typos, informal phrasing, and nonlinear presentation. This intervention failed to improve LLM judge accuracy on the transformed synthetic solutions, confirming that the gap derives from differences in reasoning structure and content rather than surface stylistic features.
""")

atoms.append(f"""---
title: "RealMath-Eval Benchmark Contains 224 Diverse Real Exam Responses"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[Benchmarking]]"
  - "[[LLM Evaluation]]"
summary: "RealMath-Eval provides 224 real-world high school exam responses with expert human grading annotations, spanning diverse problem types and student approaches."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

RealMath-Eval is a rigorously annotated benchmark consisting of 224 real-world exam responses collected from high schools. Each response has been graded by expert human evaluators, providing ground-truth labels. The benchmark spans diverse mathematical problem types and captures natural variation in student solution strategies, including correct but unconventional approaches.
""")

atoms.append(f"""---
title: "Current LLM-as-Judge Pipelines Over-rely on Synthetic Evaluation Data"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Evaluation Bias]]"
summary: "The RealMath-Eval findings suggest current LLM evaluation pipelines that rely heavily on synthetic data may not adequately capture the diversity of authentic human reasoning."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

The finding that LLM judges perform dramatically worse on real human responses than on synthetic ones has direct implications for the validity of LLM-as-judge evaluations. Current benchmarks predominantly use synthetic or curated data, which may systematically overestimate judge performance. Real-world deployment of automated grading and evaluation systems faces risks from this unmeasured gap.
""")

atoms.append(f"""---
title: "LLM Judges Near-Perfect at Solving but Struggle to Evaluate Diverse Human Reasoning"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Mathematical Reasoning]]"
summary: "While LLMs achieve near-perfect performance solving high-school mathematics, their ability to evaluate diverse human reasoning processes remains significantly deficient."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

There is a stark asymmetry between LLMs' mathematical problem-solving capability and their evaluation capability. While frontier models achieve near-perfect scores on solving high-school mathematics problems, their performance as judges of other solvers' work lags far behind. This suggests that the skills required for evaluation — recognizing diverse valid approaches, distinguishing genuine errors from unconventional but correct reasoning — are distinct from and harder than the skills required for direct problem-solving.
""")

atoms.append(f"""---
title: "RealMath-Eval Uses Expert Human Grading as Ground Truth Against LLM Judges"
created: "{TODAY}"
type: atom
source: "arXiv:2606.10254"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLM Evaluation]]"
  - "[[Benchmarking]]"
summary: "The RealMath-Eval benchmark uses expert human grading as the ground-truth standard, enabling direct comparison between LLM judge performance and human-level evaluation."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

Each of the 224 exam responses in RealMath-Eval was independently graded by expert human evaluators, establishing a rigorous ground-truth standard. This allows direct measurement of LLM judge deviation from human expertise. The MSE metric captures both the magnitude and direction of disagreement, providing a more nuanced picture of judge performance than accuracy or agreement rates alone.
""")

# === PAPER 2: Easter Eggs to Eid (8 atoms) ===
atoms.append(f"""---
title: "Only 33.5% Cross-Model Agreement on Specific Cultural Substitutions in Math Problems"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
  - "[[Evaluation Bias]]"
summary: "Three frontier LLMs agree on transformation type in 62.5% of cases but specific cultural substitutions in only 33.5% — model choice is a cultural decision, not merely a technical one."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

When three frontier LLMs (Claude Opus 4, GPT-4.1, Gemini 2.5 Pro) culturally adapt 60 English math word problems into 7 languages, they agree on transformation type in 62.5% of cases but on specific substitutions in only 33.5%. Among cases where models agree on an action, they agree on output value only 53.7% of the time. Claude and GPT are most similar (77% action agreement); Gemini and GPT least similar (71.5%). This means an educator switching models is also changing the cultural world students encounter.
""")

atoms.append(f"""---
title: "Cultural Adaptation Compresses Rather Than Expands Diversity of Entity Values"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "All 21 language-model combinations show entropy collapse (0.12-0.37 bits reduction), with adaptation paradoxically compressing cultural diversity toward fewer canonical substitutions."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Shannon entropy analysis across all 21 language-model combinations reveals universal negative entropy differences, indicating adaptation compresses cultural diversity. Reductions range from 0.12 bits (Sicilian with Gemini) to 0.37 bits (Urdu and Hindi with Gemini). Person Names collapse most severely at 2.64 bits. Pakistani languages show deepest collapse; European languages show smallest reductions. Models individually draw from a narrow pool even while disagreeing with each other.
""")

atoms.append(f"""---
title: "LLMs Prioritize Surface Cultural Markers While Preserving Deep Structural Assumptions"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "Models localize surface markers (names 0.916, foods 0.786, currencies 0.943) while preserving structural features like grade-level systems that embed culturally specific assumptions."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Entity-level analysis reveals a consistent pattern: models aggressively localize surface-level cultural markers — personal names (adaptation balance 0.916), currency names (0.943), food items (0.786) — while universally preserving deeper structural features. Grade-level designations ("third-graders") are preserved at 100%, even though Italian students attend "terza elementare," Pakistani students "Class 3," and Indian students "Standard 3." This matches Hammond's Culture Tree framework distinguishing surface culture from deep culture.
""")

atoms.append(f"""---
title: "LLMs Produce Cross-Regional Misattribution in Cultural Adaptation of Math Problems"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "Models used Bangladeshi taka in 76.2% of Bengali currency instances despite prompts specifying India, and reframed Easter egg hunts as Eid activities."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Despite prompts explicitly specifying "an elementary school math teacher in India, teaching students in Bengali," models used the Bangladeshi taka in 76.2% of Bengali currency instances (77 of 101). Hindi translations correctly used Indian rupees 100% of the time. In broader cultural substitutions, models replaced an Easter egg hunt with an "egg search competition on Eid" for Urdu and Sindhi — substituting the holiday name while preserving the activity structure, revealing pattern-matching without cultural knowledge.
""")

atoms.append(f"""---
title: "LLM Adaptation Patterns Cluster by Geographic Region Not Language Boundaries"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "Jensen-Shannon divergence analysis shows adaptation patterns cluster by geographic region — Sindhi-Urdu JSD=0.18, Italian-Sicilian JSD=0.23 — suggesting models have internalized region-specific cultural schemas."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Cross-linguistic similarity analysis using Jensen-Shannon divergence reveals that adaptation strategies cluster by geographic region rather than treating each language independently. Italian and Sicilian show nearly identical patterns (JSD = 0.23). Sindhi and Urdu are the most similar pair (JSD = 0.18). Bengali, Hindi, and Punjabi form a tight cluster (JSD = 0.23-0.25). Cross-regional comparisons show substantially higher divergence (0.30-0.39 between European and South Asian languages).
""")

atoms.append(f"""---
title: "6,489 Entity Transformations Annotated Across 76 Cultural Entity Types"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "The study produces 6,489 annotated entity transformations spanning 76 fine-grained types in 26 super-categories, with five action labels: preserved, localized, generalized, type changed, missing."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

The entity-level dataset encodes 6,489 source-target entity alignments across seven languages. The annotation schema covers 76 fine-grained entity types grouped into 26 super-categories including Personal Names, Food & Drink Items, Money & Currency Systems, Arts Performance & Media, and Religious Practices & Places. Each transformation receives one of five action labels. Inter-annotator reliability is high (Cohen's κ = 0.859, 95% CI [0.831, 0.886]).
""")

atoms.append(f"""---
title: "Models Show Stable Individual Tendencies Across All Seven Target Languages"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Cross-Lingual Bias]]"
summary: "Claude localizes most aggressively, GPT preserves most conservatively, and Gemini shows highest type-change rates — patterns consistent across all seven target languages."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Each model exhibits a stable cultural adaptation signature that persists across all seven target languages. Claude Opus 4 localizes most aggressively, systematically replacing source entities with local alternatives. GPT-4.1 preserves most conservatively, keeping more source entities unchanged. Gemini 2.5 Pro shows the highest rate of type-changed transformations, restructuring entity categories more frequently. These model-specific tendencies are independent of the target language, suggesting they stem from underlying training and alignment differences.
""")

atoms.append(f"""---
title: "Surface Plausibility Masks Deeper Cultural Adaptation Failures in LLM Outputs"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11009"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Cultural Bias]]"
  - "[[Evaluation Bias]]"
summary: "The surface plausibility of culturally adapted LLM outputs makes deeper failures — diversity collapse, regional misattribution — harder to detect, creating an automation irony."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

The paper identifies an automation irony in LLM-assisted cultural adaptation: adaptation becomes easier to generate while oversight becomes harder because outputs often look appropriate enough that scrutiny feels unnecessary. Some failures are visible in individual translations, but diversity collapse, systematic preference for surface markers, and consistent regional misattribution emerge only through corpus-level analysis. The surface plausibility that makes adapted problems look correct is precisely what makes deeper failures easy to overlook.
""")

# === PAPER 3: Shibboleth Effect (8 atoms) ===
atoms.append(f"""---
title: "Llama-4 Shows +0.800 Coercive Rhetoric Increase Under Turkish Geopolitical Wargame"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[LLM Safety]]"
  - "[[Geopolitical AI]]"
  - "[[Llama-4]]"
summary: "In a multi-agent geopolitical wargame, Llama-4 exhibits a large, Holm-corrected increase in coercive rhetoric when operating in Turkish versus English (δ=+0.800, p=.002)."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

When Llama-4 Maverick operated in Turkish rather than English during the Cerulean Sea Crisis wargame, its coercive rhetoric score increased by 0.800 standard deviations (Holm-corrected p=.002). This represents a large effect size indicating that the model's diplomatic behavior shifts substantially based solely on the language of interaction. The model becomes significantly more confrontational and less concessionary when processing the same geopolitical scenario in Turkish.
""")

atoms.append(f"""---
title: "DeepSeek-R1 and Gemini-3.1-Pro Show Directional Reversal Under Turkish Conditions"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[Geopolitical AI]]"
  - "[[DeepSeek-R1]]"
summary: "Both DeepSeek-R1 (δ=-0.860, p=.006) and Gemini-3.1-Pro (δ=-0.750, p=.005) exhibit large coercive rhetoric decreases under Turkish — the opposite direction from Llama-4."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

Contrary to expectations that Western-origin models would uniformly show increased coercion when operating in a non-Western language, DeepSeek-R1 and Gemini-3.1-Pro both exhibited large decreases in coercive rhetoric under Turkish conditions. DeepSeek-R1 showed the strongest reversal (δ=-0.860, p=.006) with direct chain-of-thought evidence for the buffering mechanism. Gemini-3.1-Pro showed a similarly large decrease (δ=-0.750, p=.005). This directional reversal demonstrates that the Shibboleth Effect is heterogeneous across architectures.
""")

atoms.append(f"""---
title: "GPT-4o Shows No Statistically Significant Cross-Lingual Geopolitical Skew"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[Geopolitical AI]]"
  - "[[GPT-4o]]"
summary: "GPT-4o yields a statistically null effect (δ=+0.130, p=.614) in the cross-lingual wargame, demonstrating that not all English-primary models exhibit the Shibboleth Effect."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

Among the six models tested, GPT-4o was the only Western-origin model to show no statistically significant cross-lingual behavioral shift. With an effect size of just +0.130 and p-value of .614, GPT-4o's geopolitical decision-making remained stable across English and Turkish conditions. This null result challenges the assumption that cross-lingual bias is a universal property of English-dominant models and suggests that OpenAI's training and alignment procedures may provide effective cross-lingual stability.
""")

atoms.append(f"""---
title: "Chain-of-Thought Institutional Anchoring Buffers Against Cross-Lingual Skew"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[LLM Safety]]"
  - "[[DeepSeek-R1]]"
summary: "DeepSeek-R1's chain-of-thought reasoning provides a buffering mechanism against cross-lingual distributional skew through institutional anchoring in explicit reasoning."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

DeepSeek-R1's chain-of-thought reasoning process served as a structural buffering mechanism against language-driven behavioral shifts. The model's explicit reasoning steps anchored its decision-making in institutional and diplomatic norms that transcended language-specific associations. This "CoT institutional anchoring" allowed the model to maintain consistent geopolitical reasoning frameworks even when operational language changed, providing direct evidence for a mechanism that mitigates the Shibboleth Effect.
""")

atoms.append(f"""---
title: "Multilingual RLHF Training Can Reverse Rather Than Merely Attenuate Cross-Lingual Bias"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[LLM Safety]]"
summary: "Gemini-3.1-Pro's sufficiently robust multilingual RLHF reversed the direction of cross-lingual behavioral skew, reducing rather than increasing coercion under Turkish."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

The finding that Gemini-3.1-Pro showed decreased coercion under Turkish (while Llama-4 showed increased coercion) demonstrates that multilingual RLHF alignment can produce qualitatively different outcomes than simply attenuating bias. Sufficiently robust multilingual training may counterbalance the English-dominant safety constraints that otherwise produce asymmetric behavior. This suggests multilingual RLHF quality is a critical design parameter for geopolitically deployed models.
""")

atoms.append(f"""---
title: "Multi-Agent Wargaming Reveals Latent Ideological Skew That Static Benchmarks Miss"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[Geopolitical AI]]"
summary: "Multi-agent geopolitical wargaming with synthetic statecraft masking reveals latent cross-lingual bias that static Q&A benchmarks and translated surveys fail to detect."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

Previous literature on cross-lingual bias relied on static, single-turn Q&A benchmarks or translated psychometric surveys, which capture models' "stated preferences" — sanitized summaries designed to evade safety filters. The multi-agent wargame methodology with synthetic statecraft masking (structurally isomorphic to real conflicts but preventing memorized retrieval) strips away superficial guardrails to reveal models' revealed preferences under adversarial friction. This approach detected large cross-lingual effects that static methods would have missed.
""")

atoms.append(f"""---
title: "Inter-Model Variance Within Western Model Class Challenges Monolithic Assumptions"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Cross-Lingual Bias]]"
  - "[[Evaluation Bias]]"
summary: "GPT-4o, Llama-4, Mistral-Large, and Gemini-3.1-Pro exhibit radically different and directionally inconsistent cross-lingual responses, challenging the assumption that 'Western models' form a monolithic class."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

Existing literature has treated "Western-origin models" as a monolithic class with presumed uniform characteristics. The Cerulean Sea Crisis experiment falsifies this assumption: four Western-origin models showed three distinct patterns — Llama-4 increased coercion, Gemini-3.1-Pro and DeepSeek-R1 decreased coercion, GPT-4o showed no effect. The pooled effect across the Western cluster is statistically indistinguishable from zero, meaning aggregate analysis would miss the significant individual effects.
""")

atoms.append(f"""---
title: "Cerulean Sea Crisis Wargame Uses SHA-256 Verified Synthetic Statecraft for Reproducibility"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11082"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[Geopolitical AI]]"
summary: "The Cerulean Sea Crisis wargame uses JSON-formatted briefings with SHA-256 hashing for integrity verification, enabling reproducible cross-lingual AI safety testing."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

The experimental platform — the Cerulean Sea Crisis — is a synthetic maritime territorial dispute structurally isomorphic to Eastern Mediterranean conflicts including Turkey's Mavi Vatan (Blue Homeland) doctrine. The environment uses JSON-formatted briefings with SHA-256 hashes logged with every simulation event to ensure integrity. Briefings were geopolitically tuned for each language (e.g., "Exclusive Economic Zone" → "Münhasır Ekonomik Bölge" in Turkish) with a strict Language Anchor directive injected into the system prompt to route token-generation through respective language strata.
""")

# === PAPER 4: Who Gets Flagged (8 atoms) ===
atoms.append(f"""---
title: "Major Watermark Benchmarks Fail to Evaluate Cross-Lingual or Demographic Fairness"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Evaluation Bias]]"
  - "[[AI Safety]]"
summary: "All major watermarking benchmarks across text, image, and video modalities evaluate exclusively on English/Western content with no demographic disaggregation — only AudioMarkBench tests across languages."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

A systematic review of major watermarking benchmarks reveals a consistent pluralistic evaluation gap. Text benchmarks (MarkMyWords, WaterBench, WaterPark), image benchmarks (WAVES, W-Bench), and video benchmarks (VideoMarkBench) all evaluate exclusively on English-language or Western-centric content. None report detection performance across languages, cultural content types, or demographic groups. Only AudioMarkBench (audio modality) tests across 25 languages and stratifies results by biological sex and age group.
""")

atoms.append(f"""---
title: "Non-Native English Speakers Face Higher False Positive Rates in Text Watermarking"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Cross-Lingual Bias]]"
  - "[[Evaluation Bias]]"
summary: "Under identical AI assistance guidelines, non-native English speakers' essays carry stronger watermark signals, producing higher false positive rates at standard detection thresholds."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Research by Xie demonstrates that under identical permitted AI assistance guidelines, essays by non-native English speakers carry stronger watermark signals than those by native speakers. AI systems make more extensive modifications to non-native text, producing more tokens under watermarked generation and encountering more high-entropy positions. The bias is not introduced by the detector but arises structurally from the interaction between the writer's linguistic background and the watermark embedding mechanism. Tokenizers trained on English-heavy corpora fragment non-Latin scripts into more subword units, compounding the disparity.
""")

atoms.append(f"""---
title: "Image Watermark Bias Untested Despite Known Demographic Asymmetries in JPEG Compression"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Evaluation Bias]]"
summary: "No published work evaluates image watermark detection by cultural content or subject demographics, despite JPEG compression degrading face recognition on darker skin by up to 34%."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

The primary image watermark robustness benchmark WAVES tests perturbations reflecting Western platform conventions. Lossy JPEG compression alone has been shown to disproportionately degrade face recognition performance on darker skin tones (up to 34% accuracy reduction) via chroma subsampling in the frequency domain. Non-Western compression codecs and processing pipelines common on platforms like WeChat and LINE are absent from evaluation. Visual content types outside ImageNet-derived distributions — calligraphic scripts, textile patterns — remain entirely untested.
""")

atoms.append(f"""---
title: "AudioMarkBench Found Female Speech Has Higher False Positive Rates in Watermark Detection"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Evaluation Bias]]"
summary: "The only audio watermark benchmark testing demographic factors found female speech exhibits higher false positive rates for watermark forgery than male speech under certain perturbations."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

AudioMarkBench, the sole exception to the pluralistic evaluation gap, evaluated watermark robustness across 25 languages and stratified results by biological sex and age group. The findings showed female speech exhibited higher false positive rates for watermark forgery than male speech under certain perturbations. Language-level variation was also detected, with Georgian and Esperanto showing anomalously different false negative rates depending on watermarking method and perturbation type. Tonal languages (Mandarin, Vietnamese, Yoruba) present acoustic profiles fundamentally different from English prosody that could interact with frequency-domain watermarking.
""")

atoms.append(f"""---
title: "Watermarking Is Part of the Pluralistic Alignment Pipeline Requiring Fairness Evaluation"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[AI Safety]]"
  - "[[Evaluation Bias]]"
summary: "The paper argues pluralistic alignment is incomplete without extending fairness scrutiny to the verification layer — a generative system can pass alignment evaluations while its watermarking produces disparate detection rates."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Sorensen et al.'s three operationalizations of pluralistic alignment (Overton, steerable, distributional) have been pursued almost exclusively at the level of model outputs. This paper argues this is insufficient: a generative system can pass a distributional pluralism evaluation while the watermarking layer that authenticates its outputs produces systematically different detection rates across the populations the alignment effort was meant to serve. This is the verification-layer analog of "pluralistic value-washing" — a superficially pluralistic pipeline whose binding component is monistic.
""")

atoms.append(f"""---
title: "Governance Frameworks Mandate Watermarking Without Requiring Fairness Evaluation"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[AI Safety]]"
  - "[[Evaluation Bias]]"
summary: "EU AI Act, US Executive Order 14110, and China's AI labeling measures all mandate watermarking for content provenance without requiring cross-lingual or demographic fairness evaluation."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

The three major regulatory efforts referencing watermarking — the European Union AI Act, United States Executive Order 14110, and China's Measures for Labeling AI-Generated Synthetic Content — all mandate some form of content marking or traceability for AI-generated outputs. Yet none require that detection performance be evaluated across languages, cultural content types, or demographic groups. The EU AI Act requires bias evaluation for general-purpose and high-risk AI systems yet exempts watermarking from equivalent scrutiny. The paper calls for extending existing audit requirements to cover watermarking.
""")

atoms.append(f"""---
title: "Three Proposed Dimensions for Pluralistic Watermark Benchmarking"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Evaluation Bias]]"
summary: "The paper proposes three minimum evaluation dimensions: cross-lingual detection parity, culturally diverse content coverage, and demographic disaggregation of detection metrics."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

The paper proposes three concrete evaluation dimensions for pluralistic watermark benchmarking: (1) cross-lingual detection parity — testing across languages with different tokenization efficiencies and morphological structures, with per-language false positive and false negative rates; (2) culturally diverse content coverage — including visual traditions outside ImageNet distributions, naturalistic acoustic environments, and diverse rhetorical conventions; (3) demographic disaggregation — reporting false positive and false negative rates stratified by linguistic background, sex, age, and cultural origin. AudioMarkBench demonstrates all three are feasible within a single benchmark.
""")

atoms.append(f"""---
title: "Cross-Lingual Translation Removes Watermark Detectability Entirely"
created: "{TODAY}"
type: atom
source: "arXiv:2604.13776"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Watermarking]]"
  - "[[Cross-Lingual Bias]]"
summary: "All current token-distribution watermarking methods lose detectability entirely when watermarked text is translated across languages, reducing detection to chance-level performance."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Research by He et al. demonstrates that all current token-distribution watermarking methods lose detectability entirely when watermarked text is translated across languages. Cross-lingual watermark removal reduces detection to chance-level performance, meaning watermarks cannot serve as reliable content authentication mechanisms in multilingual contexts under current methods. This creates a fundamental tension: deployment mandates assume watermark persistence, but translation (a common use case for AI-generated content) breaks the detection chain.
""")

# ============================================================
# MOLECULE: Cross-Paper Synthesis — Evaluation & Bias in LLMs
# ============================================================
MOLECULE_TITLE = "The Evaluation Bias Lineage - From Judge Reliability to Watermarking Fairness"

molecule = f"""---
title: "{MOLECULE_TITLE}"
created: "{TODAY}"
type: molecule
topics:
  - "[[LLM Evaluation]]"
  - "[[Evaluation Bias]]"
  - "[[Cross-Lingual Bias]]"
  - "[[AI Safety]]"
  - "[[Benchmarking]]"
summary: "Four June 2026 papers reveal a unified thesis: current LLM evaluation infrastructure — from judge benchmarks to watermarking — operates under implicitly monocultural assumptions that systematically disadvantage non-dominant languages, cultures, and reasoning styles."
atoms:
  - "[[LLM Judges Exhibit 2.5x Higher Error on Real vs Synthetic Math Responses]]"
  - "[[Synthetic Math Errors Exhibit Structural Collapse Into Low-Dimensional Subspaces]]"
  - "[[Human Math Reasoning Shows Higher Information-Theoretic Surprisal Than LLM Outputs]]"
  - "[[Surface-Level Style Transfer Cannot Close the LLM Evaluation Gap for Math]]"
  - "[[RealMath-Eval Benchmark Contains 224 Diverse Real Exam Responses]]"
  - "[[Current LLM-as-Judge Pipelines Over-rely on Synthetic Evaluation Data]]"
  - "[[LLM Judges Near-Perfect at Solving but Struggle to Evaluate Diverse Human Reasoning]]"
  - "[[RealMath-Eval Uses Expert Human Grading as Ground Truth Against LLM Judges]]"
  - "[[Only 33.5% Cross-Model Agreement on Specific Cultural Substitutions in Math Problems]]"
  - "[[Cultural Adaptation Compresses Rather Than Expands Diversity of Entity Values]]"
  - "[[LLMs Prioritize Surface Cultural Markers While Preserving Deep Structural Assumptions]]"
  - "[[LLMs Produce Cross-Regional Misattribution in Cultural Adaptation of Math Problems]]"
  - "[[LLM Adaptation Patterns Cluster by Geographic Region Not Language Boundaries]]"
  - "[[6,489 Entity Transformations Annotated Across 76 Cultural Entity Types]]"
  - "[[Models Show Stable Individual Tendencies Across All Seven Target Languages]]"
  - "[[Surface Plausibility Masks Deeper Cultural Adaptation Failures in LLM Outputs]]"
  - "[[Llama-4 Shows +0.800 Coercive Rhetoric Increase Under Turkish Geopolitical Wargame]]"
  - "[[DeepSeek-R1 and Gemini-3.1-Pro Show Directional Reversal Under Turkish Conditions]]"
  - "[[GPT-4o Shows No Statistically Significant Cross-Lingual Geopolitical Skew]]"
  - "[[Chain-of-Thought Institutional Anchoring Buffers Against Cross-Lingual Skew]]"
  - "[[Multilingual RLHF Training Can Reverse Rather Than Merely Attenuate Cross-Lingual Bias]]"
  - "[[Multi-Agent Wargaming Reveals Latent Ideological Skew That Static Benchmarks Miss]]"
  - "[[Inter-Model Variance Within Western Model Class Challenges Monolithic Assumptions]]"
  - "[[Cerulean Sea Crisis Wargame Uses SHA-256 Verified Synthetic Statecraft for Reproducibility]]"
  - "[[Major Watermark Benchmarks Fail to Evaluate Cross-Lingual or Demographic Fairness]]"
  - "[[Non-Native English Speakers Face Higher False Positive Rates in Text Watermarking]]"
  - "[[Image Watermark Bias Untested Despite Known Demographic Asymmetries in JPEG Compression]]"
  - "[[AudioMarkBench Found Female Speech Has Higher False Positive Rates in Watermark Detection]]"
  - "[[Watermarking Is Part of the Pluralistic Alignment Pipeline Requiring Fairness Evaluation]]"
  - "[[Governance Frameworks Mandate Watermarking Without Requiring Fairness Evaluation]]"
  - "[[Three Proposed Dimensions for Pluralistic Watermark Benchmarking]]"
  - "[[Cross-Lingual Translation Removes Watermark Detectability Entirely]]"
related:
  - "[[{P1_CLIP}]]"
  - "[[{P2_CLIP}]]"
  - "[[{P3_CLIP}]]"
  - "[[{P4_CLIP}]]"
publish: true
---

# The Evaluation Bias Lineage: From Judge Reliability to Watermarking Fairness

## Shared Thesis

Four papers published in June 2026 converge on a single, unsettling finding: the infrastructure that evaluates, authenticates, and adapts AI outputs breaks down when it encounters linguistic and cultural diversity. Whether the task is grading a math exam, translating a word problem, simulating geopolitical negotiations, or detecting AI-generated content, current systems optimize for the dominant distribution and fail — often silently — on everything else.

## The Three Layers of Evaluation Bias

### Layer 1: Judge Reliability (RealMath-Eval)

The Evaluation Gap identified by RealMath-Eval is foundational: if LLM judges cannot reliably evaluate authentic human reasoning (MSE ~2.96 vs ~1.17 on synthetic data), then every downstream evaluation that relies on LLM-as-judge inherits this blind spot. The finding that semantic embedding analysis reveals structural collapse in synthetic errors — while human reasoning involves higher information-theoretic surprisal — suggests the gap is fundamental rather than merely statistical. Critically, surface-level style transfer fails to close the gap, indicating the problem is not about how human reasoning *looks* but about its deeper structure.

### Layer 2: Cross-Lingual Cultural Adaptation (Easter Eggs to Eid)

The Easter Eggs to Eid paper reveals that this evaluation gap extends to cultural adaptation. Just as LLM judges struggle with diverse human reasoning styles, cultural adaptation systems compress the diversity of human experience. The entropy collapse observed across all 21 language-model combinations (0.12-0.37 bits reduction) mirrors the structural collapse in synthetic errors — in both cases, the system reduces rich diversity to narrow, predictable patterns.

The cross-regional misattribution finding — models using Bangladeshi taka for Indian Bengali students — connects directly to the Shibboleth Effect's demonstration that language-driven shifts in model behavior are not merely about factual accuracy but about which cultural world the model constructs for its users.

### Layer 3: Geopolitical Skew (Shibboleth Effect) and Watermarking Fairness (Who Gets Flagged)

The Shibboleth Effect demonstrates that cross-lingual bias is not a theoretical concern but has measurable geopolitical consequences. The finding that Llama-4 becomes significantly more coercive under Turkish while DeepSeek-R1 becomes less coercive — and GPT-4o shows no effect — reveals deep architectural differences in how models handle multilingual contexts.

Who Gets Flagged extends this to the verification layer. If watermarking — mandated by governance frameworks worldwide — produces systematically different detection rates across languages and demographics, then the infrastructure meant to authenticate AI content perpetuates the same biases the alignment community seeks to mitigate.

## Cross-Cutting Principles

**The automation irony recurs across all four papers.** RealMath-Eval shows that as evaluation becomes easier (LLM-as-judge), oversight becomes harder because judges appear to perform well on benchmarks that don't test real-world diversity. Easter Eggs to Eid demonstrates the same irony in cultural adaptation: outputs look locally appropriate, making deeper scrutiny feel unnecessary. The Shibboleth Effect reveals that static safety benchmarks similarly miss dynamic behavioral skew. Who Gets Flagged warns that watermarking appears to work — on English, on standard content — while unmeasured disparities persist.

**Surface plausibility is the enemy of safety.** Across all four domains, the systems produce outputs that pass surface-level inspection while harboring structural biases detectable only through corpus-level or adversarial analysis.

**Tokenization and entropy are the common technical substrate.** RealMath-Eval identifies higher surprisal in human reasoning. Easter Eggs to Eid measures entropy collapse in cultural adaptation. The Shibboleth Effect relies on language-specific token pathways. Who Gets Flagged traces watermark bias to tokenizer unevenness and entropy-dependent detection. The token-level statistical properties of language form the shared mechanism through which bias operates.

**Diversity is not noise — it is the signal that current systems fail to process.** The structural collapse of synthetic errors, the entropy collapse of cultural adaptation, the language-driven behavioral skew, and the demographic disparities in watermark detection all share a common root: systems trained and evaluated on homogeneous distributions fail to generalize to the actual diversity of human language, culture, and reasoning.

## Governance Implications

The policy recommendations across these papers form a coherent program: (1) evaluation infrastructure must test on diverse real-world data, not synthetic or curated benchmarks; (2) cross-lingual and cross-cultural performance must be reported as a core requirement, not an optional extension; (3) the same bias auditing requirements applied to AI models must extend to the verification and evaluation layers; (4) governance frameworks (EU AI Act, US Executive Orders, national measures) must close the gap between mandating deployment and requiring fairness evaluation.
"""

# ============================================================
# WRITE ALL FILES
# ============================================================
print("=== CREATING CLIPPINGS ===")
write_file(f"{VAULT}/99/Clippings/{sanitize(P1_CLIP)}.md", clipping_1)
write_file(f"{VAULT}/99/Clippings/{sanitize(P2_CLIP)}.md", clipping_2)
write_file(f"{VAULT}/99/Clippings/{sanitize(P3_CLIP)}.md", clipping_3)
write_file(f"{VAULT}/99/Clippings/{sanitize(P4_CLIP)}.md", clipping_4)

print(f"\n=== CREATING {len(atoms)} ATOMS ===")
for atom_content in atoms:
    # Extract title from frontmatter
    for line in atom_content.split('\n'):
        if line.startswith('title: '):
            atom_title = line.replace('title: "', '').rstrip('"').rstrip("'")
            break
    write_file(f"{VAULT}/1 - Atoms/{sanitize(atom_title)}.md", atom_content)

print(f"\n=== CREATING MOLECULE ===")
write_file(f"{VAULT}/2 - Molecules/{sanitize(MOLECULE_TITLE)}.md", molecule)

print(f"\n=== DONE ===")
print(f"Clippings: 4")
print(f"Atoms: {len(atoms)}")
print(f"Molecules: 1")
