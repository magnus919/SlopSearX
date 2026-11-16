# Portal impact review

These adapters share existing typed SearchResult, registry descriptors, categories, Jev cards, policy and filter declarations. No browser request/response schema change, new filter enforcement claim or new sensitive classification is proposed. HIBP remains uniformly sensitive; IntelX remains non-sensitive. Both real factories must be wired into the shared runtime composition before the portal can claim availability. Current source proposal deliberately does not fabricate active unbound engines.

Required later qualification: existing portal CI gate and deterministic search contracts against actual composed factories; mixed sensitive scope denial, credential unavailable discovery, error/status/result rendering and unsupported filter reports. No browser/provider/production acceptance is claimed here. Full issue932 acceptance still includes DeHashed, actual composition, shared A13 gate and exact-head CI.
