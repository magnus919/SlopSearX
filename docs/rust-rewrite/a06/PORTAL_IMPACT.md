# Scholarly portal impact

This migration preserves the shared SearchResult/AdapterResponse contract and
registered science-engine descriptors. The factories produce the same declared
publication payload fields and provenance, with reviewed typed refusal and
transport/version differences recorded in the component ledgers. No browser
component, navigation, engine activation or new configuration UI is introduced.
F06 factory registration is separate from enabled/active selection policy.

The portal must continue using the shared SearchService, capability catalog,
policy inputs, formatter and canonical cache/view boundary. No adapter-local
HTML or browser policy gate is added. Publication bounds must remain explicit
through form GET/POST, pagination and machine links; unsupported output/filter
combinations must refuse through the shared contract. A source adapter declaring
parameter consumption alone cannot establish enforcement.

OpenAlex's actual upstream date request and row-proof qualification is engine
local. It does not qualify portal-visible date proof, HTTP GET/form POST on both
paths, JSON/YAML enforcement, cache-bound separation, stale-proof reacquisition,
science MCP common-policy behavior, pagination or CSV/RSS refusal. These remain
issue984 integrated acceptance work. The native scholarly gate does not replace
the required portal contract/browser gate or claim a browser acceptance pass.

Semantic Scholar's actual factory/F10/F14 private-Valkey composition verifies
shared cooldown, health and dispatch outcomes. That service-level evidence does
not substitute for portal rendering/accessibility/browser assertions. No live
provider or production request is required or authorized by these tests.
