# Development stopped on strict-schema request rejection

The development reference-grading phase stopped before producing any grades. Of 54 prepared packets, two grading requests were rejected by the native structured-output interface because the strict schema used the unsupported `propertyNames` keyword on `facet_anchors`. The remaining 52 packets were never started. These were two rejected requests, not completed grading calls.

No grade results were returned or accepted. The stage made no Jev, answer, search, or capture calls, and confirmation was not started. The ranking candidate therefore remains unevaluated; this outcome is not evidence for or against it.

The terminal evidence identifies development stage `43d304b9-02b8-42fd-8c89-8b135cc72262`, packet stage `7a050e3f-ca88-410b-bca7-d724b6ca0b77`, and source revision `43e5fa5a217c0366c33203c0b8ffcc9d3dbec728`. The original request packets and failure receipts remain preserved in the private execution record; this public summary omits their operational metadata. No request was retried, no response was edited, and these identities are retired. Production behavior and deployment remain unchanged.

This occurred after the earlier [grade-response validation failure](../completed-2026-10-15-native-grade-schema/README.md). The earlier responses were not reused. The immediate blocker is schema compatibility: strict structured output does not permit the `propertyNames` keyword used to constrain `facet_anchors`. This old run will not be resumed. The failure is preserved here, and coverage-first remains unqualified for implementation.
