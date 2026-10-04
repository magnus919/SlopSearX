# Root component review: findings before references

## Response envelope

The first envelope understated the longest overall-relation code by one byte per case (`indeterminate` versus `gain_with_loss`). Its context pointer chose the longest selected-union ID rather than the longest visible ID. The original receipt is retained. The revised implementation uses the longest allowed code and JSON-serialized visible ID; corrected bounds remain below 10,000 bytes. This is capacity evidence only.

## Validator

Three independent synthetic uncertainty probes failed: removed-card uncertainty was rejected when the reviewer correctly marked reading uncertainty; affected-card uncertainty was incorrectly accepted with unchanged reading; and independent reading uncertainty was rejected unless another discovery label was uncertain. The registered contract requires the affected-card implication, not its converse. No reference judgment was attempted; preserve the failed receipt and fix the implementation against the unchanged contract.

Other implementation findings: the normal CLI used an incorrect repository ancestor and a stale first qualification receipt; the qualifier carried operational defaults; live trust checked historical registration bytes without verifying all current registered files. The revised live path must use the committed root combined qualification receipt, correct repository root, no private defaults, and frozen/current hash equality. Strict list/enum types and sanitized rejection remain mandatory.

All findings must be addressed and requalified before the first reference. The qualification proves integrity and internal consistency, not visible-evidence truth or ranking uplift.
