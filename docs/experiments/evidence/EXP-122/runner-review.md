Substantive runner fallback review of exact head `ea944ec047a2f006d719571f67068bc3d0fa36df`: positive; no blocking code findings. Native Droid jobs were skipped, so the standing fallback applies.

Reviewed the dedicated-owner scope stack, readiness publication, startup failure/cancellation races, serving cancellation/body errors, upstream cleanup and `_state` clearing. The original connection contexts and OAuth/static construction remain intact; their entry/exit now occur in one task. Startup errors propagate, cancelled startup cancels and awaits its owner, and all serving exits signal cleanup and await completion. Cleanup errors are not suppressed. The event-loop-synchronous readiness publication has no partial-ready window.

Tool/schema/call/auth/policy behavior is unchanged; full baseline/candidate discovery and raw controls support that guard. Explicit owner tests cover normal/error/cancel/startup paths. The real HTTP regression inspects server shutdown failure state and owner termination and fails unchanged main. It runs in a fresh process using the existing CI transport-isolation pattern; failed combined-process attempts remain documented. Existing single-global-gateway limitations are unchanged.

The added startup-future cancellation line followed two successful ordinary replay arms; it does not execute on that ordinary path, and final cancellation branches have separate tests. This is a finite lifecycle correction, not measured production availability uplift.

Merge remains conditional on every applicable current-head CI check and resolution of any separate automated-analysis findings.
