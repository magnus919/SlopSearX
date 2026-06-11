#!/usr/bin/env python3
"""Build LightRAG batch JSON from newly created vault files."""
import json, os

VAULT = "/Users/magnus/Obsidian/Magnus v2"
FILES = [
    # Clippings
    "99/Clippings/RealMath-Eval - Why SOTA Judges Struggle with Real Human Reasoning.md",
    "99/Clippings/Who Brought Easter Eggs to Eid - Auditing Cultural Translation of Math Word Problems.md",
    "99/Clippings/The Shibboleth Effect - Auditing the Cross-Lingual Distributional Skew of LLMs.md",
    "99/Clippings/Who Gets Flagged - The Pluralistic Evaluation Gap in AI Content Watermarking.md",
    # Molecule
    "2 - Molecules/The Evaluation Bias Lineage - From Judge Reliability to Watermarking Fairness.md",
    # Atoms (32 total)
    "1 - Atoms/LLM Judges Exhibit 2.5x Higher Error on Real vs Synthetic Math Responses.md",
    "1 - Atoms/Synthetic Math Errors Exhibit Structural Collapse Into Low-Dimensional Subspaces.md",
    "1 - Atoms/Human Math Reasoning Shows Higher Information-Theoretic Surprisal Than LLM Outputs.md",
    "1 - Atoms/Surface-Level Style Transfer Cannot Close the LLM Evaluation Gap for Math.md",
    "1 - Atoms/RealMath-Eval Benchmark Contains 224 Diverse Real Exam Responses.md",
    "1 - Atoms/Current LLM-as-Judge Pipelines Over-rely on Synthetic Evaluation Data.md",
    "1 - Atoms/LLM Judges Near-Perfect at Solving but Struggle to Evaluate Diverse Human Reasoning.md",
    "1 - Atoms/RealMath-Eval Uses Expert Human Grading as Ground Truth Against LLM Judges.md",
    "1 - Atoms/Only 33.5% Cross-Model Agreement on Specific Cultural Substitutions in Math Problems.md",
    "1 - Atoms/Cultural Adaptation Compresses Rather Than Expands Diversity of Entity Values.md",
    "1 - Atoms/LLMs Prioritize Surface Cultural Markers While Preserving Deep Structural Assumptions.md",
    "1 - Atoms/LLMs Produce Cross-Regional Misattribution in Cultural Adaptation of Math Problems.md",
    "1 - Atoms/LLM Adaptation Patterns Cluster by Geographic Region Not Language Boundaries.md",
    "1 - Atoms/6,489 Entity Transformations Annotated Across 76 Cultural Entity Types.md",
    "1 - Atoms/Models Show Stable Individual Tendencies Across All Seven Target Languages.md",
    "1 - Atoms/Surface Plausibility Masks Deeper Cultural Adaptation Failures in LLM Outputs.md",
    "1 - Atoms/Llama-4 Shows +0.800 Coercive Rhetoric Increase Under Turkish Geopolitical Wargame.md",
    "1 - Atoms/DeepSeek-R1 and Gemini-3.1-Pro Show Directional Reversal Under Turkish Conditions.md",
    "1 - Atoms/GPT-4o Shows No Statistically Significant Cross-Lingual Geopolitical Skew.md",
    "1 - Atoms/Chain-of-Thought Institutional Anchoring Buffers Against Cross-Lingual Skew.md",
    "1 - Atoms/Multilingual RLHF Training Can Reverse Rather Than Merely Attenuate Cross-Lingual Bias.md",
    "1 - Atoms/Multi-Agent Wargaming Reveals Latent Ideological Skew That Static Benchmarks Miss.md",
    "1 - Atoms/Inter-Model Variance Within Western Model Class Challenges Monolithic Assumptions.md",
    "1 - Atoms/Cerulean Sea Crisis Wargame Uses SHA-256 Verified Synthetic Statecraft for Reproducibility.md",
    "1 - Atoms/Major Watermark Benchmarks Fail to Evaluate Cross-Lingual or Demographic Fairness.md",
    "1 - Atoms/Non-Native English Speakers Face Higher False Positive Rates in Text Watermarking.md",
    "1 - Atoms/Image Watermark Bias Untested Despite Known Demographic Asymmetries in JPEG Compression.md",
    "1 - Atoms/AudioMarkBench Found Female Speech Has Higher False Positive Rates in Watermark Detection.md",
    "1 - Atoms/Watermarking Is Part of the Pluralistic Alignment Pipeline Requiring Fairness Evaluation.md",
    "1 - Atoms/Governance Frameworks Mandate Watermarking Without Requiring Fairness Evaluation.md",
    "1 - Atoms/Three Proposed Dimensions for Pluralistic Watermark Benchmarking.md",
    "1 - Atoms/Cross-Lingual Translation Removes Watermark Detectability Entirely.md",
]

texts = []
sources = []
for relpath in FILES:
    path = os.path.join(VAULT, relpath)
    if os.path.exists(path):
        with open(path) as f:
            content = f.read()
        # Use the filename without extension as the file_source
        filename = os.path.basename(relpath)
        texts.append(content)
        sources.append(filename)
    else:
        print(f"  WARNING: {path} not found!")

payload = {"texts": texts, "file_sources": sources}
with open("/tmp/lightrag_eval_bias_batch.json", "w") as f:
    json.dump(payload, f)

print(f"Built LightRAG batch: {len(texts)} documents")
print(f"File: /tmp/lightrag_eval_bias_batch.json")
