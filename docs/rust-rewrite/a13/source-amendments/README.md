# A13 current-source amendment

The A10–A13 qualification records and source maps remain byte-for-byte historical evidence. Their original `http.rs`, DeHashed test, remaining-fixture module, and A11 checker bytes are preserved as `.frozen` files with their baseline Git identities. The amendment manifest binds those historical pins to the exact reviewed current successors and archive bytes; live gates verify both sides without rewriting or recapturing the original records.

This is a source-identity reconciliation only. It does not claim that the `http.rs` successor is behaviorally equivalent to the archived implementation, nor does it retag historical qualification results as evidence for the successor. Current qualification results must be reported separately against the successor source.

The A13 gate also recognizes the one current PyPI timeout unit test added to the `ssx-infra` library inventory; it retains all frozen A13 suite members and its existing Linux-only addition. A02's existing coverage gate owns execution of that PyPI test.
