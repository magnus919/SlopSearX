# Release notes

## Next release

### Optional TypeSafe Jev specialist routing

Operators can now set `TYPESAFE_API_KEY` to add query-appropriate specialist
engines to ordinary searches. General engines remain in the search base, every
eligible specialist meeting the evidence-backed `0.65` threshold is added, and
there is no arbitrary specialist count cap. Explicit engine/category/media
scopes remain unchanged.

The integration is optional and fail-open: without the key—or when TypeSafe is
unavailable—SlopSearX uses its existing deterministic routing path. See
[TypeSafe Jev specialist routing](JEV_ROUTING.md) for behavior, configuration,
policy boundaries, and routing-card contributor requirements.

The CLI inherits Jev routing through the HTTP API. JSON/YAML and MCP search
responses now identify added specialists; the portal names them, and MCP scope
preview uses the same Jev-aware decision path. An unscoped MCP preview may
incur a Jev call on a cache miss, though it never searches an engine.
