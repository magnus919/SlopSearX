# Changelog

## [0.3.0](https://github.com/magnus919/SlopSearX/compare/v0.2.0...v0.3.0) (2026-07-12)


### Features

* add jobs search topic and ATS engine adapters (Greenhouse, Ashby, Lever) ([#132](https://github.com/magnus919/SlopSearX/issues/132)) ([dfd517c](https://github.com/magnus919/SlopSearX/commit/dfd517c545a1d84c2a8f4be04a651694e6c18cc1))
* diagnose empty scrape responses ([51ee882](https://github.com/magnus919/SlopSearX/commit/51ee882c96f220a404aa87f11df12459ecf87086))
* diagnose empty scrape responses ([6004d81](https://github.com/magnus919/SlopSearX/commit/6004d8149c9cbdf845649a48f0283048071cc998))


### Bug Fixes

* add type args to bare dict in __init__ for mypy 2.2.0 ([9bc5b1a](https://github.com/magnus919/SlopSearX/commit/9bc5b1a684a9f53c8d48cc9e808ddbd16488066f))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([8f7f8b7](https://github.com/magnus919/SlopSearX/commit/8f7f8b77837f0b44581ace9107aa40053a2a2efc))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([f833807](https://github.com/magnus919/SlopSearX/commit/f8338079db3b8abec1d94cbe483a5f5d247d5b11))
* centralize feature env overrides ([6031f62](https://github.com/magnus919/SlopSearX/commit/6031f625a97283acccd9113066b90a7a3535de5b))
* centralize feature env overrides ([bebdaa3](https://github.com/magnus919/SlopSearX/commit/bebdaa34043989e9e3121285e37e28e8f2169c78))
* **ci:** skip droid-review for dependabot PRs ([a14910b](https://github.com/magnus919/SlopSearX/commit/a14910b33bdac1300cbae957ff326f1c5e5dfa6f))
* install cssselect at runtime ([2433336](https://github.com/magnus919/SlopSearX/commit/243333698ce2f0f7d7550f090af262f35e5afc5e))
* install cssselect at runtime ([e859bc2](https://github.com/magnus919/SlopSearX/commit/e859bc273e7e30089c9b1e73b60ee0f08441cd4c))
* **ratelimit:** remove unused sidecar stub ([9b22616](https://github.com/magnus919/SlopSearX/commit/9b22616ae6c718b7b64673ac0978354f6cab5084))
* **ratelimit:** remove unused sidecar stub ([9ae0dad](https://github.com/magnus919/SlopSearX/commit/9ae0dadaa05e4059f9b33533b625ec4ecce2b03d))


### Documentation

* add engine troubleshooting guidance ([f350d49](https://github.com/magnus919/SlopSearX/commit/f350d496fcf93bbf02cf2920849e8f713e06b2dc))
* add engine troubleshooting guidance ([59e796c](https://github.com/magnus919/SlopSearX/commit/59e796ca1262f829f51c666479c54c552f136af4))

## [0.2.0](https://github.com/magnus919/SlopSearX/compare/v0.1.1...v0.2.0) (2026-07-02)


### Features

* add error tracking, alerting, product analytics, and error-to-insight pipeline ([77caf77](https://github.com/magnus919/SlopSearX/commit/77caf77c044bc61b3a5705368d386d5b66f66cbc))
* add error tracking, alerting, product analytics, and error-to-insight pipeline ([7a7eeea](https://github.com/magnus919/SlopSearX/commit/7a7eeea71a875dbd39f07e55ea8c9bcdc7dd3f5e))
* add feature flag infrastructure and regenerate wiki ([5864cdf](https://github.com/magnus919/SlopSearX/commit/5864cdf86c0c82c848a94d9f0be9e24f62a67fd4))
* add image search support to DuckDuckGo adapter ([#124](https://github.com/magnus919/SlopSearX/issues/124)) ([5bc615e](https://github.com/magnus919/SlopSearX/commit/5bc615e47b1e7eca9c1c370940c0b9d58f7a0a12))
* add pre-commit hooks, complexity, dead-code, duplicate detection, and import-linter ([b2ff3a3](https://github.com/magnus919/SlopSearX/commit/b2ff3a38c12b14dffa5a8fb11d0728c6714491e1))
* add pre-commit hooks, complexity/dead-code/duplicate detection, and import-linter ([8d26d55](https://github.com/magnus919/SlopSearX/commit/8d26d552d4fb284d39e6ff1acc275fffdc34a22f))
* aggressive caching, circuit breaker, query audit trail ([#92](https://github.com/magnus919/SlopSearX/issues/92)) ([#93](https://github.com/magnus919/SlopSearX/issues/93)) ([54bd5be](https://github.com/magnus919/SlopSearX/commit/54bd5becfa99c9924ea17a9f6efca35f52f5e9f6))
* fix 12 remaining Agent Readiness signals ([f4a230a](https://github.com/magnus919/SlopSearX/commit/f4a230acf2081762abc97c3618e157d516c0a27b))
* fix 12 remaining Agent Readiness signals ([615849d](https://github.com/magnus919/SlopSearX/commit/615849d70aa38073ad0d4200f225070bfb107715))


### Bug Fixes

* correct CI regressions from pipeline hardening ([ca3e52b](https://github.com/magnus919/SlopSearX/commit/ca3e52b542bfa32132f8906ff3d45d7f48864943))
* integration tests for CI without Valkey ([1e1e89d](https://github.com/magnus919/SlopSearX/commit/1e1e89d2d1955472d83e2e4eaf26a762bf39c249))
* route Brave adapter by category instead of always hitting /web ([13133e7](https://github.com/magnus919/SlopSearX/commit/13133e79b663a8ef7b2d392d7cd77948e96f3e5c)), closes [#123](https://github.com/magnus919/SlopSearX/issues/123)
* stop hitting Brave API in health checks ([393a746](https://github.com/magnus919/SlopSearX/commit/393a7464d9e5ecffa2f6457c661a2aed883cf50d))


### Documentation

* add feature flag workflow and pre-commit guidance ([1c7a2cf](https://github.com/magnus919/SlopSearX/commit/1c7a2cf7ff3005f8a34e06342651fb25e63e98d2))
