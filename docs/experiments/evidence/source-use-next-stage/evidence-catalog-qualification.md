# Deterministic passage identity qualification

The pilot exposed quotation-copying failures, so this private experiment helper assigns each passage an immutable identifier derived from source ID, exact context hash and Unicode character offsets. Answers can reference identifiers rather than recreate Markdown quotations. Resolution requires the source to have been delivered to that arm and verifies context, offsets, text and passage hash. Unknown, mutated and cross-arm references fail closed.

Six tests cover Unicode/Markdown reconstruction, empty sources, duplicate IDs, changed contexts, altered passages and cross-arm/unknown references. Applying the helper offline to all four frozen five-source contexts resolved every passage and reconstructed the exact original context. No source text or generated passage catalogs are published. No model, search or Jev calls were made.

This is identity/integrity evidence only. Fixed-width segments may split a sentence or code block; consumers must retain adjacent context and cannot treat a passage identifier as proof that a claim is supported. Segmentation is a controlled proposal, not optimized passage extraction or a demonstrated answer-quality improvement. The completed pilot remains unchanged and EXP-072 remains rejected.

Before any new model run, freeze observable checks for persisted-edit diagnostics versus preview diagnostics, baseline comparison and affected-file scope, and targeted failure reproduction versus broader regression validation. Freeze model requests, token/resource budgets and reference resolution policy separately. A comparison must hold source availability and task semantics constant and count unsupported claims even when their references resolve. New development and untouched confirmation requirements remain in the production acceptance checklist.

See [qualification receipt](evidence-catalog-qualification.json), [helper](evidence-catalog.py.txt) and [tests](evidence-catalog-tests.py.txt). This helper is an experiment artifact and is not wired into product behavior.
