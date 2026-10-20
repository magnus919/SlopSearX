# Bounty Objective: Consider adding Marginalia search adapter
Target: magnus919/SlopSearX Issue #271
URL: https://github.com/magnus919/SlopSearX/issues/271

## Issue Description
## Proposal

Consider adding Marginalia as a SlopSearX source. This issue records an evaluation candidate, not approval to implement, purchase access, or enable it by default.

## Potential value

Independent-web and blog discovery could surface useful sources absent from mainstream result sets.

## Access and cost to verify

Official API supports a shared experimental public key and noncommercial access; commercial use has separate licensing. Recheck current terms and rate limits before implementation.

## Source-specific evaluation

Evaluate distinctive useful URLs on niche queries, shared-key throttling, license compatibility, and whether an explicit discovery scope avoids burying unique results under presence ranking.

## Decision criteria

- Capture bounded representative live responses and compare useful incremental coverage, relevance, latency, reliability, and cost with existing sources.
- Confirm current API documentation, access terms, credential needs, rate limits, and caching/redistribution requirements.
- Propose categories and routing scope; new engines remain Tier 2 unless separately approved.
- If accepted, use the standard single-file registered adapter and pooled HTTP lifecycle; classify errors without raising, declare only audited capabilities/filter enforcement, and preserve the shared policy gate.
- Add sanitized captured-response replay tests and bounded live contract coverage, update the README engine table and adapter documentation, and document configuration and cost controls.
- Record a go/no-go decision with evidence. Avoid assuming API availability or synthetic results demonstrate production relevance gains.

## Official references

- https://about.marginalia-search.com/article/api/


## Instructions for Kuro (Autonomous Solver):
1. Use skills: `systematic-debugging` and `code-review-and-quality`.
2. Inspect the repository structure, locate relevant files, and understand the bug/feature.
3. Apply the minimal, robust fix or implementation.
4. Run any existing test suites or linters in the workspace.
5. Ensure the fix adheres to the project's existing code style.
6. When done, output a summary of files modified and verification results.
