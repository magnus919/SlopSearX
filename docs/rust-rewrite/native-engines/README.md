# Additive native engines

`catalog.json` declares Rust-only additions beyond the frozen 51-engine F06
migration inventory. Runtime `Registry::builtins()` composes both catalogs and
rejects duplicate names or invalid descriptors. `Registry::migration_baseline()`
keeps the historical parity set available to the F06 gate. Original F06 fragments,
source pins, routing expectations and captures remain unchanged.

F06 validates both compiled sets separately; registry tests check original
per-engine equality, native completeness, routing and duplicate rejection.
DeepSeek's factory/configuration, transport, service/cache and HTTP/HTML tests
prove its executable availability. Static metadata never grants dispatch authority;
configuration, factory binding, credentials, policy and health remain authoritative.

Native additions need their own descriptor and tests, documentation, shared-policy
and portal review. Do not add a Python adapter just to extend a native catalog.
See [DeepSeek operator and integration guide](../../DEEPSEEK.md).
