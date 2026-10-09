# Selector resource observations

`coverage_jev_execution` records resource observations from the selector executor itself. Each executed operation records the serialized request and archived response byte counts, actual HTTPX transport dispatch count, parsed input/output usage when known, the per-ledger serialized sequence, and measured maximum concurrent operations under the executor lock. Missing usage remains unknown; configured limits are not substituted for observations.

The provisional result receipt retains `phase_elapsed_ms_before_receipt`, which stops before that receipt is durably written. After the result receipt and any deadline correction have been fsynced, the executor measures elapsed milliseconds and returns the measurement with `CallResult`. `close_stage` verifies the observation against the result and binds it into the terminal inventory. The elapsed value therefore covers execution through the final result/correction fsync, but does not claim to include the later terminal-inventory fsync.

Measurements are local execution evidence, not admission or quality evidence. They do not alter the one-second selector deadline, request/response caps, retry policy, or ledger accounting. If a measurement is missing for an uninvoked or preflight-stopped slot, the terminal inventory records `not-observed` rather than inventing a value.

Elapsed milliseconds round upward so a sub-millisecond overrun cannot appear within the one-second limit. Concurrency observations cover the same study ledger; they do not claim serialization across unrelated ledgers or processes. The qualified stage runner must bind its complete schedule to one ledger.
