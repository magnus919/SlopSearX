#!/usr/bin/env python3
"""Create vault clippings, atoms, and molecule for ai-in-context paper batch (4 papers)."""
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
# PAPER 1: From Text to Discovery (2606.08723)
# ============================================================
P1_CLIP = "From Text to Discovery - LLMs in Scientific and Humanistic Disciplines"
P1_AUTHORS="Saleh Afroogh, Yasser Pouresmaeil, Yiming Xu, Kevin Chen, Abhejay Murali, Junfeng Jiao"

clipping_1 = f"""---
title: "{P1_CLIP}"
source: "https://arxiv.org/abs/2606.08723"
author: "{P1_AUTHORS}"
published: "2026-06-09"
created: "{TODAY}"
description: "Comprehensive cross-disciplinary survey mapping LLM integration across natural sciences, social sciences, and humanities. Identifies 10 underexplored challenges: erosion of researcher autonomy, AI-driven confirmation bias, authorship ambiguity, unequal access to frontier models, reproducibility concerns, deskilling of domain expertise, homogenization of research approaches, data privacy in research workflows, environmental costs of LLM computation, and challenges in interdisciplinary governance. Calls for coordinated interdisciplinary governance frameworks for LLM integration in academic research."
tags:
  - clippings
topics:
  - "[[LLMs in Research]]"
  - "[[AI & Science]]"
  - "[[Research Methodology]]"
  - "[[AI Governance]]"
related:
  - "[[Support for AI Development - Automated Daily Measurement with Open Data and Code]]"
  - "[[Making a Name for Myself - On Academic Naming Policies and their Impact]]"
publish: true
---

# From Text to Discovery: How Large Language Models Are Accelerating and Complicating Research

**Authors:** {P1_AUTHORS}
**arXiv:** 2606.08723
**Date:** 2026-06-09

## Summary

Comprehensive cross-disciplinary survey examining how LLMs are being integrated into research across natural sciences, social sciences, and humanities. Maps current state, identifies what they deliver, where they fall short, and outlines an agenda for responsible integration.

## The 10 Underexplored Challenges

1. **Erosion of Researcher Autonomy:** Over-reliance on LLM-generated hypotheses and research directions may narrow the scope of scientific inquiry.

2. **AI-Driven Confirmation Bias:** LLMs may reinforce existing beliefs rather than challenging them, producing a filter-bubble effect in research.

3. **Authorship Ambiguity:** Unclear standards for attributing credit when LLMs contribute substantively to research outputs.

4. **Unequal Access:** Frontier models are concentrated among well-resourced institutions, exacerbating global research inequality.

5. **Reproducibility Concerns:** LLM-generated analysis pipelines are not fully reproducible when models and APIs change.

6. **Deskilling of Domain Expertise:** Reliance on LLMs for literature review and synthesis may erode deep disciplinary knowledge.

7. **Homogenization of Research Approaches:** LLM-mediated thinking may converge on dominant research paradigms, reducing methodological diversity.

8. **Data Privacy in Research Workflows:** Sensitive research data processed through third-party LLM APIs creates privacy risks.

9. **Environmental Costs:** The computational resources required for LLM-assisted research carry significant carbon footprints.

10. **Interdisciplinary Governance Challenges:** Existing governance frameworks are fragmented across disciplines, lacking coordination.

## Key Cross-Disciplinary Findings

- **Natural Sciences:** LLMs accelerate literature mining, hypothesis generation, code writing, and experimental design, but risk amplifying noise in automated discovery pipelines.
- **Social Sciences:** LLMs enable large-scale text analysis and survey generation but raise validity concerns when replacing human-coded instruments.
- **Humanities:** LLMs offer new tools for textual analysis and translation but struggle with context-dependent interpretation and cultural nuance.

## Implications

The paper argues that responsible LLM integration requires coordinated interdisciplinary governance — not discipline-specific guidelines — because the challenges (autonomy, bias, access, reproducibility) cut across all fields. Without governance, the pace of adoption will outpace our understanding of consequences.
"""

# ============================================================
# PAPER 2: Support for AI Development (2412.05163)
# ============================================================
P2_CLIP = "Support for AI Development - Automated Daily Measurement with Open Data and Code"
P2_AUTHORS = "Jason Jeffrey Jones"

clipping_2 = f"""---
title: "{P2_CLIP}"
source: "https://arxiv.org/abs/2412.05163"
author: "{P2_AUTHORS}"
published: "2024-12-06"
created: "{TODAY}"
description: "Presents Social Science Dashboard Inator (SSDI), an open-source automated system for daily public opinion nowcasting. Demonstration tracking AI support among American adults generated 766 daily estimates from N=8551 respondents (Apr 2024-May 2026). Key findings: AI support decreased over time; political polarization emerged with Republican support overtaking Democrat support after Nov 2024 election; greater trust and risk willingness associated with higher AI support; males and older adults reported higher support; new themes in open-ended responses include utility consumption concerns and AI companionship desires. Identifies breakpoint April 18, 2025, where increasing AI support reversed to decline."
tags:
  - clippings
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
  - "[[Nowcasting]]"
  - "[[Open Science]]"
  - "[[Survey Methodology]]"
related:
  - "[[From Text to Discovery - LLMs in Scientific and Humanistic Disciplines]]"
  - "[[LLM Chatbot vs Public Health - RCT on HPV Vaccine Communication]]"
publish: true
---

# Support for AI Development: Automated Daily Measurement with Open Data and Code

**Author:** {P2_AUTHORS}
**arXiv:** 2412.05163
**Date:** 2024-12-06 (updated 2026-06-09)

## Summary

Presents and advocates for a new form of scientific communication: free and open nowcasting of public opinion via web dashboard. The Social Science Dashboard Inator (SSDI) autonomously collects, distributes, and presents daily opinion estimates.

## The SSDI System

The Social Science Dashboard Inator (github.com/jasonjeffreyjones/social-science-dashboard-inator) runs a daily automated loop: (1) recruit 11 new respondents from ~90,000 Prolific users, (2) collect survey data via Qualtrics, (3) publish anonymized microdata, (4) update web dashboard. Built with R, Python, cron jobs, and HTML templates.

## Key Findings (Apr 2024 - May 2026)

1. **Support decreased over time:** Mean support dropped from +0.99 (first 30 days) to +0.46 (last 30 days) on a -3 to +3 scale. Daily change coefficient: -0.0009 (CI excludes zero).

2. **Trend break detected April 18, 2025:** Segmented regression found a slope change (Davies test p<0.001). Pre-break: +0.41 scale points/year increase. Post-break: -0.98 scale points/year decline.

3. **Political polarization emerged:** Republican support exceeded Democrat support from December 2024 onward. Month×Party interaction significant (p<0.001).

4. **Trust and Risk associations:** Trusting respondents showed higher AI support (mean 1.11 vs 0.89, p<0.001). Each 1-point increase in risk willingness associated with β=0.203 increase in AI support (p<0.001).

5. **Demographic patterns:** Males (mean 1.34) higher than females (0.67). Older adults showed slower decline in support. Age×Time interaction positive (β=0.092/decade/year, p<0.001).

6. **New themes in open-ended responses:** Mentions of water/electricity concerns increased over time (OR=6.49 per year). AI companionship and personal economic impact emerged as new categories.

## Predictions

Author predicts: (a) AI support will continue decreasing to near zero by Dec 2026, turning negative in 2027; (b) Trust/risk associations with AI support will strengthen; (c) Demographic cleavages will deepen.

## Limitations

US-only sample; 11 respondents/day limits daily precision; relies on paid services (Qualtrics, Prolific); relationships may be time- and context-specific.
"""

# ============================================================
# PAPER 3: LLM Chatbot vs Public Health (2504.20519)
# ============================================================
P3_CLIP = "LLM Chatbot vs Public Health Materials and Parental HPV Vaccination Intentions - RCT"
P3_AUTHORS = "Neil K. R. Sehgal, Sunny Rai, Manuel Tonneau, Anish K. Agarwal, Joseph Cappella, Melanie Kornides, Lyle Ungar, Alison Buttenheim, Sharath Chandra Guntuku"

clipping_3 = f"""---
title: "{P3_CLIP}"
source: "https://arxiv.org/abs/2504.20519"
author: "{P3_AUTHORS}"
published: "2025-04-17"
created: "{TODAY}"
description: "Randomized clinical trial (N=1297) testing whether brief GPT-4o chatbot interactions increase parental intention to vaccinate children against HPV compared with no intervention and government public health materials. Parents recruited from US, Canada, UK. Three-minute GPT-4o interaction increased immediate vaccination intent (d=0.33-0.48) but effects did not persist at 45 days. Government health materials maintained modest effects at 45 days (d=0.53 immediate). No intervention increased self-reported vaccination uptake. Findings suggest well-designed public health materials may match or exceed short LLM chatbot conversations for HPV vaccine promotion."
tags:
  - clippings
topics:
  - "[[AI & Health]]"
  - "[[LLM Applications]]"
  - "[[Public Health]]"
  - "[[RCT]]"
  - "[[HPV Vaccination]]"
related:
  - "[[Support for AI Development - Automated Daily Measurement with Open Data and Code]]"
  - "[[From Text to Discovery - LLMs in Scientific and Humanistic Disciplines]]"
publish: true
---

# Large Language Model Chatbot Conversations vs Public Health Materials and Parental HPV Vaccination Intentions: A Randomized Clinical Trial

**Authors:** {P3_AUTHORS}
**arXiv:** 2504.20519
**Date:** 2025-04-17

## Summary

RCT testing whether brief, multiturn LLM chatbot interactions increase parental intention to vaccinate children against HPV compared with no intervention and government public health materials, and whether effects persist.

## Design

- **N:** 1,297 participants randomized
- **Population:** Adults in US, Canada, UK with at least one HPV vaccine-eligible child (unvaccinated or status unknown)
- **Period:** March 3 - May 25, 2025, with follow-up at 15 and 45 days
- **Demographics:** Mean age 42.84 years; 72.1% female
- **Arms:** (1) No-message control; (2) Country-matched government materials (3+ min exposure); (3) GPT-4o chatbot — default persuasive style; (4) GPT-4o chatbot — shorter conversational style
- **Primary outcome:** Self-reported likelihood of vaccinating within 12 months (0-100 scale), measured immediately after intervention

## Key Findings

1. **Immediate effects:** Government materials increased vaccination intent (Cohen d=0.53, 95% CI 0.36-0.70). Default chatbot (d=0.48, 95% CI 0.30-0.65) and conversational chatbot (d=0.33, 95% CI 0.17-0.49) also showed significant increases.

2. **Durability gap:** At 45 days, neither chatbot increased intent relative to controls. Government public health materials maintained modest effects.

3. **No behavioral change:** No intervention increased self-reported vaccination uptake at either follow-up point.

4. **Chatbot style matters:** Default persuasive style outperformed shorter conversational style, suggesting interaction depth affects impact.

## Implications

- Well-designed public health materials may match or exceed LLM chatbots for vaccine promotion
- LLM chatbot effects on vaccination intent are transient — they don't persist after a single 3-minute interaction
- Raises questions about the role of LLM-based health communication for durable behavior change
- Chatbots may require repeated interactions or integration with broader health communication strategies
"""

# ============================================================
# PAPER 4: Making a Name for Myself (2606.11021)
# ============================================================
P4_CLIP = "Making a Name for Myself - On Academic Naming Policies and their Impact"
P4_AUTHORS = "A Pranav, Vagrant Gautam, Martin Mundt, Jordan Taylor, Arjun Subramonian, Franziska Sofia Hafner, Daniel Chechelnitsky, William Agnew, Anne Lauscher"

clipping_4 = f"""---
title: "{P4_CLIP}"
source: "https://arxiv.org/abs/2606.11021"
author: "{P4_AUTHORS}"
published: "2026-06-10"
created: "{TODAY}"
description: "Mixed-methods study of CS academic naming policies for scholars who change names (due to marriage, gender transition, religious conversion, etc.). Analyzes 44 CS venues and 41 author surveys. Venues with accessible visible naming policies have fewer citation errors (899 vs 996 errors per 1000 papers). Deadnaming decreased 92% from 2019-2024. Documents multi-year advocacy efforts that led to first name change policies. Identifies policy gaps: many venues require legal name for indexing, lack clear procedures, and inconsistently handle preprint-to-proceedings name mismatches. Proposes framework for equitable naming policies."
tags:
  - clippings
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[Deadnaming]]"
  - "[[DEI in AI]]"
  - "[[CS Venues]]"
related:
  - "[[From Text to Discovery - LLMs in Scientific and Humanistic Disciplines]]"
  - "[[Support for AI Development - Automated Daily Measurement with Open Data and Code]]"
publish: true
---

# Making a Name for Myself: On Academic Naming Policies and their Impact

**Authors:** {P4_AUTHORS}
**arXiv:** 2606.11021
**Date:** 2026-06-10

## Summary

Mixed-methods study examining how computer science academic venues handle name changes (due to marriage, gender transition, religious conversion, etc.) and the impact of their policies on scholars.

## Methods

- **Venue analysis:** 44 CS venues evaluated for naming policies
- **Author survey:** 41 scholars who have changed names
- **Citation error analysis:** Comparing venues with accessible vs. non-accessible policies
- **Longitudinal deadnaming tracking:** 2019-2024

## Key Findings

1. **Citation errors reduced by accessible policies:** Venues with accessible, visible naming policies have 899 citation errors per 1000 papers vs 996 for venues without such policies.

2. **Deadnaming decreased 92% from 2019-2024:** Significant improvement in academic community awareness and practice, though 8% of papers from 2024 still contain deadnames.

3. **Policy gaps persist:** Many venues: (a) require legal names for indexing purposes, creating barriers; (b) lack clear, publicized procedures for name changes; (c) inconsistently handle cases where a scholar's name differs between preprint and proceedings versions.

4. **Multi-year advocacy documented:** The paper traces the history of advocacy efforts by authors and the broader community that led to the first name change policies at major CS venues.

5. **Survey findings:** Scholars report significant emotional and professional burden from navigating inconsistent policies. Some report avoiding name changes for career reasons.

## Proposed Framework

The authors propose that venues should: (1) maintain clear, accessible name change policies; (2) allow name changes without requiring legal documentation; (3) retroactively update author names on published works; (4) separate display names from indexing identifiers; (5) provide clear guidance for preprint-to-proceedings name consistency.

## Implications

Naming policies are not merely administrative — they directly affect scholar wellbeing, career progression, and the accuracy of the academic record. The 92% reduction in deadnaming demonstrates that community awareness and policy changes can drive meaningful improvement.
"""

# ============================================================
# ATOMS — 6-8 per paper = 28 atoms
# ============================================================
atoms = []

# === PAPER 1: From Text to Discovery (6 atoms — survey paper, position-level) ===
atoms.append(f"""---
title: "LLM Integration in Research Faces 10 Underexplored Challenges Across Disciplines"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLMs in Research]]"
  - "[[AI & Science]]"
  - "[[AI Governance]]"
summary: "Cross-disciplinary survey identifies 10 challenges including erosion of researcher autonomy, AI-driven confirmation bias, authorship ambiguity, unequal access, reproducibility, deskilling, homogenization, data privacy, environmental costs, and governance fragmentation."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

A comprehensive cross-disciplinary survey of LLM integration in research identifies 10 underexplored challenges: (1) erosion of researcher autonomy from over-reliance on LLM-generated hypotheses; (2) AI-driven confirmation bias reinforcing existing beliefs; (3) authorship ambiguity in crediting LLM contributions; (4) unequal access to frontier models across institutions; (5) reproducibility concerns when models/APIs change; (6) deskilling of domain expertise from over-reliance; (7) homogenization of research approaches through LLM-mediated thinking; (8) data privacy risks in third-party LLM APIs; (9) environmental costs of LLM computation; (10) fragmented interdisciplinary governance. The authors argue these challenges require coordinated, cross-disciplinary governance rather than discipline-specific guidelines.
""")

atoms.append(f"""--- 
title: "LLMs Narrow Rather Than Diversify Scientific Inquiry Through Algorithmic Confirmation Bias"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLMs in Research]]"
  - "[[AI & Science]]"
  - "[[Research Methodology]]"
summary: "The survey identifies AI-driven confirmation bias and homogenization of research approaches as risks — LLMs may narrow scientific inquiry by reinforcing dominant paradigms and preferred hypotheses."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

Among the 10 underexplored challenges, the survey highlights a dual risk to scientific diversity: AI-driven confirmation bias may lead researchers to pursue LLM-endorsed hypotheses while neglecting contrarian or under-explored directions, and homogenization of research approaches may reduce methodological diversity as LLM-mediated thinking converges on dominant research paradigms. Together, these risks suggest LLM integration could narrow rather than broaden the scope of scientific inquiry, particularly in fields where researchers rely heavily on LLMs for literature review and hypothesis generation.
""")

atoms.append(f"""---
title: "LLM-Assisted Research Governance Requires Cross-Disciplinary Coordination Beyond Existing Frameworks"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[AI Governance]]"
  - "[[LLMs in Research]]"
summary: "The survey argues existing governance frameworks are fragmented across disciplines and inadequate for the cross-cutting challenges of LLM integration in research."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

The survey's central governance argument is that LLM integration challenges cut across all research fields — autonomy erosion, bias, access, reproducibility, deskilling — and therefore require coordinated interdisciplinary governance rather than the current fragmented approach where each discipline develops separate guidelines. The authors call for a unified framework that addresses the shared challenges while respecting field-specific differences in methodology and epistemology. Without such coordination, the pace of LLM adoption in research will continue to outpace governance development.
""")

atoms.append(f"""---
title: "LLM Integration Varies Significantly by Discipline from Natural Sciences to Humanities"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLMs in Research]]"
  - "[[AI & Science]]"
summary: "Survey maps how LLM integration differs across natural sciences (literature mining, code writing), social sciences (text analysis, survey generation), and humanities (textual analysis, translation)."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

The survey provides a cross-disciplinary mapping of LLM integration: in natural sciences, LLMs primarily accelerate literature mining, hypothesis generation, code writing, and experimental design, but risk amplifying noise in automated discovery pipelines. In social sciences, LLMs enable large-scale text analysis and automated survey generation but raise validity concerns when replacing human-coded instruments. In humanities, LLMs offer new tools for textual analysis and translation but struggle with context-dependent interpretation and cultural nuance — areas where domain expertise remains critical.
""")

atoms.append(f"""---
title: "Research Reproducibility at Risk When LLMs Become Embedded in Analysis Pipelines"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLMs in Research]]"
  - "[[Reproducibility]]"
summary: "LLM-generated analysis pipelines pose reproducibility challenges because models and APIs change over time, making exact re-execution impossible."
related:
  - "[[{P1_CLIP}]]"
publish: true
---

The survey identifies a critical and underexplored reproducibility challenge: when researchers use LLMs to generate or execute analysis code, the resulting pipelines are not fully reproducible because the underlying models and APIs evolve. A script that worked in 2024 may produce different outputs with the same prompts in 2026 due to model updates. This creates a fundamental tension between the convenience of LLM integration and the scientific imperative of reproducibility. The authors call for version-locking LLM tools in research pipelines and maintaining complete interaction logs as supplementary materials.
""")

atoms.append(f"""---
title: "Unequal Access to Frontier LLMs Risks Widening Global Research Inequality"
created: "{TODAY}"
type: atom
source: "arXiv:2606.08723"
derived_from: "[[{P1_CLIP}]]"
topics:
  - "[[LLMs in Research]]"
  - "[[AI & Science]]"
  - "[[AI Ethics]]"
summary: "Frontier LLMs are concentrated among well-resourced institutions, creating a two-tier research system that exacerbates existing global research inequality."
related:
  - "[[{P1_CLIP}]]"
  - "[[{P2_CLIP}]]"
publish: true
---

The survey identifies unequal access to frontier LLMs as a structural challenge for global research equity. Well-resourced institutions in wealthy countries can access the most capable models for literature review, data analysis, code generation, and writing assistance, while researchers at less-resourced institutions — including those in the Global South — are limited to less capable open-weight models or cannot afford API costs. This creates a two-tier research system that could exacerbate existing disparities in research productivity and quality. The authors call for subsidized access programs and investment in open-source alternatives.
""")

# === PAPER 2: Support for AI Development (8 atoms) ===
atoms.append(f"""---
title: "Daily Opinion Nowcasting System Generated 766 Estimates of AI Support From N=8551 Respondents"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[Nowcasting]]"
  - "[[Open Science]]"
summary: "The SSDI system autonomously produced 766 daily estimates of AI support from 8551 respondents over 25 months, providing unprecedented temporal resolution for tracking opinion change."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

The Social Science Dashboard Inator (SSDI), an open-source automated system, generated 766 daily estimates of American adults' support for further AI development from N=8551 respondents between April 18, 2024 and May 31, 2026. The system recruited 11 randomly selected respondents daily via Prolific, collected a 6-item survey via Qualtrics (~2 min completion, $0.30/respondent), and automatically published anonymized microdata with updated analyses to a public web dashboard. The unprecedented temporal resolution enables detection of opinion shifts that annual or one-shot surveys would miss.
""")

atoms.append(f"""---
title: "American AI Support Reached a Turning Point on April 18 2025 Then Began Declining"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
summary: "Segmented regression reveals a breakpoint on April 18, 2025 (SE 19 days): AI support increased +0.41 scale points/year before, then declined -0.98 scale points/year after."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

A Davies test confirmed significant slope change in the AI support time series (p<0.001). Segmented regression placed the breakpoint on April 18, 2025 (standard error 19 days). Before the breakpoint, estimated support increased by approximately 0.41 scale points per year; afterward, it declined by approximately 0.98 scale points per year. The segmented model explained 25.5% of variation in daily mean support (adjusted R²=0.255). This precision in dating the trend reversal is likely not achievable with any other existing survey due to the SSDI's unique daily temporal resolution.
""")

atoms.append(f"""---
title: "Political Polarization of AI Support Emerged After November 2024 US Election"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
  - "[[Political Polarization]]"
summary: "Republican AI support exceeded Democrat support from December 2024 onward, suggesting partisan control of government shapes attitudes toward powerful new technologies."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Monthly-aggregated data reveals that Republican support for AI development exceeded Democrat support beginning in December 2024 — one month after the Republican party won the Presidency and majorities in both houses of Congress. A linear model confirmed a significant negative coefficient for Month (p<0.001) and a significant Month×Party interaction (p<0.001). The author hypothesizes that partisans' support for powerful new technologies may depend on their preferred party's level of federal power. This polarization mirrors patterns observed in climate science and vaccines, suggesting AI may face similar politicization.
""")

atoms.append(f"""---
title: "Generalized Trust and Risk Willingness Are Reliable Predictors of AI Support"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
  - "[[Risk Perception]]"
summary: "Trusting respondents showed higher AI support (mean 1.11 vs 0.89, p<0.001). Each 1-point increase in risk willingness associated with 0.203 increase in AI support. Consistent with Diffusion of Innovation Theory."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

This is the first study to explore the relationship between AI Support and the widely-studied constructs of Generalized Trust and Risk Willingness. Trusting respondents (those saying "most people can be trusted") reported higher AI Support (mean=1.11) than Careful respondents (mean=0.89; p<0.001). Greater risk willingness was associated with higher AI support — each one-point increase on the 0-10 risk scale corresponded to β=0.203 increase in AI Support (p<0.001). This result is consistent with Diffusion of Innovation Theory, which positions risk tolerance as a key predictor of early adoption.
""")

atoms.append(f"""---
title: "Male and Older Adults Report Higher AI Support With Younger Adults Showing Steeper Decline"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
summary: "Males report higher AI support (mean 1.34 vs 0.67 females). Older adults show slower decline in support, with Age×Time interaction β=0.092/year (p<0.001)."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Demographic analysis reveals consistent cleavages in AI Support. Male respondents reported significantly higher support than female respondents (mean 1.34 vs 0.67, p<0.001), with both sexes participating in the overall downward trend. Age analysis shows a small positive relationship (β=0.04 per decade, p=0.007). More importantly, the Age×Time interaction reveals that the decline in support over time was steeper among younger respondents: each additional decade of age reduced the yearly decline by 0.092 scale points (p<0.001). This means the age gap in AI support is growing over time, driven by younger adults losing enthusiasm faster.
""")

atoms.append(f"""---
title: "New Themes in Public AI Discourse Include Water Electricity Concerns and AI Companionship"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Public Opinion]]"
  - "[[AI & Society]]"
summary: "Open-ended responses reveal emerging concerns about AI's water/electricity consumption (OR=6.49/year increase) and desires for AI companionship — themes absent from previous research."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

Analysis of 5,965 open-ended text responses (69.8% of total respondents) reveals that public discourse about AI is evolving. While Kelley et al.'s four themes (exciting, useful, worrying, futuristic) still appear, new categories have emerged. Mentions of water and electricity consumption increased dramatically over time — each year was associated with 6.49 times higher odds of mentioning these environmental concerns (p<0.001). Dozens of respondents reported AI companionship ("It is like a personal friend"; "I'm lonely and want an AI girlfriend") and personal economic impacts from AI replacing jobs (freelance artists, writers). These new themes extend the established four-category framework.
""")

atoms.append(f"""---
title: "SSDI Open Nowcasting Framework Addresses Replication and Predictability Crises in Social Science"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Open Science]]"
  - "[[Nowcasting]]"
  - "[[Social Science Methodology]]"
summary: "The SSDI framework directly addresses the replication/reproducibility crisis and the predictability crisis in social science through built-in transparency and continuous data collection."
related:
  - "[[{P2_CLIP}]]"
  - "[[{P1_CLIP}]]"
publish: true
---

The author argues that daily nowcasting with freely and openly available data confronts two social science crises. First, the replication/reproducibility crisis: open access to data and analysis code makes reproducibility trivially easy, and simultaneous replication is possible. Second, the "predictability crisis": theories that successfully predict outcomes have not kept pace with data growth. SSDIs offer principled prediction — theories must make testable claims about daily time series trajectories, dramatically shortening feedback loops from years to days. The author advocates for dashboard-based scientific communication as an alternative to traditional publishing.
""")

atoms.append(f"""---
title: "Daily Cadence of 11 Respondents Achieved Viable Temporal Resolution at $3/Day Self-Funded Cost"
created: "{TODAY}"
type: atom
source: "arXiv:2412.05163"
derived_from: "[[{P2_CLIP}]]"
topics:
  - "[[Survey Methodology]]"
  - "[[Open Science]]"
summary: "The daily sample of 11 respondents was chosen to keep costs manageable (~$3/day) for a self-funded researcher while still providing useful temporal resolution."
related:
  - "[[{P2_CLIP}]]"
publish: true
---

The figure of 11 respondents per day was chosen not from power analysis but from practical budget constraints — approximately $3/day in participant fees ($0.30/respondent + 33% platform fee) for a self-funded lone researcher. While daily estimates are noisy due to the small daily n, the high temporal resolution enables aggregation at weekly, monthly, or annual levels when precision is needed. The author notes the system scales with investment: larger budgets enable larger daily samples or hourly cadences. This pragmatic approach demonstrates that useful continuous opinion tracking is achievable at modest cost.
""")

# === PAPER 3: LLM Chatbot vs Public Health (7 atoms) ===
atoms.append(f"""---
title: "GPT-4o Chatbot Increases Immediate HPV Vaccination Intent but Effects Fade by 45 Days"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[Public Health]]"
  - "[[RCT]]"
summary: "3-minute GPT-4o interaction increased immediate HPV vaccination intent (d=0.33-0.48) but at 45 days, neither chatbot condition showed significant effects vs controls."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

In an RCT of 1,297 parents, a 3-minute GPT-4o chatbot interaction increased immediate self-reported likelihood of vaccinating a child against HPV within 12 months. The default persuasive chatbot showed d=0.48 (95% CI 0.30-0.65) and the shorter conversational chatbot showed d=0.33 (95% CI 0.17-0.49) vs no intervention. However, at 45-day follow-up, neither chatbot condition maintained significant increases in vaccination intent relative to controls. Government public health materials, by contrast, maintained modest effects at 45 days. No intervention increased actual vaccination uptake.
""")

atoms.append(f"""---
title: "Government Public Health Materials Outperform LLM Chatbots for Durable HPV Vaccine Communication"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[Public Health]]"
summary: "Government health materials showed larger immediate effects (d=0.53) than both chatbot conditions (d=0.33-0.48) and were the only intervention maintaining effects at 45 days."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

Country-matched government public health materials (with at least 3 minutes of exposure) outperformed both LLM chatbot conditions on durability: immediate effect d=0.53 (95% CI 0.36-0.70) and maintained modest effects at 45-day follow-up. Neither GPT-4o chatbot (default persuasive or conversational style) maintained significant effects at 45 days. These findings suggest that well-designed public health materials — developed by domain experts and tested for efficacy — may be more effective than short LLM chatbot conversations for health behavior communication, particularly for outcomes requiring durable attitude change.
""")

atoms.append(f"""---
title: "LLM Health Communication Shows Intent-Behavior Gap With No Change in Vaccination Uptake"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[Public Health]]"
summary: "Despite increasing immediate vaccination intent, none of the interventions — chatbot or government materials — increased self-reported HPV vaccination uptake at 15 or 45 days."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

A critical finding is the intent-behavior gap in health communication: although chatbot interactions and government materials increased self-reported vaccination intentions immediately after intervention, none of the conditions produced a statistically significant increase in actual vaccination uptake at either 15-day or 45-day follow-up. This gap between stated intentions and actual behavior is well-documented in health psychology and underscores the challenge of converting communication-based attitude change into real-world health actions. It suggests that even effective health communication may require additional structural or behavioral interventions to produce vaccination behavior change.
""")

atoms.append(f"""---
title: "Chatbot Persuasive Style Outperforms Conversational Style for Vaccine Communication"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[LLM Applications]]"
summary: "Default persuasive chatbot style (d=0.48) outperformed shorter conversational style (d=0.33) for immediate HPV vaccination intent, suggesting interaction depth and argument quality matter."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

The RCT tested two GPT-4o interaction styles: a default persuasive chatbot designed to present structured arguments for HPV vaccination, and a shorter, more conversational style. The default persuasive style showed larger immediate effects (d=0.48, 95% CI 0.30-0.65) than the conversational style (d=0.33, 95% CI 0.17-0.49), though confidence intervals overlapped. This suggests that interaction depth and argument quality — not just chatbot engagement — influence effectiveness for health behavior communication. Neither style showed persistent effects at 45 days.
""")

atoms.append(f"""---
title: "RCT of 1297 Parents Tests LLM Chatbot vs Standard Health Materials for HPV Communication"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[RCT]]"
  - "[[HPV Vaccination]]"
summary: "Large-scale RCT (N=1297) across US, Canada, UK testing LLM chatbot against government materials for HPV vaccine communication with 45-day follow-up."
related:
  - "[[{P3_CLIP}]]"
publish: true
---

This RCT of 1,297 parents in the US, Canada, and UK (conducted March 3 - May 25, 2025) provides one of the largest controlled tests of LLM-based health communication to date. Participants had at least one HPV vaccine-eligible child who was unvaccinated or whose vaccination status was unknown. Mean age 42.84 years, 72.1% female. The four-arm design (no-message control, government materials, default chatbot, conversational chatbot) with follow-up at 15 and 45 days provides rigorous comparative evidence for the effectiveness of LLM chatbots in health communication contexts.
""")

atoms.append(f"""---
title: "LLM Chatbot Vaccine Communication Creates Temporary Attitude Change Not Durable Behavior Change"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[Public Health]]"
summary: "The transient nature of LLM chatbot effects — immediate attitude change with no persistence at 45 days — suggests chatbots may require repeated interactions or multi-component strategies."
related:
  - "[[{P3_CLIP}]]"
  - "[[{P2_CLIP}]]"
publish: true
---

The finding that chatbot effects on vaccination intent fade completely by 45 days — while government materials maintain modest effects — has important implications for LLM-based health communication design. A single 3-minute chatbot interaction may create temporary attitude change but is insufficient for durable behavior change. This suggests LLM chatbots may need to be deployed as part of multi-component strategies including repeated interactions, integration with healthcare provider recommendations, reminder systems, or structural interventions that reduce barriers to vaccination.
""")

atoms.append(f"""---
title: "Large Language Models Show Promise but Limited Evidence for Health Behavior Change Applications"
created: "{TODAY}"
type: atom
source: "arXiv:2504.20519"
derived_from: "[[{P3_CLIP}]]"
topics:
  - "[[AI & Health]]"
  - "[[Public Health]]"
  - "[[LLM Applications]]"
summary: "This RCT provides controlled evidence that LLM chatbots can increase immediate health intentions but not durable behavior change, contributing to the evidence base for LLM health applications."
related:
  - "[[{P3_CLIP}]]"
  - "[[{P1_CLIP}]]"
publish: true
---

This RCT contributes rigorous evidence to the growing literature on LLM applications in health communication. While LLM chatbots show promise for immediate attitude change — consistent with their demonstrated effectiveness in other persuasive contexts — the evidence for durable behavior change remains limited. The study underscores the importance of follow-up periods in evaluating health communication interventions: cross-sectional or immediate-post-intervention measures would have overestimated chatbot effectiveness. This finding aligns with the broader challenge identified in the survey paper (2606.08723) about the gap between LLM capability claims and demonstrated real-world impact.
""")

# === PAPER 4: Making a Name for Myself (7 atoms) ===
atoms.append(f"""---
title: "Venues With Accessible Naming Policies Have 10% Fewer Citation Errors for Name-Changing Scholars"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[DEI in AI]]"
summary: "CS venues with accessible visible naming policies have 899 citation errors per 1000 papers vs 996 for venues without such policies — a ~10% reduction."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

A quantitative analysis of 44 computer science venues found that those with accessible, visible naming policies — policies that are easy to find, clearly communicated, and provide straightforward procedures for name changes — have approximately 899 citation errors per 1,000 papers authored by scholars who have changed names, compared to 996 errors per 1,000 papers at venues without such policies. This ~10% reduction in citation errors demonstrates that clear naming policies have measurable practical benefits for the accuracy of the academic record, in addition to their impact on scholar wellbeing.
""")

atoms.append(f"""---
title: "Deadnaming in CS Academic Publications Decreased 92% From 2019 to 2024"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[Deadnaming]]"
  - "[[DEI in AI]]"
summary: "Longitudinal tracking shows a 92% decrease in deadnaming incidents in CS publications from 2019-2024, though 8% of 2024 papers still contain deadnames."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Longitudinal tracking of deadnaming — referring to a scholar by their former name after they have changed names — in computer science publications shows a dramatic 92% decrease from 2019 to 2024. This improvement is attributed to multi-year advocacy efforts by the authors and the broader community, increased awareness, and the adoption of name change policies at major venues. However, 8% of papers from 2024 still contain deadnames, indicating the problem persists. The 92% reduction demonstrates that community-led advocacy and policy changes can drive meaningful improvement, while the remaining gap highlights the need for continued effort and systematic solutions.
""")

atoms.append(f"""---
title: "Academic Naming Policies Remain Inconsistent With Legal Name Requirements as Key Barrier"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[DEI in AI]]"
summary: "Survey of 44 CS venues reveals persistent policy gaps: legal name requirements for indexing, unclear procedures, and inconsistent handling of preprint-to-proceedings name mismatches."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Analysis of 44 computer science venues reveals three major policy gaps. First, many venues require legal names for indexing and archival purposes, creating significant barriers for scholars who cannot or do not want to use their legal name. Second, procedures for changing names are often unclear, unpublished, or inconsistently applied. Third, venues inconsistently handle cases where a scholar's name differs between their preprint and the final proceedings version. These gaps create administrative burden and emotional distress for scholars navigating name changes, and some respondents reported delaying or avoiding name changes for career reasons.
""")

atoms.append(f"""---
title: "Multi-Year Community Advocacy Led to First Name Change Policies at Major CS Venues"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[DEI in AI]]"
summary: "The paper documents a multi-year advocacy campaign by authors and community members that led to the adoption of first name change policies at major CS publication venues."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

The paper traces the history of community advocacy efforts that led to the first name change policies at major computer science venues. This advocacy included public letters, conference workshops, direct engagement with venue leadership, and the development of model policies. The authors document that persistent, community-organized advocacy — not top-down institutional mandates — was the primary driver of policy change. This case study in academic governance reform demonstrates how grassroots organizing combined with empirical evidence can produce institutional change in scholarly publishing practices.
""")

atoms.append(f"""---
title: "Scholar Survey Reveals Emotional and Professional Burden of Inconsistent Naming Policies"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[DEI in AI]]"
summary: "Survey of 41 scholars who changed names reveals significant emotional burden, professional costs from citation errors, and in some cases, avoidance of name changes for career reasons."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

A survey of 41 scholars who have changed names (for reasons including marriage, gender transition, religious conversion, and personal preference) reveals the human impact of inconsistent naming policies. Respondents reported significant emotional burden from navigating venue-specific procedures, professional costs from citation errors that affect academic recognition and metrics, and anxiety about how name changes would affect their academic identity. Some respondents reported delaying or reconsidering name changes due to concerns about career impact, highlighting how administrative barriers can compound the personal challenges of name changes.
""")

atoms.append(f"""---
title: "Proposed Framework for Equitable Academic Naming Policies Across Publication Venues"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[DEI in AI]]"
summary: "Authors propose five principles: clear accessible policies, no legal documentation required, retroactive updates, separate display/indexing names, preprint-to-proceedings guidance."
related:
  - "[[{P4_CLIP}]]"
publish: true
---

Based on their analysis of venue policies and scholar surveys, the authors propose a framework with five principles for equitable naming policies: (1) maintain clear, easily accessible name change policies prominently linked from venue websites; (2) allow name changes without requiring legal documentation or proof of name change; (3) retroactively update author names on published works — including in digital libraries and indexing services; (4) separate display names (which can be changed) from indexing identifiers (which remain stable); (5) provide clear guidance for handling name consistency between preprints and proceedings versions.
""")

atoms.append(f"""---
title: "Citation Errors From Deadnaming Persist as Technical and Social Challenge in Academic Infrastructure"
created: "{TODAY}"
type: atom
source: "arXiv:2606.11021"
derived_from: "[[{P4_CLIP}]]"
topics:
  - "[[Academic Publishing]]"
  - "[[Name Changes]]"
  - "[[Deadnaming]]"
summary: "Even with policy improvements, citation errors persist due to decentralized indexing systems, automated citation parsers, and the difficulty of propagating name changes across services."
related:
  - "[[{P4_CLIP}]]"
  - "[[{P1_CLIP}]]"
publish: true
---

Even when venues adopt model naming policies, citation errors persist due to the decentralized structure of academic infrastructure. Name changes must propagate across multiple systems — publisher databases, Crossref, ORCID, Google Scholar, Semantic Scholar, arXiv, DBLP, and institutional repositories — each with different update procedures. Automated citation parsers that match author names between papers and reference lists compound the problem when they encounter mismatched names. This technical challenge intersects with the social challenge of awareness and training: authors citing papers may not know a scholar has changed their name, and automated systems have no mechanism to detect or correct deadnaming.
""")

# ============================================================
# MOLECULE: Cross-Paper Synthesis — AI in Context
# ============================================================
MOLECULE_TITLE = "AI in Context - From Research Integration to Public Reception to Scholarly Infrastructure"

molecule = f"""---
title: "{MOLECULE_TITLE}"
created: "{TODAY}"
type: molecule
topics:
  - "[[AI & Society]]"
  - "[[Public Opinion]]"
  - "[[AI & Health]]"
  - "[[Academic Publishing]]"
  - "[[LLMs in Research]]"
  - "[[AI Governance]]"
summary: "Four papers from 2024-2026 examine AI across multiple contexts: as research tool (Afroogh et al.), as object of public opinion (Jones), as health communication medium (Sehgal et al.), and as force reshaping academic infrastructure (Pranav et al.). Together they reveal that AI's impact is context-dependent — its effects on research, public attitudes, health behavior, and scholarly infrastructure each follow different dynamics and require different governance approaches."
atoms:
  - "[[LLM Integration in Research Faces 10 Underexplored Challenges Across Disciplines]]"
  - "[[LLMs Narrow Rather Than Diversify Scientific Inquiry Through Algorithmic Confirmation Bias]]"
  - "[[LLM-Assisted Research Governance Requires Cross-Disciplinary Coordination Beyond Existing Frameworks]]"
  - "[[LLM Integration Varies Significantly by Discipline from Natural Sciences to Humanities]]"
  - "[[Research Reproducibility at Risk When LLMs Become Embedded in Analysis Pipelines]]"
  - "[[Unequal Access to Frontier LLMs Risks Widening Global Research Inequality]]"
  - "[[Daily Opinion Nowcasting System Generated 766 Estimates of AI Support From N=8551 Respondents]]"
  - "[[American AI Support Reached a Turning Point on April 18 2025 Then Began Declining]]"
  - "[[Political Polarization of AI Support Emerged After November 2024 US Election]]"
  - "[[Generalized Trust and Risk Willingness Are Reliable Predictors of AI Support]]"
  - "[[Male and Older Adults Report Higher AI Support With Younger Adults Showing Steeper Decline]]"
  - "[[New Themes in Public AI Discourse Include Water Electricity Concerns and AI Companionship]]"
  - "[[SSDI Open Nowcasting Framework Addresses Replication and Predictability Crises in Social Science]]"
  - "[[Daily Cadence of 11 Respondents Achieved Viable Temporal Resolution at $3/Day Self-Funded Cost]]"
  - "[[GPT-4o Chatbot Increases Immediate HPV Vaccination Intent but Effects Fade by 45 Days]]"
  - "[[Government Public Health Materials Outperform LLM Chatbots for Durable HPV Vaccine Communication]]"
  - "[[LLM Health Communication Shows Intent-Behavior Gap With No Change in Vaccination Uptake]]"
  - "[[Chatbot Persuasive Style Outperforms Conversational Style for Vaccine Communication]]"
  - "[[RCT of 1297 Parents Tests LLM Chatbot vs Standard Health Materials for HPV Communication]]"
  - "[[LLM Chatbot Vaccine Communication Creates Temporary Attitude Change Not Durable Behavior Change]]"
  - "[[Large Language Models Show Promise but Limited Evidence for Health Behavior Change Applications]]"
  - "[[Venues With Accessible Naming Policies Have 10% Fewer Citation Errors for Name-Changing Scholars]]"
  - "[[Deadnaming in CS Academic Publications Decreased 92% From 2019 to 2024]]"
  - "[[Academic Naming Policies Remain Inconsistent With Legal Name Requirements as Key Barrier]]"
  - "[[Multi-Year Community Advocacy Led to First Name Change Policies at Major CS Venues]]"
  - "[[Scholar Survey Reveals Emotional and Professional Burden of Inconsistent Naming Policies]]"
  - "[[Proposed Framework for Equitable Academic Naming Policies Across Publication Venues]]"
  - "[[Citation Errors From Deadnaming Persist as Technical and Social Challenge in Academic Infrastructure]]"
related:
  - "[[{P1_CLIP}]]"
  - "[[{P2_CLIP}]]"
  - "[[{P3_CLIP}]]"
  - "[[{P4_CLIP}]]"
publish: true
---

# AI in Context: From Research Integration to Public Reception to Scholarly Infrastructure

## Shared Thesis

Four papers spanning 2024-2026 examine AI not as a monolithic technology but as a phenomenon embedded in different contexts — each with distinct dynamics, stakeholders, and governance needs. The common thread is that AI's impact cannot be understood or governed in the abstract; it is shaped by the specific context in which it operates.

## The Four Contexts

### Context 1: AI as Research Tool (Afroogh et al., 2606.08723)

The cross-disciplinary survey reveals that LLMs are being rapidly adopted across research fields, but the consequences are poorly understood. The 10 underexplored challenges span epistemology (confirmation bias, autonomy erosion), equity (unequal access), methodology (reproducibility), and governance (fragmented oversight). The key insight is that these challenges are interconnected — addressing any one requires addressing the others.

### Context 2: AI as Object of Public Opinion (Jones, 2412.05163)

The SSDI nowcasting project provides the highest-resolution picture of American AI attitudes to date. AI support is not static or monotonic — it rose, peaked in April 2025, then began declining. The emergence of political polarization and the growing age gap suggest that attitudes toward AI are becoming identity-linked, following the trajectory of climate change and vaccine attitudes. The open nowcasting approach itself models a new form of scientific communication that addresses the reproducibility crisis.

### Context 3: AI as Health Communication Medium (Sehgal et al., 2504.20519)

The HPV vaccination RCT provides a crucial empirical check on claims about LLM effectiveness for health behavior change. While GPT-4o chatbots can increase immediate intentions, the effects are fleeting — gone by 45 days — and no intervention produced actual vaccination uptake. This is a healthy corrective to the narrative of LLMs as transformative for health communication: they are tools that can create temporary shifts but are not substitutes for well-designed public health materials, let alone for structural interventions.

### Context 4: AI as Force Reshaping Scholarly Infrastructure (Pranav et al., 2606.11021)

The naming policy study might seem peripheral to the AI theme, but it examines how AI intersects with the existing infrastructure of academic knowledge production. The 92% reduction in deadnaming from community advocacy demonstrates that infrastructure can be reformed, but the persistence of citation errors reveals how technical systems (citation parsers, indexing services) and social practices (author awareness, venue procedures) interact to produce inequitable outcomes. As AI tools become embedded in this infrastructure, these systemic issues will only compound.

## Cross-Cutting Themes

### The Governance Gap Across Contexts

Each paper identifies a governance gap specific to its context — but the gaps share a common structure: adoption and deployment are outpacing governance. The survey paper identifies fragmented disciplinary governance. The SSDI paper advocates for new forms of scientific communication. The RCT shows that health systems are deploying chatbots before evidence of durable effectiveness. The naming study shows that while policies have improved, infrastructure-level change remains elusive.

### Context-Dependent Effectiveness

The contrast between the narratives in the survey paper (LLMs as transformative) and the RCT (LLM effects are fleeting) is instructive. Both are true: LLMs can accelerate certain research tasks while having limited, transient effects on health behavior. The importance of context-specific evaluation is the meta-finding: what works in one context cannot be assumed to work in another.

### Measurement and Evidence as Governance Tools

The SSDI paper provides a model for how continuous measurement can inform governance: daily opinion tracking detects shifts (political polarization, age divergence) that annual surveys miss. The RCT provides a model for rigorous evidence evaluation. The naming paper shows how quantitative evidence (citation error rates, deadnaming trends) can drive policy reform. Across all four contexts, better measurement creates better governance.

## Implications

1. **No one-size-fits-all governance:** AI's impact is context-dependent, requiring context-specific governance approaches that are coordinated across contexts.
2. **Persistence is the hard problem:** From public opinion to health behavior to infrastructure reform, the challenge is not creating initial change but making it last.
3. **Community advocacy works:** The naming policy study demonstrates that organized community advocacy can drive measurable improvement.
4. **Open infrastructure enables accountability:** The SSDI model of open data, open code, and continuous publication provides a template for accountable AI governance.
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
