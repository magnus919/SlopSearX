# DeepSeek Search (native Rust)

DeepSeek is an optional general-web retrieval adapter. It is disabled by default;
Brave and existing default-engine selection remain unchanged. It calls only the
standalone Search API: no model, answer generation, Fetch, or automatic page
retrieval. No comparative search-quality benefit is claimed.

## Operator configuration

```sh
ENGINE_DEEPSEEK_ENABLED=true
ENGINE_DEEPSEEK_API_KEY=<operator-owned-key>
ENGINE_DEEPSEEK_MAX_RESULTS=10
ENGINE_DEEPSEEK_TIMEOUT_MS=5000
```

Or configure the engine in the normal YAML file, keeping the key in the existing
secret/environment configuration mechanism:

```yaml
engines:
  deepseek:
    enabled: true
    max_results: 10
    timeout_ms: 5000
```

A key alone never enables the engine. Enabled without a key fails with an
instruction naming `ENGINE_DEEPSEEK_API_KEY`; disabled configurations are harmless.
Count must be 1..50, timeout 1..300000 ms, and the key nonblank without header control
characters. Never put keys in URLs or shared fixtures. Secret settings and
Authorization Debug output are redacted. The default endpoint is
`https://api.deepseek.com/v1/tools/search`; an operator endpoint override must be
HTTP(S) without credentials, query parameters, or a fragment. Use HTTPS for the
provider; plain HTTP permits private loopback fixtures and trusted operator tests.

## Native composition handoff

The Rust rewrite's composition root loads `ssx_config`, creates the shared HTTP
owner and F10 admission authority, then calls:

```rust,ignore
let binding = ssx_infra::adapters::register_deepseek(
    &mut registry, http.clone(), admission.clone(), &config.engines["deepseek"],
    &deployment_generation,
)?;
if let Some(binding) = binding {
    engine_bindings.insert("deepseek".into(), binding);
}
```

The nonempty deployment generation is required at registration. Change it on every
configuration or key rotation so a new composition cannot reuse stale cached
captures under the previous identity. Never use the raw key as the generation.

Pass these bindings to the existing `SearchDependencies` and clones of the shared
`SearchService` to HTTP/MCP/workers. Use the same registry for discovery. Do not
create a separate search pipeline. The native HTTP acceptance test exercises this
path with mock upstream responses. Production executable composition, packaging,
and cutover remain owned by the [Rust Rewrite milestone](https://github.com/magnus919/SlopSearX/milestone/11);
the foundation executable is not a provider-enabled deployment command.

## Capabilities and privacy

Queries contain 1..128 Unicode scalar characters and cannot be all whitespace.
Overlong queries fail without truncation, rewriting, splitting, or fan-out.
Explicit site, phrase, language and date operators pass through unchanged.

Only query, configured count and `need_content:false` are sent. Client IP,
identifiers, history, original-question and localization fields are omitted.
Queries are transmitted to DeepSeek. The adapter does not forward provider
summaries, authority tiers or rerank scores into shared ranking semantics. It
uses display snippets; raw text is escaped by the existing HTML renderer. URLs
must be HTTP(S) with a host and no credentials. Invalid entries are skipped;
original positions are preserved. Valid RFC3339 dates are retained; invalid or
missing dates are omitted without dropping valid siblings. New results use Tier 2.

No structured language, relative-time, date, or SafeSearch enforcement is claimed.
Strict SafeSearch is rejected by shared policy before dispatch when the selected
scope cannot enforce it. Page numbers above 1 return an unsupported diagnostic
with zero paid first-page replays; this does not prevent viewing already captured
results through the shared snapshot cursor. In mixed searches other engines can
complete normally. Provider count uses engine configuration, independently of
request `max_results` presentation slices, preserving full cache/snapshot capture.

## Limits, billing and troubleshooting

Verified against provider documentation on **October 27, 2026**: the advertised
price is **$0.75 per 1,000 successful search requests**, independent of count and
model usage. The guide defines billable success as HTTP 200 with nonempty results;
failed/empty requests and timeouts are not billed. The provider also describes a
temporary free period for named integrations; do not assume SlopSearX qualifies.
Prices/terms can change; this is documentation, not billing logic. Search's
100 QPS account-wide limit is a ceiling shared across keys, not a target. The
native descriptor defaults to a conservative per-engine rate of 1; operators
must configure the shared F10 authority for their deployment and other account
users. No public shared key is supplied. Review provider terms and caching or
redistribution permissions before public deployment; this adapter grants none.

- Authentication (`401`): check the operator key; no unchanged retry.
- Insufficient balance (`402`): replenish the provider account; no unchanged retry.
- Rate limit (`429`): feeds shared cooldown; at most three total attempts under
  one caller/adapter deadline. Admission is checked before every attempt.
- Request timeout (`408`) or `5xx`: at most three attempts, using 100/200 ms
  backoff without Retry-After. Retry-After uses the existing shared bounded
  cooldown parser (1..300 seconds); the overall deadline still applies. When
  parsing reaches the 300-second ceiling, no automatic retry is issued, avoiding
  an early retry caused by clipping a longer provider delay.
- Other failures, invalid JSON/envelope or transport failures: classified engine
  errors; provider bodies and credentials are never echoed. Redirects are not
  followed. Response wire/decoded bodies are bounded to 2 MiB. Cancellation drops
  the owned future; no background retry task remains.

Offline fixtures establish request/response and integration behavior, not live
service availability or result quality. A live smoke test requires separately
authorized key usage and spending. Deployment is outside this feature.

Sources: [API reference](https://api-docs.deepseek.com/api/search/),
[guide and billing rules](https://api-docs.deepseek.com/guides/search/),
[pricing](https://api-docs.deepseek.com/quick_start/pricing/).
